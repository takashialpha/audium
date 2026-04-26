mod app;
mod cli;
mod filepicker;
mod library;
mod modal;
mod player;
mod settings;
mod ui;
mod actions;

use std::path::{Path, PathBuf};
use anyhow::Result;
use clap::Parser;
use cli::Cli;

use actions::download_audio;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    download_audio("https://youtu.be/xj-PCS9HNjs?si=ovoUE8JNRPXszY74");
    app::run(cli)
}
