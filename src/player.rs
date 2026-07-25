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

// -- Channel types ----------------------------------------------------------

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
    /// Seek to an absolute position. `paused` says whether to stay paused
    /// afterwards; `speed` is re-applied to the fresh decoder.
    Seek {
        path: PathBuf,
        position: Duration,
        paused: bool,
        speed: f32,
    },
    SetVolume(f32),
    /// Change playback speed without restarting the decoder, so it takes
    /// effect mid-track with no audible gap.
    SetSpeed(f32),
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

// -- Volume constants -------------------------------------------------------

const VOLUME_STEP: f32 = 0.01;
const VOLUME_MIN: f32 = 0.0;
const VOLUME_MAX: f32 = 1.0;

// -- Handle (UI-side) -------------------------------------------------------

/// Owned by `AppState`: sends commands to the audio thread and receives its
/// events. Shadows volume and pause state so reads never block on that thread.
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

    /// Seek to `position` in the current track. Takes the path because the
    /// audio thread reopens the file for a fresh decoder. Leaves `is_paused`
    /// to the caller.
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

    /// Changes playback speed in place, with no decoder restart or gap.
    pub fn set_speed(&mut self, speed: f32) {
        self.playback_speed = speed;
        self.send(PlayerCommand::SetSpeed(speed));
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

// -- Audio thread -----------------------------------------------------------

/// Spawns the audio thread and returns its `PlayerHandle`. `default_volume`
/// comes from `Settings`, so both sides start at the saved value with no
/// post-init correction.
pub fn spawn_audio_thread(default_volume: f32) -> Result<PlayerHandle> {
    let volume = default_volume.clamp(VOLUME_MIN, VOLUME_MAX);

    let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();

    // opened on the *spawning* thread so errors propagate before the handoff
    let mut sink = DeviceSinkBuilder::open_default_sink()
        .context("could not open default audio output sink")?;
    sink.log_on_drop(false);

    thread::Builder::new()
        .name("audium-audio".into())
        .spawn(move || audio_thread_main(&sink, &cmd_rx, &event_tx, volume))?;

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
    sink: &MixerDeviceSink,
    cmd_rx: &Receiver<PlayerCommand>,
    event_tx: &Sender<PlayerEvent>,
    default_volume: f32,
) {
    let player = Player::connect_new(sink.mixer());
    player.set_volume(default_volume);
    // speed lives on the Player, so a change applies in place: reopening the
    // file to alter it would stutter
    player.set_speed(1.0);

    let mut stopped_explicitly = true;

    loop {
        // drain pending commands, non-blocking
        loop {
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_command(&player, cmd, &mut stopped_explicitly, event_tx);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // natural track completion
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
                    player.set_speed(speed);
                    player.append(source);
                    player.play();
                }
                Err(e) => {
                    // stopped=true suppresses a spurious TrackFinished, which
                    // would auto-advance forever if every track were broken
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
                    // passed straight through: the Player applies speed to the
                    // rate, not to the seek target
                    let _ = source.try_seek(position);
                    player.set_speed(speed);
                    player.append(source);
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

        PlayerCommand::SetSpeed(s) => {
            player.set_speed(s);
        }

        PlayerCommand::Quit => {
            player.stop();
        }
    }
}

// -- Duration resolution ----------------------------------------------------

/// Checks that `path` opens and decodes, to reject a file (a CLI argument,
/// say) before playback starts.
pub fn validate_decodable(path: &std::path::Path) -> Result<()> {
    open_source(path).map(|_| ())
}

/// Opens a file only to ask the decoder its total duration, for the progress
/// bar. `None` when it cannot be determined, as for an MP3 with no Xing
/// header.
pub fn resolve_duration(path: &std::path::Path) -> Option<Duration> {
    let file = File::open(path).ok()?;
    let source = Decoder::try_from(file).ok()?;
    source.total_duration()
}
