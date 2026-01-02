use crate::APP_NAME;
use app_base::app::ConfigPath;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[command(
    name = APP_NAME,
    version,
    about = "A simple tui music app made in rust"
)]
pub struct Cli {}

impl ConfigPath for Cli {
    fn config_path(&self) -> Option<PathBuf> {
        None
    }
}
