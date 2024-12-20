use crate::messages::{Message, Package};
use crate::stop_token::StopToken;
use crate::{config, state};
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, StopContainerOptions,
};
use bollard::models::{ContainerStateStatusEnum, HostConfig};
use bollard::Docker;
use futures::future::join_all;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::time::sleep;
use tracing::log::{error, warn};
use tracing::{debug, info};

pub async fn start(sender: Sender<Message>, receiver: Receiver<Message>, stop_token: StopToken) {
    if let Err(err) = run(sender, receiver, stop_token).await {
        error!("Orchestrator stopped with error: {err}");
    } else {
        info!("Stopped orchestrator");
    }
}

struct PackageToBuild {
    package: Package,
    url: String,
}

async fn run(
    sender: Sender<Message>,
    mut receiver: Receiver<Message>,
    mut stop_token: StopToken,
) -> Result<(), Error> {
    let image = config::image();
    let docker = Docker::connect_with_socket_defaults()?;
    if let Err(err) = docker.inspect_image(&image).await {
        return Err(Error::ImageNotAvailable(err));
    }

    let mut packages_to_build = Vec::new();
    let mut active_containers: HashMap<Package, String> = HashMap::new();

    loop {
        if stop_token.stopped() {
            let docker = Arc::new(docker);
            let stop_tasks: Vec<_> = active_containers
                .into_iter()
                .map(|(package, container)| {
                    let docker = docker.clone();
                    async move {
                        if let Err(err) = docker
                            .stop_container(&container, Some(StopContainerOptions { t: 0 }))
                            .await
                        {
                            error!("Failed to stop container {container} for {package}: {err}");
                        };
                        remove_container(&docker, &container).await;
                    }
                })
                .collect();

            join_all(stop_tasks).await;
            return Ok(());
        }
        if !receiver.is_empty() {
            let message = receiver.recv().await?;
            if let Message::BuildPackage(package) = message {
                packages_to_build.push(PackageToBuild {
                    url: state::get_build_url(&package).await.unwrap_or_default(),
                    package,
                });
            } else if let Message::RemovePackages(packages) = message {
                for package in packages {
                    if let Some(index) = packages_to_build
                        .iter()
                        .position(|to_build| to_build.package == package)
                    {
                        packages_to_build.remove(index);
                    }
                    if let Some(container) = active_containers.remove(&package) {
                        info!("Stopping build of package {package}, as it has been removed.");
                        if let Err(err) = docker
                            .stop_container(&container, Some(StopContainerOptions { t: 0 }))
                            .await
                        {
                            error!("Failed to stop container {container} for {package}: {err}");
                        };
                        if let Err(err) = docker.remove_container(&container, None).await {
                            error!("Failed to stop container {container} for {package}: {err}");
                        };
                    }
                }
            }
        }
        if active_containers.len() < config::max_builders() {
            if let Some(index) = {
                let mut index = None;
                for (i, pkg) in packages_to_build.iter().enumerate() {
                    if state::are_dependencies_met(&pkg.package).await {
                        index = Some(i);
                        break;
                    }
                }
                index
            } {
                let build = packages_to_build.remove(index);
                let container_id =
                    start_build_container(&docker, &image, &build.package, &build.url).await?;
                active_containers.insert(build.package, container_id);
            }
        }
        clean_up_containers(&docker, &sender, &mut active_containers).await?;
        sleep(Duration::from_millis(100)).await;
    }
}

async fn start_build_container(
    docker: &Docker,
    image: &str,
    package: &Package,
    url: &str,
) -> Result<String, Error> {
    let options = CreateContainerOptions {
        name: package.to_string(),
        ..Default::default()
    };
    let env_package = format!("PACKAGE={package}");
    let env_url = format!("URL={url}");
    let env_repo = format!("REPO={}", config::repo_name());
    let env_port = format!("PORT={}", config::port());
    let config = Config {
        image: Some(image),
        env: Some(vec![&env_package, &env_url, &env_repo, &env_port]),
        host_config: Some(HostConfig {
            memory: config::max_memory(),
            ..Default::default()
        }),
        ..Default::default()
    };

    let response = docker.create_container(Some(options), config).await?;
    debug!("Created container {} for {package}", response.id);
    if !response.warnings.is_empty() {
        warn!("Encountered warnings:");
    }
    for x in response.warnings {
        warn!("{x}");
    }

    docker.start_container::<String>(&response.id, None).await?;
    state::add_running_container(response.id[0..12].to_string()).await;
    Ok(response.id)
}

