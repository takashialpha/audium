use anyhow::{Context, Result};
use rodio::Source;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::{
    fs::File,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

// ── Channel types ──────────────────────────────────────────────────────────

/// Commands sent from the UI thread to the audio thread.
#[derive(Debug)]
pub enum PlayerCommand {
    Play(PathBuf),
    Stop,
    Pause,
    Resume,
    /// Seek to an absolute position.  `paused` tells the thread whether to
    /// stay paused after repositioning.
    Seek {
        path: PathBuf,
        position: Duration,
        paused: bool,
    },
    SetVolume(f32),
    Quit,
}

/// Events sent from the audio thread back to the UI thread.
#[derive(Debug)] // Error and DurationResolved are never being read, implement them for the features in TODO.md;
pub enum PlayerEvent {
    /// The decoder resolved a total duration for the just-started track.
    DurationResolved(Duration),
    /// Playback of the current track finished naturally (not via Stop).
    TrackFinished,
    /// A file could not be opened / decoded; carries a human-readable reason.
    Error(String),
}

// ── Volume constants ───────────────────────────────────────────────────────

const VOLUME_STEP: f32 = 0.01;
const VOLUME_MIN: f32 = 0.0;
const VOLUME_MAX: f32 = 1.0;

// ── Handle (UI-side) ───────────────────────────────────────────────────────

/// Owned by `AppState`.  Sends commands to the audio thread and receives
/// events from it.  Also tracks UI-side volume and pause state so the UI
/// never has to block on the audio thread for reads.
pub struct PlayerHandle {
    cmd_tx: Sender<PlayerCommand>,
    event_rx: Receiver<PlayerEvent>,

    /// Shadow of the audio thread's volume, kept in sync via `SetVolume`.
    pub volume: f32,
    /// Shadow of the audio thread's pause state.
    pub is_paused: bool,
}

impl PlayerHandle {
    pub fn send(&self, cmd: PlayerCommand) {
        // Errors here mean the audio thread has panicked; ignore gracefully.
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn play(&mut self, path: PathBuf) {
        self.is_paused = false;
        self.send(PlayerCommand::Play(path));
    }

    pub fn stop(&mut self) {
        self.is_paused = false;
        self.send(PlayerCommand::Stop);
    }

    pub fn pause(&mut self) {
        self.is_paused = true;
        self.send(PlayerCommand::Pause);
    }

    pub fn resume(&mut self) {
        self.is_paused = false;
        self.send(PlayerCommand::Resume);
    }

    /// Seek to `position` in the current track.  The path is required because
    /// the audio thread must reopen the file to create a fresh decoder.
    /// `is_paused` on the handle is not changed here — the caller manages that.
    pub fn seek(&self, path: PathBuf, position: Duration, paused: bool) {
        self.send(PlayerCommand::Seek {
            path,
            position,
            paused,
        });
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(VOLUME_MIN, VOLUME_MAX);
        self.send(PlayerCommand::SetVolume(self.volume));
    }

    pub fn volume_up(&mut self) {
        self.set_volume(self.volume + VOLUME_STEP);
    }

    pub fn volume_down(&mut self) {
        self.set_volume(self.volume - VOLUME_STEP);
    }

    /// Drains all pending events without blocking.  Returns them in order.
    pub fn drain_events(&self) -> Vec<PlayerEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            out.push(ev);
        }
        out
    }
}

impl Drop for PlayerHandle {
    fn drop(&mut self) {
        self.send(PlayerCommand::Quit);
    }
}

// ── Audio thread ───────────────────────────────────────────────────────────

/// Spawns the audio thread and returns a `PlayerHandle` for the UI thread.
/// `default_volume` comes from `Settings` so both the handle shadow and the
/// audio thread start at the user's saved value — no post-init correction needed.
pub fn spawn_audio_thread(default_volume: f32) -> Result<PlayerHandle> {
    let volume = default_volume.clamp(VOLUME_MIN, VOLUME_MAX);

    let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();

    // Open the device on the *spawning* thread so we can propagate errors
    // before handing off to the audio thread.
    let mut sink = DeviceSinkBuilder::open_default_sink()
        .context("could not open default audio output sink")?;
    sink.log_on_drop(false);

    thread::Builder::new()
        .name("audium-audio".into())
        .spawn(move || audio_thread_main(sink, cmd_rx, event_tx, volume))?;

    Ok(PlayerHandle {
        cmd_tx,
        event_rx,
        volume,
        is_paused: false,
    })
}

/// Entry point for the audio thread.
fn audio_thread_main(
    sink: MixerDeviceSink,
    cmd_rx: Receiver<PlayerCommand>,
    event_tx: Sender<PlayerEvent>,
    default_volume: f32,
) {
    let player = Player::connect_new(sink.mixer());
    player.set_volume(default_volume);

    let mut stopped_explicitly = false;

    loop {
        // --- Process all pending commands first (non-blocking) ------------
        loop {
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_command(&player, cmd, &mut stopped_explicitly);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // --- Check for natural track completion ---------------------------
        if !stopped_explicitly && player.empty() {
            stopped_explicitly = true;
            let _ = event_tx.send(PlayerEvent::TrackFinished);
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn handle_command(player: &Player, cmd: PlayerCommand, stopped: &mut bool) {
    match cmd {
        PlayerCommand::Play(path) => {
            player.stop();
            *stopped = false;
            match File::open(&path) {
                Err(e) => {
                    eprintln!("audium-audio: failed to open {:?}: {e}", path);
                }
                Ok(file) => match Decoder::try_from(file) {
                    Err(e) => {
                        eprintln!("audium-audio: failed to decode {:?}: {e}", path);
                    }
                    Ok(source) => {
                        player.append(source);
                        player.play();
                    }
                },
            }
        }

        PlayerCommand::Seek {
            path,
            position,
            paused,
        } => {
            player.stop();
            *stopped = false;
            match File::open(&path) {
                Err(e) => eprintln!("audium-audio: seek: failed to open {:?}: {e}", path),
                Ok(file) => match Decoder::try_from(file) {
                    Err(e) => eprintln!("audium-audio: seek: failed to decode {:?}: {e}", path),
                    Ok(mut source) => {
                        let _ = source.try_seek(position);
                        player.append(source);
                        if paused {
                            player.pause();
                        } else {
                            player.play();
                        }
                    }
                },
            }
        }

        PlayerCommand::Stop => {
            *stopped = true;
            player.stop();
        }

        PlayerCommand::Pause => {
            player.pause();
        }

        PlayerCommand::Resume => {
            player.play();
        }

        PlayerCommand::SetVolume(v) => {
            player.set_volume(v.clamp(VOLUME_MIN, VOLUME_MAX));
        }

        PlayerCommand::Quit => {
            player.stop();
        }
    }
}

// ── Duration resolution ────────────────────────────────────────────────────

/// Opens a file purely to ask the decoder for its total duration, without
/// starting playback.  Called from the UI thread after `Play` is dispatched
/// so we can display a progress bar.
///
/// Returns `None` if the duration cannot be determined (e.g. live streams,
/// some MP3s without Xing/VBRI headers).
pub fn resolve_duration(path: &std::path::Path) -> Option<Duration> {
    let file = File::open(path).ok()?;
    let source = Decoder::try_from(file).ok()?;
    source.total_duration()
}
