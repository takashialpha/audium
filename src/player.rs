use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::{fs::File, path::Path};

const VOLUME_STEP: f32 = 0.05;
const VOLUME_MIN: f32 = 0.0;
const VOLUME_MAX: f32 = 1.0;

/// Main audio player wrapper.
/// Keeps the audio device sink alive so playback doesn't stop unexpectedly.
pub struct AudiumPlayer {
    /// The device sink must stay alive for as long as we want audio to play.
    /// We disable the loud drop warning because we intentionally keep it.
    _sink: MixerDeviceSink,
    player: Player,
    volume: f32,
}

impl AudiumPlayer {
    pub fn new() -> Result<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink()
            .context("could not open default audio output sink")?;

        // Suppress the "Dropping DeviceSink" warning (we are keeping it alive on purpose)
        sink.log_on_drop(false);

        let player = Player::connect_new(sink.mixer());
        player.pause(); // start in paused state

        Ok(Self {
            _sink: sink,
            player,
            volume: 0.7,
        })
    }

    pub fn play_file(&mut self, path: &Path) -> Result<()> {
        self.player.stop();

        let file =
            File::open(path).with_context(|| format!("opening audio file: {}", path.display()))?;

        let source = Decoder::try_from(file)
            .with_context(|| format!("decoding audio file: {}", path.display()))?;

        self.player.append(source);
        self.player.set_volume(self.volume);
        self.player.play();

        Ok(())
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
