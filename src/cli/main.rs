#![warn(clippy::pedantic)]
mod actions;
mod config;
mod log_formatter;
mod util;

use crate::log_formatter::ColorFormatter;
use clap::{Parser, Subcommand};
use std::process::ExitCode;
use thiserror::Error;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;
use ureq::ErrorKind;
use coordinator::print_version;

#[derive(Parser)]
struct Arguments {
    #[command(subcommand)]
    action: Action,
    /// Name of the profile to use
    #[arg(long, default_value = "config")]
    profile: String,
}

#[derive(Subcommand, Clone)]
enum Action {
    /// Add new packages to the coordinator
    Add(actions::Add),
    /// Remove packages from the coordinator
    Remove(actions::Remove),
    /// Force the coordinator to rebuild the package
    Rebuild(actions::Rebuild),
    /// Display the status of coordinator
    Status,
    /// Setup archie's config
    Init,
    /// Print version info
    Version,
}

fn main() -> Result<ExitCode, Error> {
    let subscriber = FmtSubscriber::builder()
        .event_format(ColorFormatter)
        .with_max_level(Level::TRACE) // Log all levels
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Arguments::parse();

    let mut config = config::load(&args.profile);

    if !config.initialized && !matches!(args.action, Action::Init) {
        println!("Archie's config is not set up. Run 'archie init' to set it up.");
        return Ok(ExitCode::FAILURE);
    }

    let result = match args.action {
        Action::Add(add) => actions::add(&config, add),
        Action::Remove(remove) => actions::remove(&config, remove),
        Action::Rebuild(rebuild) => actions::rebuild(&config, &rebuild),
        Action::Status => actions::status(&config),
        Action::Init => config::init(&mut config, &args.profile).map_err(Error::from),
        Action::Version => {
            print_version();
            Ok(0)
        },
    };

    let exit_code = try_to_interpret_error(result)?;
    Ok(ExitCode::from(exit_code))
}

/// Tries to get more information to display to the user from the error.
fn try_to_interpret_error(result: Result<u8, Error>) -> Result<u8, Error> {
    Ok(match result {
        Ok(exit_code) => exit_code,
        Err(Error::Request(err)) => match err.kind() {
            ErrorKind::Dns | ErrorKind::ConnectionFailed | ErrorKind::TooManyRedirects => {
                let transport = err.into_transport().unwrap();
                error!("{transport}");
                1
            }
            _ => return Err(Error::Request(err)),
        },
        Err(err) => return Err(err),
    })
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
