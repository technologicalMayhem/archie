mod actions;
mod config;
mod util;

use std::process::ExitCode;
use clap::{Parser, Subcommand};
use thiserror::Error;
use tracing::error;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
    tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(fmt::layer().without_time())
        .init();
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
