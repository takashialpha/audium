use app_base::{cli::CliArgs, run};
use clap::Parser;

mod app;

const APP_NAME: &str = "audium";

#[derive(Debug, Parser)]
#[command(
    name = APP_NAME,
    version,
    about = "A simple tui audio player made in rust"
)]
struct Cli {}

fn main() {
    let cli = Cli::parse();

    let cli_args = CliArgs { config: None };

    if let Err(e) = run(app::MotMot, None, cli_args) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
