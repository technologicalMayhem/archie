use crate::aur::get_last_modified_from_aur;
use crate::messages::{Message, Package};
use crate::scheduler::Error::CouldNotReachAUR;
use crate::state;
use crate::state::{get_build_times, get_last_check, packages, set_last_check};
use crate::stop_token::StopToken;
use bimap::{BiHashMap, BiMap};
use std::collections::HashMap;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{error, info};

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
    let mut retries: HashMap<Package, u8> = HashMap::new();
    let mut workers: BiHashMap<String, Package> = BiMap::new();

    loop {
        if stop_token.stopped() {
            break;
        }

        let now = OffsetDateTime::now_utc().unix_timestamp();

        if next_update_check < now {
            if check_for_package_updates(&sender, stop_token).await.is_ok() {
                next_update_check = now + TIMEOUT;

                retries.clear();
            } else {
                next_update_check = now + RETRY_TIME;
            }
        }

        match receiver.try_recv() {
            Ok(message) => match message {
                Message::AddPackages(packages) => {
                    for package in packages {
                        if !state::is_package_tracked(&package).await {
                            state::track_package(&package).await;
                            send_message(&sender, Message::BuildPackage(package));
                        }
                    }
                }
                Message::RemovePackages(packages) => {
                    state::remove_packages(&packages).await;
                }
                Message::AcceptedWork { package, worker } => {
                    workers.insert(worker, package);
                }
                Message::BuildSuccess(package) => {
                    workers.remove_by_right(&package);
                    retries.remove(&package);
                }
                Message::BuildFailure { worker } => {
                    if let Some(package) = workers.get_by_left(&worker) {
                        if let Some(retries) = retries.get_mut(package) {
                            *retries += 1;
                            if *retries < 3 {
                                send_message(&sender, Message::BuildPackage(package.clone()));
                            }
                        } else {
                            retries.insert(package.clone(), 1);
                        }
                    } else {
                        error!("Failed to lookup worker. This is a bug.");
                    }
                }
                Message::BuildPackage(_) | Message::ArtifactsUploaded { .. } => (),
            },
            Err(TryRecvError::Closed) => break,
            Err(err) => {
                error!("An error occurred whilst trying to read a message: {err}");
            }
        }

        stop_token.sleep(Duration::from_secs(60)).await;
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
    for package in packages().await {
        if let Some(last_check) = get_last_check(&package).await {
            if last_check + TIMEOUT <= now {
                packages_to_check.push(package);
            }
        } else {
            never_built.push(package);
        }
    }

    let last_modified = match get_last_modified_from_aur(&packages_to_check).await {
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
