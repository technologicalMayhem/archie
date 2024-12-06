use crate::messages::{Message, Package};
use crate::repository::REPO_DIR;
use crate::stop_token::StopToken;
use crate::{config, state};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use coordinator::{Artifacts, RemovePackages, Status, WorkAssignment, WorkOrders};
use futures::future::join_all;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::task::spawn;
use tower_http::services::ServeDir;
use tracing::error;
use tracing::log::info;

static WORK: Mutex<Vec<Package>> = Mutex::new(Vec::new());

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

pub async fn start(
    mut receiver: Receiver<Message>,
    sender: Sender<Message>,
    mut stop_token: StopToken,
) {
    let mut worker_token = stop_token.child();

    let web = spawn(async move {
        let state = RequestState { sender };
        let router = Router::new()
            .route("/status", get(status))
            .route("/packages/add", post(hand_out_work))
            .route("/packages/remove", post(remove_package))
            .route("/work", post(add_package))
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
    });

    let message_handler = spawn(async move {
        loop {
            let message = select! {
                work = receiver.recv() => Some(work),
                () = worker_token.wait() => None,
            };

            if let Some(Ok(Message::BuildPackage(package))) = message {
                let mut list = WORK.lock().expect("Failed to acquire work lock");
                list.push(package);
            } else {
                break;
            }
        }
    });

    join_all(vec![web, message_handler]).await;
    info!("Stopped web server");
}

async fn hand_out_work(
    state: State<RequestState>,
    headers: HeaderMap,
) -> Result<Json<WorkAssignment>, StatusCode> {
    if let Some(package) = WORK.lock().expect("Could not acquire work lock").pop() {
        let Some(Ok(worker)) = headers.get("hostname").map(HeaderValue::to_str) else {
            return Err(StatusCode::BAD_REQUEST);
        };
        info!("Handing out work ({package}) to {worker}");
        state.send_message(Message::AcceptedWork {
            package: package.to_string(),
            worker: worker.to_string(),
        })?;

        Ok(Json(WorkAssignment { package }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
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
