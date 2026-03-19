use clap::Parser;
use std::path::PathBuf;

pub const APP_NAME: &str = "audium";

#[derive(Debug, Parser, Clone)]
#[command(
    name = APP_NAME,
    version,
    about = "A simple tui music app made in rust"
)]
pub struct Cli {
    /// Music file to open
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,
}
