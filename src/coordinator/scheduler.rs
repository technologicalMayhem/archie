use crate::aur::get_last_modified_from_aur;
use crate::stop_token::StopToken;
use coordinator::WorkAssignment;
use futures::future::join_all;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::broadcast::Sender;
use tokio::spawn;
use tracing::{error, info};
use crate::state::{get_build_times, get_last_check, packages, set_last_check};

const TIMEOUT: i64 = 4 * 60 * 60; // 4 Hours

pub async fn start(work_sender: Sender<WorkAssignment>, mut token: StopToken) {
    let rebuilds = spawn(schedule_rebuilds(
        work_sender.clone(),
        token.child(),
    ));
    let retries = spawn(schedule_retries(work_sender, token.child()));

    join_all(vec![rebuilds, retries, token.wait_spawn()]).await;

    info!("Stopping scheduler");
}

async fn schedule_rebuilds(
    work_sender: Sender<WorkAssignment>,
    mut stop_token: StopToken,
) {
    let stop_token = &mut stop_token;
    info!("Starting");
    loop {
        if stop_token.stopped() {
            break;
        }

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
                continue;
            }
        };

        for (package, build_time) in get_build_times(&packages_to_check).await {
            if let Some(last_modified) = last_modified.get(package) {
                if *last_modified > build_time {
                    info!("{package} needs to be rebuilt");
                    let _result = work_sender.send(WorkAssignment {
                        package: package.to_string(),
                    });
                } else {
                    set_last_check(package, now).await;
                }
            }
        }

        for package_name in never_built {
            info!("{package_name} needs to be built");
            let _result = work_sender.send(WorkAssignment {
                package: package_name,
            });
        }

        stop_token.sleep(Duration::from_secs(TIMEOUT as u64)).await;
    }
}

async fn schedule_retries(
    work_sender: Sender<WorkAssignment>,
    mut stop_token: StopToken,
) {
}
