use crate::cli::Cli;
use app_base::{
    App,
    app::{Context, Privilege, error::AppError},
};

pub struct MotMot;

impl App for MotMot {
    type Config = ();
    type Cli = Cli;

    fn privilege() -> Privilege {
        Privilege::User
    }

    fn run(&self, ctx: Context<Self::Config, Self::Cli>) -> Result<(), AppError> {
        println!("{:?}", ctx);
        Ok(())
    }
}
