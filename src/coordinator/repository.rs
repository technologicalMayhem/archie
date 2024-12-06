use crate::stop_token::StopToken;
use crate::{config, state};
use coordinator::Artifacts;
use std::process::Command;
use thiserror::Error;
use tokio::fs::{remove_file, try_exists, write};
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tracing::error;
use tracing::log::info;

pub const REPO_DIR: &str = "/output/";
const REPO_ADD: &str = "repo-add";

pub async fn start(artifacts_receive: Receiver<Artifacts>, stop_token: StopToken) {
    if let Err(err) = run_repository(artifacts_receive, stop_token).await {
        error!("Stopped with an error: {err}");
    }
}

async fn run_repository(
    mut artifacts_receive: Receiver<Artifacts>,
    mut stop_token: StopToken,
) -> Result<(), Error> {
    info!("Starting");
    let repo_name = config::repo_name();

    recreate_repo(&repo_name).await;

    loop {
        let artifact = select! {
            work = artifacts_receive.recv() => Some(work),
            () = stop_token.wait() => None,
        };
        let Some(Ok(artifacts)) = artifact else {
            break;
        };

        info!("Successfully built {}", artifacts.package_name);
        let mut file_names = Vec::new();
        for (name, data) in artifacts.files {
            write(format!("{REPO_DIR}{name}"), data).await?;
            file_names.push(name);
        }

        state::lock_repo().await;
        if add_to_repo(&repo_name, &file_names) {
            state::build_package(&artifacts.package_name, artifacts.build_time, file_names).await;
        }
        state::unlock_repo().await;
    }

    info!("Stopped repository");
    Ok(())
}

async fn recreate_repo(repo_name: &str) {
    state::lock_repo().await;

    info!("Recreating repository");

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

    state::unlock_repo().await;
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
    let result = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            error!("Failed to spawn {REPO_ADD}: {err}");
            return false;
        }
    };

    let output = match result.wait_with_output() {
        Ok(output) => output,
        Err(err) => {
            error!("An error occurred whilst waiting for {REPO_ADD} to exit: {err}");
            return false;
        }
    };

    if !output.status.success() {
        if let Some(exit_code) = output.status.code() {
            error!("{REPO_ADD} failed with exit code {exit_code}",);
        } else {
            error!("{REPO_ADD} was terminated by a signal");
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
