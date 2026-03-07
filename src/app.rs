use crate::cli::Cli;
use app_base::{
    App,
    app::{Context, Privilege, error::AppError},
};

pub struct Audium;

impl App for Audium {
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
