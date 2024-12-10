use crate::aur::get_last_modified;
use crate::messages::{Message, Package};
use crate::scheduler::Error::CouldNotReachAUR;
use crate::state::{get_build_times, get_last_check, set_last_check, tracked_packages};
use crate::stop_token::StopToken;
use crate::{aur, config, state};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use time::OffsetDateTime;
use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{error, info, warn};

const TIMEOUT: i64 = 4 * 60 * 60; // 4 Hours
const RETRY_TIME: i64 = 5 * 60; // 5 minutes

pub async fn start(sender: Sender<Message>, receiver: Receiver<Message>, token: StopToken) {
    info!("Starting");
    run(sender, receiver, token).await;
    info!("Stopping scheduler");
}

async fn run(sender: Sender<Message>, mut receiver: Receiver<Message>, mut token: StopToken) {
    let stop_token = &mut token;
    let mut next_update_check = 0;
    let mut next_retry_check = 0;
    let mut retries: HashMap<Package, u8> = HashMap::new();

    loop {
        let now = OffsetDateTime::now_utc().unix_timestamp();

        if next_update_check < now {
            if check_for_package_updates(&sender, stop_token).await.is_ok() {
                next_update_check = now + TIMEOUT;
                retries.clear();
            } else {
                next_update_check = now + RETRY_TIME;
            }
        }

        if next_retry_check < now {
            for (package, attempt) in &retries {
                if *attempt < config::max_retries() {
                    info!("Retrying build for {package}");
                    send_message(&sender, Message::BuildPackage(package.clone()));
                }
            }
            next_retry_check = now + RETRY_TIME;
        }

        let message: Option<Result<Message, RecvError>> = select! {
            message = receiver.recv() => Some(message),
            () = stop_token.sleep(Duration::from_secs(60)) => None,
        };
        if stop_token.stopped() {
            break;
        }

        match message {
            Some(Ok(message)) => match message {
                Message::AddPackages(packages) => {
                    add_package(&sender, packages, false).await;
                }
                Message::AddDependencies(packages) => {
                    add_package(&sender, packages, true).await;
                }
                Message::RemovePackages(packages) => {
                    state::remove_packages(&packages).await;
                    info!("Stopped tracking {}", packages.iter().join(", "));
                    let unneeded = state::unneeded_dependencies().await;
                    if !unneeded.is_empty() {
                        send_message(&sender, Message::RemovePackages(unneeded));
                    }
                }
                Message::BuildSuccess(package) => {
                    retries.remove(&package);
                }
                Message::BuildFailure(package) => {
                    if let Some(retries) = retries.get_mut(&package) {
                        *retries += 1;
                    } else {
                        retries.insert(package.clone(), 1);
                    };
                }
                Message::BuildPackage(_) | Message::ArtifactsUploaded { .. } => (),
            },
            Some(Err(RecvError::Closed)) => {
                error!("Message channel closed");
                break;
            }
            Some(Err(RecvError::Lagged(lag))) => {
                error!("The message channel lagged by {lag}. This should not happen!");
            }
            _ => (),
        }
    }
}

async fn add_package(sender: &Sender<Message>, packages: HashSet<Package>, dependencies: bool) {
    let aur_dependencies = match aur::get_dependencies(&packages).await {
        Ok(deps) => deps,
        Err(err) => {
            error!(
                "Failed to fetch dependencies for {packages:?}. Could not add them. Error: {err}"
            );
            return;
        }
    };

    let mut dependency_copies = aur_dependencies.clone();
    for package in packages {
        if !state::is_package_tracked(&package).await {
            let Some(package_dependencies) = dependency_copies.remove(&package) else {
                warn!("Failed to get dependencies for {package}. This might mean it is a meta package");
                continue;
            };
            state::track_package(&package, package_dependencies, dependencies).await;
            info!("Added new package {package}");
            send_message(sender, Message::BuildPackage(package));
        }
    }

    let dependencies: HashSet<Package> = aur_dependencies.into_values().flatten().collect();
    if !dependencies.is_empty() {
        send_message(sender, Message::AddDependencies(dependencies));
    }
}

async fn check_for_package_updates(
    sender: &Sender<Message>,
    stop_token: &mut StopToken,
) -> Result<(), Error> {
    info!("Checking for package updates");
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut packages_to_check = Vec::new();
    let mut never_built = Vec::new();
    for package in tracked_packages().await {
        if let Some(last_check) = get_last_check(&package).await {
            if last_check + TIMEOUT <= now {
                packages_to_check.push(package);
            }
        } else {
            never_built.push(package);
        }
    }

    let last_modified = match get_last_modified(&packages_to_check).await {
        Ok(last_modified) => last_modified,
        Err(err) => {
            error!("Failed to lookup package info in the AUR: {err}");
            stop_token.sleep(Duration::from_secs(5 * 60)).await;
            return Err(CouldNotReachAUR);
        }
    };

    for (package, build_time) in get_build_times(&packages_to_check).await {
        if let Some(last_modified) = last_modified.get(package) {
            if *last_modified > build_time {
                info!("{package} needs to be rebuilt");
                send_message(sender, Message::BuildPackage(package.to_string()));
            } else {
                set_last_check(package, now).await;
            }
        }
    }

    for package in never_built {
        info!("{package} needs to be built");
        send_message(sender, Message::BuildPackage(package));
    }

    Ok(())
}

fn send_message(sender: &Sender<Message>, message: Message) {
    if let Err(err) = sender.send(message) {
        error!("There was an error send a message: {err}");
    }
}

enum Error {
    CouldNotReachAUR,
}
