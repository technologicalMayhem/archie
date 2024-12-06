use crate::repository::REPO_DIR;
use crate::state::track_package;
use crate::stop_token::StopToken;
use crate::{config, state};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use coordinator::{Artifacts, Status, WorkAssignment, WorkOrders};
use futures::future::join_all;
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::task::spawn;
use tower_http::services::ServeDir;
use tracing::error;
use tracing::log::info;

static WORK: Mutex<Vec<WorkAssignment>> = Mutex::new(Vec::new());

pub async fn start(
    mut work_receiver: Receiver<WorkAssignment>,
    work_sender: Sender<WorkAssignment>,
    artifacts_sender: Sender<Artifacts>,
    mut stop_token: StopToken,
) {
    let mut worker_token = stop_token.child();

    let web = spawn(async move {
        let router = Router::new()
            .route("/status", get(status))
            .route("/packages/add", post(add_package))
            .route("/packages/remove", post(remove_package))
            .route("/work", post(receive_work))
            .with_state(work_sender)
            .route(
                "/artifacts",
                post(receive_artifacts).layer(DefaultBodyLimit::disable()),
            )
            .with_state(artifacts_sender)
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

    let worker = spawn(async move {
        loop {
            let work = select! {
                work = work_receiver.recv() => Some(work),
                () = worker_token.wait() => None,
            };

            if let Some(Ok(work)) = work {
                let mut list = WORK.lock().expect("Failed to acquire work lock");
                list.push(work);
            } else {
                break;
            }
        }
    });

    join_all(vec![web, worker]).await;
    info!("Stopped web server");
}

async fn add_package(headers: HeaderMap) -> Result<Json<WorkAssignment>, StatusCode> {
    if let Some(work) = WORK.lock().expect("Could not acquire work lock").pop() {
        let name = headers
            .get("hostname")
            .and_then(|val| val.to_str().ok())
            .unwrap_or("Unknown");
        info!("Handing out work ({}) to {name}", work.package);
        Ok(Json(work))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn receive_work(
    work_sender: State<Sender<WorkAssignment>>,
    Json(work_orders): Json<WorkOrders>,
) -> Result<(), StatusCode> {
    for assignment in work_orders.packages {
        track_package(&assignment.package).await;
        info!("Received assigment to build {}", assignment.package);
        if let Err(err) = work_sender.send(assignment) {
            error!("An error occurred when receiving work: {err}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(())
}

async fn receive_artifacts(
    State(artifacts): State<Sender<Artifacts>>,
    Json(data): Json<Artifacts>,
) {
    info!(
        "Got artifacts for {}. Received {} files.",
        data.package_name,
        data.files.len()
    );

    artifacts.send(data).expect("Could not send artifact.");
}

async fn remove_package() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

async fn status() -> Json<Status> {
    Json(Status {
        packages: state::packages().await,
    })
}
