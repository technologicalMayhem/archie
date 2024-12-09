mod actions;
mod config;
mod util;
mod log_formatter;

use std::process::ExitCode;
use clap::{Parser, Subcommand};
use thiserror::Error;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;
use crate::log_formatter::ColorFormatter;

#[derive(Parser)]
struct Arguments {
    #[command(subcommand)]
    action: Action,
    /// Name of the profile to use
    #[arg(long, default_value = "config" )]
    profile: String
}

#[derive(Subcommand, Clone)]
enum Action {
    /// Add new packages to the coordinator
    Add(actions::Add),
    /// Remove packages from the coordinator
    Remove(actions::Remove),
    /// Display the status of coordinator
    Status,
    /// Setup archie's config
    Init,
}

fn main() -> Result<ExitCode, Error> {
    let subscriber = FmtSubscriber::builder()
        .event_format(ColorFormatter)
        .with_max_level(Level::TRACE) // Log all levels
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let args = Arguments::parse();

    let mut config = config::load(&args.profile);

    if !config.initialized && !matches!(args.action, Action::Init) {
        println!("Archie's config is not set up. Run 'archie init' to set it up.");
        return Ok(ExitCode::FAILURE);
    }

    let exit_code = match args.action {
        Action::Add(add) => actions::add(&config, add)?,
        Action::Remove(remove) => actions::remove(&config, remove)?,
        Action::Status => actions::status(&config)?,
        Action::Init => config::init(&mut config, &args.profile)?,
    };

    Ok(ExitCode::from(exit_code))
}

#[derive(Debug, Error)]
enum Error {
    #[error("{0}")]
    Config(#[from] config::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("An error occurred whilst making a request: {0}")]
    Request(#[from] Box<ureq::Error>),
}
