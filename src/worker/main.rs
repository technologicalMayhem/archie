use coordinator::endpoints::Endpoints;
use coordinator::{abort_if_not_in_docker, print_version, Artifacts};
use reqwest::header::{HeaderMap, HeaderValue};
use std::collections::HashMap;
use std::fs::{create_dir_all, exists, read_to_string, remove_dir_all};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::process::Command;
use tracing::{error, info, log, Level};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    abort_if_not_in_docker();
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    print_version();

    let mut headers = HeaderMap::new();
    let hostname = read_to_string("/etc/hostname")?.replace('\n', "");
    info!("Hostname: {hostname}");
    headers.insert("hostname", HeaderValue::from_str(&hostname)?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let endpoints = Endpoints {
        address: "172.17.0.1".to_string(),
        https: false,
        ..Default::default()
    };

    let Ok(package) = std::env::var("PACKAGE") else {
        error!("Failed to read environment variable 'PACKAGE'");
        std::process::exit(1);
    };

    log::info!("Building {}", package);
    let artifacts = build_pkg(package).await?;

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
        .spawn()?
        .wait_with_output()
        .await
        .expect("Failed to execute command");

    if !output.status.success() {
        log::error!("Command {app} did not exit successfully");
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
