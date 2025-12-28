use crate::error::error::AudioPlayerError;
use crate::fstools::cache;
use std::fs::{self, File};
use std::io;
use std::process::Command;

pub struct SelectFile {
    pub file_path: String,
}

impl SelectFile {
    pub fn new() -> Result<SelectFile, AudioPlayerError> {
        let cache = cache::Cache::init()?;
        let cache_dir = &cache.cache_dir;
        let final_txt_dir = format!("{}/input_ranger_file.txt", cache_dir);
        File::create(&final_txt_dir).map_err(AudioPlayerError::IoError)?;
        Ok(SelectFile {
            file_path: final_txt_dir,
        })
    }

    fn clear_file(&mut self) -> io::Result<()> {
        let file = File::create(&self.file_path)?;
        file.set_len(0)?;
        Ok(())
    }

    pub fn get_file(&mut self) -> Result<(), AudioPlayerError> {
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ranger -c --choosefile={} --selectfile=/home/",
                &self.file_path
            ))
            .status()
            .map_err(AudioPlayerError::IoError)?;

        if !status.success() {
            return Err(AudioPlayerError::FailedToSelectFile);
        }

        self.file_path = fs::read_to_string(&self.file_path)
            .map_err(AudioPlayerError::IoError)?
            .trim()
            .to_string();

        if self.file_path.is_empty() {
            return Err(AudioPlayerError::NoFileSelected);
        }

        Ok(())
    }
}
