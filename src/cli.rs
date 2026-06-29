use clap::Parser;
use std::path::PathBuf;

pub const APP_NAME: &str = "audium";

#[derive(Debug, Parser, Clone)]
#[command(
    name = APP_NAME,
    version,
    about = "A terminal music app.",
    long_about = "A terminal music app: keyboard-driven, for people who live in the command line."
)]
pub struct Cli {
    /// Audio file to open immediately
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,
}
