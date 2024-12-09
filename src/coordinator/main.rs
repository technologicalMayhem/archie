mod aur;
mod config;
mod messages;
mod orchestrator;
mod repository;
mod scheduler;
mod state;
mod stop_token;
mod web_server;

use crate::messages::Message;
use crate::stop_token::StopToken;
use coordinator::abort_if_not_in_docker;
use itertools::Itertools;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast::channel;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tracing::log::info;
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<(), Error> {
    abort_if_not_in_docker();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(FilterFn::new(|msg| msg.target().starts_with("coordinator")))
        .with(LevelFilter::DEBUG)
        .init();

    let mut set = JoinSet::new();
    let mut stop_token = StopToken::new();
    let (send, receive) = channel::<Message>(128);

    info!("Starting application");
    let pkg = state::tracked_packages().await.iter().join("\n");
    if pkg.is_empty() {
        info!("No packages being managed right now");
    } else {
        info!("Packages:\n{pkg}");
    }

    set.spawn(aur::update_non_aur_packages(stop_token.child()));
    set.spawn(web_server::start(send.clone(), stop_token.child()));
    set.spawn(orchestrator::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    set.spawn(repository::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    set.spawn(scheduler::start(
        send.clone(),
        receive.resubscribe(),
        stop_token.child(),
    ));
    set.spawn(setup_stop_mechanism(stop_token));

    set.join_all().await;

    info!("Exited gracefully");
    Ok(())
}

fn register_signals() -> Result<Arc<AtomicBool>, Error> {
    let stop_triggered = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, stop_triggered.clone())?;
    signal_hook::flag::register(SIGTERM, stop_triggered.clone())?;

    Ok(stop_triggered)
}

async fn setup_stop_mechanism(stop_token: StopToken) {
    let Ok(stop_triggered) = register_signals() else {
        stop_token.trigger_stop();
        return;
    };

    while !stop_triggered.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(10)).await;
    }

    stop_token.trigger_stop();
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
}
