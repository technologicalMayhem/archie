mod config;

use coordinator::{abort_if_not_in_docker, Artifacts, WorkAssignment};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::fs::{create_dir_all, exists, read_to_string, remove_dir_all};
use std::time::Duration;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::process::Command;
use tracing::{info, log, Level};
use coordinator::endpoints::Endpoints;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    abort_if_not_in_docker();
    tracing_subscriber::fmt().with_max_level(Level::DEBUG).init();

    let mut sleeping = false;
    let mut headers = HeaderMap::new();
    let hostname = read_to_string("/etc/hostname")?.replace('\n', "");
    info!("Hostname: {hostname}");
    headers.insert("hostname", HeaderValue::from_str(&hostname)?);
    let client = reqwest::Client::builder().default_headers(headers).build()?;
    let endpoints = Endpoints {
        address: "172.17.0.1".to_string(),
        https: false,
        ..Default::default()
    };

    let work_assignment: WorkAssignment = loop {
        let response = client.get(endpoints.work()).send().await?;
        if response.status() == StatusCode::NOT_FOUND {
            if !sleeping {
                log::info!("No work available. Waiting.");
                sleeping = true;
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        }
        break response.json().await.expect("Could not read body");
    };

    log::info!("Building {}", work_assignment.package);
    let artifacts = build_pkg(work_assignment.package).await?;

    let response = client
        .post(endpoints.artifacts())
        .json(&artifacts)
        .send()
        .await?;

    log::info!("Sent off artifacts. Got back a {}", response.status());
    Ok(())
}

async fn build_pkg(package_name: String) -> Result<Artifacts, AppError> {
    if exists("/home/worker/build")? {
        remove_dir_all("/home/worker/build")?;
    }
    create_dir_all("/home/worker/build")?;

    let build_time = OffsetDateTime::now_utc().unix_timestamp();

    run_command("paru", &["-Sy"]).await?;
    run_command("paru", &["-G", &package_name]).await?;
    run_command(
        "paru",
        &[
            "-B",
            "--nouseask",
            "--skipreview",
            "--noupgrademenu",
            "--failfast",
            &package_name,
        ],
    )
    .await?;

    let mut dir = tokio::fs::read_dir(format!("/home/worker/build/{package_name}")).await?;
    let mut files = HashMap::new();
    while let Some(entry) = dir.next_entry().await? {
        if entry.file_type().await?.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .ends_with(".pkg.tar.zst")
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let data = tokio::fs::read(entry.path()).await?;

            log::info!("File: {name}");

            files.insert(name, data);
        }
    }

    Ok(Artifacts {
        package_name,
        build_time,
        files,
    })
}

async fn run_command(app: &str, args: &[&str]) -> Result<(), AppError> {
    let output = Command::new(app)
        .current_dir("/home/worker/build")
        .args(args)
        .output()
        .await
        .expect("Failed to execute command");

    if !output.status.success() {
        log::error!(
            "Command {app} {} did not exist successfully.\nStdout:\n{}\n\nStderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        return Err(AppError::ProcessFailed);
    }

    Ok(())
}

#[derive(Debug, Error)]
enum AppError {
    #[error("Failed to make a request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Deserialize error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("Failed to run process")]
    ProcessFailed,
    #[error("Invalid header value: {0}")]
    Header(#[from] reqwest::header::InvalidHeaderValue),
}
