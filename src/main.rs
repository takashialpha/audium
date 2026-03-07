use app_base::run;
use audium::{app, cli::Cli};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(app::Audium, None, cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