async fn clean_up_containers(
    docker: &Docker,
    sender: &Sender<Message>,
    active_containers: &mut HashMap<Package, String>,
) -> Result<(), Error> {
    let mut removed: Vec<Package> = Vec::new();
    for (package, id) in active_containers.iter() {
        let container = match docker.inspect_container(id, None).await {
            Ok(container) => container,
            Err(err) => {
                warn!("Failed to inspect container {id}: {err}");
                continue;
            }
        };
        let Some(state) = container.state else {
            error!("Could not get container state for {id}");
            continue;
        };
        let Some(status) = state.status else {
            error!("Container {id} does not have a status");
            continue;
        };

        let Some(exit_code) = state.exit_code else {
            error!("Container {id} does not have a exit code");
            continue;
        };

        match status {
            ContainerStateStatusEnum::EXITED => {
                if exit_code != 0 {
                    warn!("{id} exited abnormally. Printing logs:");
                    get_logs(docker, id).await;
                    if let Err(err) = sender.send(Message::BuildFailure(package.to_string())) {
                        error!("Failed to send message: {err}");
                    }
                }
                remove_container(docker, id).await;
                state::remove_running_container(&id[0..12]).await;
                removed.push(package.to_owned());
                continue;
            }
            ContainerStateStatusEnum::CREATED
            | ContainerStateStatusEnum::RESTARTING
            | ContainerStateStatusEnum::PAUSED
            | ContainerStateStatusEnum::DEAD
            | ContainerStateStatusEnum::EMPTY
            | ContainerStateStatusEnum::REMOVING => {
                warn!("Container ({id}) in unusual state: {status}.");
            }
            ContainerStateStatusEnum::RUNNING => (),
        }
    }

    for package in removed {
        active_containers.remove(&package);
    }

    Ok(())
}

async fn remove_container(docker: &Docker, id: &str) {
    if let Err(err) = docker.remove_container(id, None).await {
        warn!("Failed to remove container {id}: {err}");
    } else {
        debug!("Cleaned up {id}");
    }
}

async fn get_logs(docker: &Docker, id: &str) {
    let mut logs = docker.logs::<String>(
        id,
        Some(LogsOptions {
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );

    let mut entries = Vec::new();
    while let Some(log_result) = logs.next().await {
        match log_result {
            Ok(log) => {
                let (t, mut m) = match log {
                    LogOutput::StdErr { message } => {
                        ("ERR", String::from_utf8_lossy(&message).to_string())
                    }
                    LogOutput::StdOut { message } => {
                        ("OUT", String::from_utf8_lossy(&message).to_string())
                    }
                    LogOutput::StdIn { message } => {
                        ("IN", String::from_utf8_lossy(&message).to_string())
                    }
                    LogOutput::Console { message } => {
                        ("CON", String::from_utf8_lossy(&message).to_string())
                    }
                };

                if m.ends_with('\n') {
                    m.pop();
                }
                entries.push(format!("{t} - {m}"));
            }
            Err(err) => {
                entries.push(format!("Error for log entry: {err}"));
            }
        }
    }

    let full_log = entries.join("\n");
    warn!("{full_log}");
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Bollard error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Could not query image.")]
    ImageNotAvailable(bollard::errors::Error),
    #[error("Channel error {0}")]
    Channel(#[from] tokio::sync::broadcast::error::RecvError),
    #[error("Failed to read environment variable: {0}")]
    Env(#[from] std::env::VarError),
    #[error("A join error occurred: {0}")]
    Join(#[from] tokio::task::JoinError),
}
