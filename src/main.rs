mod app;
mod cli;
mod filepicker;
mod library;
mod modal;
mod player;
mod settings;
mod ui;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    app::run(cli)
}
