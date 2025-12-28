use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(PartialEq)]
enum State {
    WaitingForFile,
    Paused,
    Playing,
}

pub struct AudioPlayer {
    state: State,
    sink: Option<Sink>,
    current_file_name: Option<String>,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, String> {
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to initialize audio output: {}", e))?;

        Ok(AudioPlayer {
            state: State::WaitingForFile,
            sink: None,
            current_file_name: None,
            _stream,
            stream_handle,
        })
    }

    pub fn play_file(&mut self, path: PathBuf) -> Result<(), String> {
        self.current_file_name = path
            .file_stem()
            .and_then(|os_str| os_str.to_os_string().into_string().ok());

        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode audio file: {}", e))?;

        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        sink.append(decoder);
        sink.play();

        self.sink = Some(sink);
        self.state = State::Playing;

        Ok(())
    }

    pub fn toggle_playing(&mut self) {
        if let Some(sink) = &self.sink {
            match self.state {
                State::Playing => {
                    self.state = State::Paused;
                    sink.pause();
                }
                State::Paused => {
                    self.state = State::Playing;
                    sink.play();
                }
                _ => {}
            }
        }
    }
    pub fn progress(&self) -> f32 {
        // temporary not working
    }

    pub fn pause_or_play_button_text(&self) -> &str {
        match self.state {
            State::Playing => "Pause",
            _ => "Play",
        }
    }

    pub fn restart(&mut self) {
        if let Some(ref mut sink) = self.sink {
            sink.stop();
        }
        self.state = State::WaitingForFile;
        self.sink = None; // Drop the sink (temporary not working)
    }
}
