#![warn(clippy::pedantic)]
mod config;
mod messages;
mod orchestrator;
mod query_package;
mod repository;
mod scheduler;
mod state;
mod stop_token;
mod web_server;

use crate::messages::Message;
use crate::stop_token::StopToken;
use coordinator::{abort_if_not_in_docker, combine_for_display, print_version};
use signal_hook::consts::{SIGINT, SIGTERM};
use std::collections::HashMap;
use std::env::var;
use std::fs::{copy, create_dir_all, exists, read_to_string, write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use itertools::Itertools;
use thiserror::Error;
use tokio::sync::broadcast::channel;
use tokio::task::{Id, JoinSet};
use tokio::time::sleep;
use tracing::error;
use tracing::log::info;
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*};

pub const SSH_KEY_PATH: &str = "/config/id_ed25519";

#[tokio::main]
async fn main() -> Result<(), Error> {
    abort_if_not_in_docker();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(FilterFn::new(|msg| msg.target().starts_with("coordinator")))
        .with(get_log_level())
        .init();
    print_version();

    setup_ssh()?;

    let mut set = JoinSet::new();
    let mut master_stop_token = StopToken::new();
    let mut stop_token = master_stop_token.child();
    let (send, receive) = channel::<Message>(128);

    let pkg = state::tracked_packages().await;
    if pkg.is_empty() {
        info!("No packages being managed right now");
    } else {
        info!("Managing {}", combine_for_display(pkg));
    }

    let aur_update = set.spawn(query_package::update_non_aur_packages(stop_token.child()));
    let web_server = set.spawn(web_server::start(send.clone(), stop_token.child()));
    let orchestrator = set.spawn(orchestrator::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    let repository = set.spawn(repository::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    let scheduler = set.spawn(scheduler::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    let signal_listener = set.spawn(setup_stop_mechanism(master_stop_token));

    let task_ids: HashMap<Id, &str> = HashMap::from([
        (aur_update.id(), "AUR Updater"),
        (web_server.id(), "web Server"),
        (orchestrator.id(), "orchestrator"),
        (repository.id(), "repository"),
        (scheduler.id(), "scheduler"),
        (signal_listener.id(), "signal listener"),
    ]);

    handle_task_exits(&mut set, &mut stop_token, task_ids).await;

    Ok(())
}

async fn handle_task_exits(
    set: &mut JoinSet<()>,
    stop_token: &mut StopToken,
    task_ids: HashMap<Id, &str>,
) {
    let mut graceful = true;
    while let Some(join) = set.join_next_with_id().await {
        match join {
            Ok((id, ())) => {
                if !stop_token.stopped() {
                    if let Some(task) = task_ids.get(&id) {
                        error!("{task} quit unexpectedly.");
                    } else {
                        error!("Failed to lookup task id for a task that quit unexpectedly. This should not happen");
                    }

                    graceful = false;
                }
            }
            Err(err) => {
                if let Some(task) = task_ids.get(&err.id()) {
                    error!("{task} quit unexpectedly.");
                } else {
                    error!("Failed to lookup task id for a task that quit unexpectedly. This should not happen");
                }
                error!("The error was: {:?}", std::error::Error::source(&err));
                stop_token.trigger_stop();
                graceful = false;
            }
        }
    }

    if graceful {
        info!("Exited gracefully");
    }
}

fn register_signals() -> Result<Arc<AtomicBool>, Error> {
    let stop_triggered = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, stop_triggered.clone())?;
    signal_hook::flag::register(SIGTERM, stop_triggered.clone())?;

    Ok(stop_triggered)
}

async fn setup_stop_mechanism(stop_token: StopToken) {
    match register_signals() {
        Ok(stop_triggered) => {
            while !stop_triggered.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(10)).await;
            }
        }
        Err(err) => {
            error!("Failed to register signal hooks: {err}")
        }
    }
    stop_token.trigger_stop();
}

fn get_log_level() -> LevelFilter {
    let level = var("LOG_LEVEL").unwrap_or_default();
    match level.to_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        "off" => LevelFilter::OFF,
        _ => LevelFilter::INFO,
    }
}

fn setup_ssh() -> Result<(), Error> {
    if !exists(SSH_KEY_PATH)? {
        let output = Command::new("ssh-keygen")
            .args(["-f", SSH_KEY_PATH, "-t", "ed25519"])
            .output()?;
        if !output.status.success() {
            return Err(Error::GenerateKey)
        }
        info!("Generated a new SSH key.");
    }
    create_dir_all("/root/.ssh/")?;
    copy(SSH_KEY_PATH, "/root/.ssh/id_ed25519")?;
    write("/root/.ssh/config", "StrictHostKeyChecking accept-new")?;

    let pub_key = read_to_string(format!("{SSH_KEY_PATH}.pub"))?;
    let trimmed_key = pub_key.trim().split(' ').take(2).join(" ");

    info!("The public key is:");
    info!("{trimmed_key}");

    Ok(())
}

#[derive(Debug, Error)]
enum Error {
    #[error("Failed to make a request: {0}")]
    Ureq(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Deserialize error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] orchestrator::Error),
    #[error("State error: {0}")]
    State(#[from] state::Error),
    #[error("Failed to generate ssh key")]
    GenerateKey
}
