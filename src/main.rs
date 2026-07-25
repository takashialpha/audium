mod app;
mod cli;
mod filepicker;
mod library;
mod lyrics;
mod modal;
mod nav;
mod numeric;
mod player;
mod settings;
mod state;
mod ui;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    app::run(cli)
}
