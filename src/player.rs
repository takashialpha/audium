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
    Play {
        path: PathBuf,
        speed: f32,
    },
    Stop,
    Pause,
    Resume,
    /// Seek to an absolute position.  `paused` tells the thread whether to
    /// stay paused after repositioning.  `speed` is re-applied to the fresh
    /// decoder so speed changes take effect immediately.
    Seek {
        path: PathBuf,
        position: Duration,
        paused: bool,
        speed: f32,
    },
    SetVolume(f32),
    Quit,
}

/// Events sent from the audio thread back to the UI thread.
#[derive(Debug)]
pub enum PlayerEvent {
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
    /// Current playback speed multiplier, sent with every Play/Seek command.
    pub playback_speed: f32,
}

impl PlayerHandle {
    pub fn send(&self, cmd: PlayerCommand) {
        // Errors here mean the audio thread has panicked; ignore gracefully.
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn play(&mut self, path: PathBuf) {
        self.is_paused = false;
        self.send(PlayerCommand::Play {
            path,
            speed: self.playback_speed,
        });
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
    /// `is_paused` on the handle is not changed here; the caller manages that.
    pub fn seek(&self, path: PathBuf, position: Duration, paused: bool) {
        self.send(PlayerCommand::Seek {
            path,
            position,
            paused,
            speed: self.playback_speed,
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
/// audio thread start at the user's saved value; no post-init correction needed.
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
        playback_speed: 1.0,
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

    let mut stopped_explicitly = true;

    loop {
        // --- Process all pending commands first (non-blocking) ------------
        loop {
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_command(&player, cmd, &mut stopped_explicitly, &event_tx);
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

// Opens and decodes an audio file; returns the source or a human-readable error.
fn open_source(path: &std::path::Path) -> Result<Decoder<std::io::BufReader<File>>> {
    let file =
        File::open(path).with_context(|| format!("could not open \"{}\"", path.display()))?;
    Decoder::try_from(file)
        .map_err(|e| anyhow::Error::msg(format!("could not decode \"{}\": {e}", path.display())))
}

fn handle_command(
    player: &Player,
    cmd: PlayerCommand,
    stopped: &mut bool,
    event_tx: &Sender<PlayerEvent>,
) {
    match cmd {
        PlayerCommand::Play { path, speed } => {
            player.stop();
            match open_source(&path) {
                Ok(source) => {
                    *stopped = false;
                    player.append(source.speed(speed));
                    player.play();
                }
                Err(e) => {
                    // Keep stopped=true so the audio loop does not fire a
                    // spurious TrackFinished (which would cause auto-advance
                    // into an infinite error loop if all tracks are broken).
                    *stopped = true;
                    let _ = event_tx.send(PlayerEvent::Error(e.to_string()));
                }
            }
        }

        PlayerCommand::Seek {
            path,
            position,
            paused,
            speed,
        } => {
            player.stop();
            match open_source(&path) {
                Ok(mut source) => {
                    *stopped = false;
                    let _ = source.try_seek(position);
                    player.append(source.speed(speed));
                    if paused {
                        player.pause();
                    } else {
                        player.play();
                    }
                }
                Err(e) => {
                    *stopped = true;
                    let _ = event_tx.send(PlayerEvent::Error(format!("Seek failed: {e}")));
                }
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

/// Checks that `path` can be opened and decoded as audio.
/// Used to reject files up front (e.g. a CLI argument) before playback starts.
pub fn validate_decodable(path: &std::path::Path) -> Result<()> {
    open_source(path).map(|_| ())
}

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
