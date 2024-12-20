use crate::messages::Message;
use crate::query_package::{Error, PackageData};
use crate::repository::REPO_DIR;
use crate::stop_token::StopToken;
use crate::{config, query_package, state, SSH_KEY_PATH};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use coordinator::{
    AddPackageUrl, AddPackageUrlResponse, AddPackages, AddPackagesResponse, Artifacts,
    ForceRebuild, ForceRebuildResponse, RemovePackages, RemovePackagesResponse, Status,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs::read;
use tokio::net::TcpListener;
use tokio::sync::broadcast::Sender;
use tower_http::services::ServeDir;
use tracing::log::info;
use tracing::{debug, error};

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
        .route("/packages/add-url", post(add_package_url))
        .route("/packages/remove", post(remove_package))
        .route("/packages/rebuild", post(force_rebuild))
        .route(
            "/artifacts",
            post(receive_artifacts).layer(DefaultBodyLimit::disable()),
        )
        .route("/key", get(get_key))
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
    Json(add): Json<AddPackages>,
) -> Result<Json<AddPackagesResponse>, StatusCode> {
    let package_info = query_package::do_packages_exist(&add.packages)
        .await
        .map_err(|err| {
            error!("Failed to get packages from the AUR: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let tracked_packages = state::tracked_packages().await;

    let not_found: HashSet<String> = add
        .packages
        .difference(&package_info)
        .map(String::to_owned)
        .collect();
    let already_tracked: HashSet<String> = tracked_packages
        .intersection(&add.packages)
        .map(String::to_owned)
        .collect();
    let to_be_added: HashSet<String> = add
        .packages
        .difference(&tracked_packages)
        .map(String::to_owned)
        .collect();

    if !to_be_added.is_empty() {
        state.send_message(Message::AddPackages(to_be_added.clone()))?;
    }

    Ok(Json(AddPackagesResponse {
        added: to_be_added,
        not_found,
        already_tracked,
    }))
}

async fn add_package_url(
    state: State<RequestState>,
    Json(add): Json<AddPackageUrl>,
) -> Result<Json<AddPackageUrlResponse>, StatusCode> {
    match query_package::check_pkgbuild(&add.url).await {
        Ok(data) => {
            if state::is_package_tracked(&data.name).await {
                Ok(Json(AddPackageUrlResponse::AlreadyAdded(data.name)))
            } else {
                let name = data.name.clone();
                state.send_message(Message::AddPackageUrl { url: add.url, data })?;
                Ok(Json(AddPackageUrlResponse::Ok(name)))
            }
        }
        Err(err) => Ok(Json(AddPackageUrlResponse::Error(err.to_string()))),
    }
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

    debug!(
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
) -> Result<Json<RemovePackagesResponse>, StatusCode> {
    let tracked_packages = state::tracked_packages().await;
    let not_tracked: HashSet<String> = remove
        .packages
        .difference(&tracked_packages)
        .map(String::to_owned)
        .collect();

    let to_be_removed: HashSet<String> = tracked_packages
        .intersection(&remove.packages)
        .map(String::to_owned)
        .collect();

    if !to_be_removed.is_empty() {
        state.send_message(Message::RemovePackages(to_be_removed.clone()))?;
    }

    Ok(Json(RemovePackagesResponse {
        removed: to_be_removed,
        not_tracked,
    }))
}

async fn force_rebuild(
    state: State<RequestState>,
    Json(rebuild): Json<ForceRebuild>,
) -> Result<Json<ForceRebuildResponse>, StatusCode> {
    let tracked_packages = state::tracked_packages().await;
    let not_found: HashSet<String> = rebuild
        .packages
        .difference(&tracked_packages)
        .map(String::to_owned)
        .collect();

    if not_found.is_empty() {
        for package in rebuild.packages {
            info!("User requested rebuild for {package}");
            state.send_message(Message::BuildPackage(package))?;
        }
    }

    Ok(Json(ForceRebuildResponse { not_found }))
}

async fn get_key(headers: HeaderMap) -> Result<Vec<u8>, StatusCode> {
    let Some(hostname) = headers
        .get("hostname")
        .and_then(|header| header.to_str().ok())
    else {
        return Err(StatusCode::BAD_REQUEST);
    };

    if !state::is_container_running(hostname).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    read(SSH_KEY_PATH)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn status() -> Json<Status> {
    Json(Status {
        packages: state::tracked_packages().await,
    })
}

fn sanitize_filename(file_name: &str) -> String {
    Path::new(file_name)
        .file_name()
        .unwrap_or_else(|| "default".as_ref())
        .to_string_lossy()
        .to_string()
}
