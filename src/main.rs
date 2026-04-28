mod app;
mod cli;
mod filepicker;
mod library;
mod modal;
mod player;
mod settings;
mod ui;
mod actions;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

use crate::actions::download_audio_with_binary;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    app::run(cli)
}
