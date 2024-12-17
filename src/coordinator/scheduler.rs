use crate::messages::{Message, Package};
use crate::query_package::{get_last_modified, PackageData};
use crate::scheduler::Error::CouldNotReachAUR;
use crate::state::{get_build_times, tracked_packages_aur, tracked_packages_url};
use crate::stop_token::StopToken;
use crate::{config, query_package, state};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{debug, error, info, warn};

const RETRY_TIME: i64 = 5 * 60; // 5 minutes

pub async fn start(sender: Sender<Message>, receiver: Receiver<Message>, token: StopToken) {
    run(sender, receiver, token).await;
    info!("Stopping scheduler");
}

async fn run(sender: Sender<Message>, mut receiver: Receiver<Message>, mut token: StopToken) {
    let stop_token = &mut token;
    let mut next_update_check = 0;
    let mut next_retry_check = 0;
    let mut retries: HashMap<Package, u8> = HashMap::new();
    let update_check_interval = i64::from(config::update_check_interval());

    loop {
        let now = OffsetDateTime::now_utc().unix_timestamp();

        if next_update_check < now {
            if check_for_package_updates(&sender).await {
                next_update_check = now + update_check_interval;
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
                Message::AddPackageUrl { url, data } => add_package_url(&sender, url, data).await,
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
                _ => (),
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
    let aur_dependencies = match query_package::get_dependencies(&packages).await {
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
            state::track_package(package.clone(), package_dependencies, dependencies).await;
            info!("Added new package {package}");
            send_message(sender, Message::BuildPackage(package));
        }
    }

    let dependencies: HashSet<Package> = aur_dependencies.into_values().flatten().collect();
    if !dependencies.is_empty() {
        send_message(sender, Message::AddDependencies(dependencies));
    }
}

async fn add_package_url(sender: &Sender<Message>, url: String, data: PackageData) {
    send_message(sender, Message::AddDependencies(data.depends.clone()));
    state::track_package_url(data.name.clone(), url.clone(), data.depends).await;
    send_message(sender, Message::BuildPackage(data.name));
}

async fn check_for_package_updates(sender: &Sender<Message>) -> bool {
    debug!("Checking for package updates");

    let mut success = true;

    if let Err(err) = check_aur_packages(sender).await {
        error!("Failed to check aur packages for updates: {err}");
        success = false;
    }

    if let Err(err) = check_url_packages(sender).await {
        error!("Failed to check url packages for updates");
        for (package, error) in err {
            error!("Error whilst checking {package}: {error}");
        }
        success = false;
    }

    success
}

async fn check_aur_packages(sender: &Sender<Message>) -> Result<(), Error> {
    debug!("Checking aur packages for updates");
    let tracked_packages = tracked_packages_aur().await;
    let mut never_built = tracked_packages.clone();

    let last_modified = match get_last_modified(&tracked_packages).await {
        Ok(last_modified) => last_modified,
        Err(err) => {
            error!("Failed to lookup package info in the AUR: {err}");
            return Err(CouldNotReachAUR);
        }
    };

    for (package, build_time) in get_build_times(&tracked_packages).await {
        if let Some(last_modified) = last_modified.get(&package) {
            if *last_modified > build_time {
                info!("{package} needs to be rebuilt");
                send_message(sender, Message::BuildPackage(package.to_string()));
            }
        }
        never_built.remove(&package);
    }

    for package in never_built {
        info!("{package} needs to be built");
        send_message(sender, Message::BuildPackage(package));
    }

    Ok(())
}

async fn check_url_packages(
    sender: &Sender<Message>,
) -> Result<(), Vec<(Package, query_package::Error)>> {
    debug!("Checking url packages for updates");
    let mut tracked_packages = tracked_packages_url().await;
    let mut never_built = tracked_packages.clone();
    let mut errors = Vec::new();

    for (package, build_time) in get_build_times(&tracked_packages.keys().cloned().collect()).await
    {
        never_built.remove(&package);

        let Some(url) = tracked_packages.remove(&package) else {
            error!("Could not find package url for {package} in the tracked packages. This should never happen!");
            continue;
        };

        match query_package::check_pkgbuild(&url).await {
            Ok(data) => {
                if build_time < data.last_modified {
                    send_message(sender, Message::BuildPackage(package));
                }
            }
            Err(err) => {
                errors.push((package, err));
            }
        }
    }

    for (package, _) in never_built {
        info!("{package} needs to be built");
        send_message(sender, Message::BuildPackage(package));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn send_message(sender: &Sender<Message>, message: Message) {
    if let Err(err) = sender.send(message) {
        error!("There was an error send a message: {err}");
    }
}

#[derive(Debug, Error)]
enum Error {
    #[error("Failed to reach AUR")]
    CouldNotReachAUR,
}
