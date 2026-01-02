use app_base::{
    App,
    app::{Context, Privilege, error::AppError},
};

use crate::cli::Cli;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AppConfig;

pub struct MotMot;

impl App for MotMot {
    type Config = AppConfig;
    type Cli = Cli;

    fn privilege() -> Privilege {
        Privilege::User
    }

    fn run(&self, ctx: Context<Self::Config, Self::Cli>) -> Result<(), AppError> {
        println!("{:?}", ctx);
        todo!()
    }
}
