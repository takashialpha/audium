use crate::APP_NAME;
use clap::Parser;

#[derive(Debug, Parser, Clone)]
#[command(
    name = APP_NAME,
    version,
    about = "A simple tui music app made in rust"
)]
pub struct Cli {}
