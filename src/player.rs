use anyhow::{Context, Result};
use rodio::Source;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::{fs::File, path::Path, time::Duration};

const VOLUME_STEP: f32 = 0.1;
const VOLUME_MIN: f32 = 0.0;
const VOLUME_MAX: f32 = 1.0;

/// Main audio player wrapper.
/// Keeps the audio device sink alive so playback doesn't stop unexpectedly.
pub struct AudiumPlayer {
    _sink: MixerDeviceSink,
    player: Player,
    volume: f32,
}

impl AudiumPlayer {
    pub fn new() -> Result<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink()
            .context("could not open default audio output sink")?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
            volume: 0.7,
        })
    }

    /// Stops current playback, loads `path`, starts playing, and returns the
    /// track's total duration if the decoder can determine it.
    pub fn play_file(&mut self, path: &Path) -> Result<Option<Duration>> {
        self.player.stop();
        let file =
            File::open(path).with_context(|| format!("opening audio file: {}", path.display()))?;
        let source = Decoder::try_from(file)
            .with_context(|| format!("decoding audio file: {}", path.display()))?;
        let duration = source.total_duration();
        self.player.append(source);
        self.player.set_volume(self.volume);
        self.player.play();
        Ok(duration)
    }

    pub fn toggle_pause(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(VOLUME_MIN, VOLUME_MAX);
        self.player.set_volume(self.volume);
    }

    pub fn volume_up(&mut self) {
        self.set_volume(self.volume + VOLUME_STEP);
    }

    pub fn volume_down(&mut self) {
        self.set_volume(self.volume - VOLUME_STEP);
    }
}
