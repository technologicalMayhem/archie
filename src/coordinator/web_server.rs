use crate::messages::{Message, Package};
use crate::repository::REPO_DIR;
use crate::stop_token::StopToken;
use crate::{config, state};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use coordinator::{Artifacts, RemovePackages, Status, WorkOrders};
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;
use tokio::sync::broadcast::Sender;
use tower_http::services::ServeDir;
use tracing::error;
use tracing::log::info;

#[derive(Clone)]
struct RequestState {
    sender: Sender<Message>,
}

impl RequestState {
    fn send_message(&self, message: Message) -> Result<(), StatusCode> {
        if let Err(err) = self.sender.send(message) {
            error!("Failed to send message: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            Ok(())
        }
    }
}

pub async fn start(sender: Sender<Message>, mut stop_token: StopToken) {
    let state = RequestState { sender };
    let router = Router::new()
        .route("/status", get(status))
        .route("/packages/add", post(add_package))
        .route("/packages/remove", post(remove_package))
        .route(
            "/artifacts",
            post(receive_artifacts).layer(DefaultBodyLimit::disable()),
        )
        .with_state(state)
        .nest_service("/repo", ServeDir::new(REPO_DIR));

    let port = config::port();
    info!("Starting web server on port {port}");
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    let server_result = axum::serve(listener, router)
        .with_graceful_shutdown(async move { stop_token.wait().await })
        .await;
    if let Err(err) = server_result {
        error!("Web server exited with error: {err}");
    }

    info!("Stopped web server");
}

async fn add_package(
    state: State<RequestState>,
    Json(work): Json<WorkOrders>,
) -> Result<(), StatusCode> {
    state.send_message(Message::AddPackages(
        work.packages
            .into_iter()
            .map(|pkg| pkg.package as Package)
            .collect(),
    ))
}

async fn receive_artifacts(
    state: State<RequestState>,
    Json(data): Json<Artifacts>,
) -> Result<(), StatusCode> {
    let mut files = Vec::new();
    for (name, data) in &data.files {
        let file_name = sanitize_filename(name);
        if let Err(err) = tokio::fs::write(
            PathBuf::new().join(REPO_DIR).join(sanitize_filename(name)),
            data,
        )
        .await
        {
            error!("Failed to write artifact to disk: {err}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        files.push(file_name);
    }

    info!(
        "Got artifacts for {}. Received {} files.",
        data.package_name,
        data.files.len()
    );

    state.send_message(Message::ArtifactsUploaded {
        package: data.package_name,
        files,
        build_time: data.build_time,
    })
}

async fn remove_package(
    state: State<RequestState>,
    Json(remove): Json<RemovePackages>,
) -> Result<(), StatusCode> {
    state.send_message(Message::RemovePackages(remove.packages))
}

async fn status() -> Json<Status> {
    Json(Status {
        packages: state::packages().await,
    })
}

fn sanitize_filename(file_name: &str) -> String {
    Path::new(file_name)
        .file_name()
        .unwrap_or_else(|| "default".as_ref())
        .to_string_lossy()
        .to_string()
}
