use std::fs::exists;
use crate::messages::{Message, Package};
use crate::stop_token::StopToken;
use crate::{config, state};
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use tokio::fs::{remove_file, try_exists};
use tokio::select;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{debug, error};
use tracing::log::info;

pub const REPO_DIR: &str = "/output/";
const REPO_ADD: &str = "repo-add";
const REPO_REMOVE: &str = "repo-remove";

pub async fn start(sender: Sender<Message>, receive: Receiver<Message>, stop_token: StopToken) {
    if let Err(err) = run_repository(sender, receive, stop_token).await {
        error!("Stopped with an error: {err}");
    }
}

async fn run_repository(
    sender: Sender<Message>,
    mut receive: Receiver<Message>,
    mut stop_token: StopToken,
) -> Result<(), Error> {
    let repo_name = config::repo_name();

    recreate_repo(&repo_name).await;

    loop {
        let artifact = select! {
            work = receive.recv() => Some(work),
            () = stop_token.wait() => None,
        };
        let Some(Ok(message)) = artifact else {
            break;
        };

        match message {
            Message::ArtifactsUploaded {
                package,
                files,
                build_time,
            } => {
                info!("Successfully built {}", package);

                if add_to_repo(&repo_name, &files) {
                    state::build_package(&package, build_time, files).await;
                    if let Err(err) = sender.send(Message::BuildSuccess(package.clone())) {
                        error!("Failed to send message: {err}");
                    }
                }
            }
            Message::RemovePackages(packages) => {
                let mut files = Vec::new();
                let mut packages_to_remove = Vec::new();
                for package in packages {
                    let mut package_files = state::get_files(&package).await;
                    if !package_files.is_empty() {
                        files.append(&mut package_files);
                        packages_to_remove.push(package);
                    }
                }
                remove_from_repo(&repo_name, &files, &packages_to_remove);
            }
            Message::AddPackages(_)
            | Message::AddDependencies(_)
            | Message::BuildPackage(_)
            | Message::BuildSuccess(_)
            | Message::BuildFailure { .. } => (),
        }
    }

    info!("Stopped repository");
    Ok(())
}

async fn recreate_repo(repo_name: &str) {
    debug!("Recreating repository");

    let repo_files = vec![
        format!("{REPO_DIR}{repo_name}.db"),
        format!("{REPO_DIR}{repo_name}.db.tar.zst"),
        format!("{REPO_DIR}{repo_name}.files"),
        format!("{REPO_DIR}{repo_name}.files.tar.zst"),
    ];

    for file in repo_files {
        if try_exists(&file).await.ok().unwrap_or(false) {
            if let Err(err) = remove_file(&file).await {
                error!("Failed to delete file {file}: {err}");
            }
        }
    }

    let files = state::get_all_files().await;

    add_to_repo(repo_name, &files);
}

fn add_to_repo(repo_name: &str, files: &Vec<String>) -> bool {
    let mut command = Command::new(REPO_ADD);
    command.current_dir(REPO_DIR);
    command.args([
        "--new",
        "--remove",
        "--prevent-downgrade",
        "--verify",
        &format!("{repo_name}.db.tar.zst"),
    ]);
    command.args(files);
    run_command(command)
}

fn remove_from_repo(repo_name: &str, files: &Vec<String>, packages: &Vec<Package>) -> bool {
    if !exists(PathBuf::new().join(REPO_DIR).join(format!("{repo_name}.db.tar.zst"))).unwrap_or(false) {
        return false;
    }

    let mut command = Command::new(REPO_REMOVE);
    command.current_dir(REPO_DIR);
    command.args([&format!("{repo_name}.db.tar.zst")]);
    command.args(packages);
    let command_result = run_command(command);

    let repo_dir = PathBuf::new().join(REPO_DIR);
    for file in files {
        if let Err(err) = std::fs::remove_file(repo_dir.join(file)) {
            error!("Failed to delete {file}: {err}");
        }
    }

    command_result
}

fn run_command(mut command: Command) -> bool {
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => {
            error!("Failed to spawn {REPO_ADD}: {err}");
            return false;
        }
    };

    if !output.status.success() {
        if let Some(exit_code) = output.status.code() {
            error!("{:?} failed with exit code {exit_code}", command.get_args());
        } else {
            error!("{:?} was terminated by a signal", command.get_args());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        error!("Stdout: {stdout}");
        error!("Stderr: {stderr}");
    }

    output.status.success()
}

#[derive(Debug, Error)]
enum Error {
    #[error("Encountered an IO error")]
    IO(#[from] std::io::Error),
}
