use crate::messages::Package;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{exists, read_to_string};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio::fs::write;
use tokio::sync::RwLock;
use tracing::error;

const STATE_FILE: &str = "/config/state.json";
static STATE: LazyLock<State> = LazyLock::new(|| match load_state() {
    Ok(state) => state,
    Err(err) => {
        error!("Failed to load application state: {err}");
        std::process::exit(2);
    }
});

#[derive(Serialize, Deserialize, Clone)]
pub struct PackageStatus {
    pub last_build: i64,
    pub last_check: i64,
    pub files: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Persistent {
    pub package_status: HashMap<Package, Option<PackageStatus>>,
}

#[derive(Default)]
struct Ephermal {
    repository_locked: bool,
}

#[derive(Clone)]
struct State {
    persistent: Arc<RwLock<Persistent>>,
    ephemeral: Arc<RwLock<Ephermal>>,
}

fn load_state() -> Result<State, Error> {
    let persistent = if exists(STATE_FILE)? {
        serde_json::de::from_str(&read_to_string(STATE_FILE)?)?
    } else {
        Persistent {
            package_status: HashMap::new(),
        }
    };

    Ok(State {
        persistent: Arc::new(RwLock::new(persistent)),
        ephemeral: Arc::new(RwLock::new(Ephermal::default())),
    })
}

async fn save_state() {
    let state = STATE.persistent.read().await;
    let Ok(serialized) = serde_json::ser::to_vec(&*state) else {
        error!("Failed to serialize state file.");
        return;
    };
    if let Err(err) = write(STATE_FILE, serialized).await {
        error!("Encountered an error whilst writing state file: {err}");
    }
}

pub async fn build_package(package_name: &str, build_time: i64, files: Vec<String>) {
    let mut state = STATE.persistent.write().await;
    state.package_status.insert(
        package_name.to_string(),
        Some(PackageStatus {
            last_build: build_time,
            last_check: build_time,
            files,
        }),
    );
    drop(state);
    save_state().await;
}

pub async fn track_package(package: &Package) {
    let mut state = STATE.persistent.write().await;
    state.package_status.insert(package.to_string(), None);
    drop(state);
    save_state().await;
}

pub async fn tracked_packages() -> HashSet<Package> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .keys()
        .map(String::clone)
        .collect()
}

pub async fn get_last_check(package_name: &str) -> Option<i64> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .get(package_name)
        .and_then(|pkg| pkg.as_ref().map(|pkg| pkg.last_check))
}

pub async fn set_last_check(package_name: &str, last_check: i64) {
    if let Some(Some(status)) = STATE
        .persistent
        .write()
        .await
        .package_status
        .get_mut(package_name)
    {
        status.last_check = last_check;
    }

    save_state().await;
}

pub async fn get_build_times(packages: &[String]) -> Vec<(&str, i64)> {
    let states = &STATE.persistent.read().await.package_status;
    packages
        .iter()
        .filter_map(|pkg| {
            states
                .get(pkg)
                .and_then(Option::as_ref)
                .map(|status| (pkg.as_str(), status.last_build))
        })
        .collect()
}

pub async fn get_files(package: &Package) -> Vec<String> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(name, status)| {
            if name == package {
                status.as_ref().map(|status| status.files.clone())
            } else {
                None
            }
        })
        .flatten()
        .collect()
}

pub async fn get_all_files() -> Vec<String> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(_, status)| status.as_ref().map(|status| status.files.clone()))
        .flatten()
        .collect()
}

pub async fn is_package_tracked(package: &Package) -> bool {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .contains_key(package)
}

pub async fn remove_packages(package: &HashSet<Package>) {
    let mut persistent = STATE.persistent.write().await;

    for package in package {
        persistent.package_status.remove(package);
    }

    drop(persistent);
    save_state().await;
}

pub async fn lock_repo() {
    STATE.ephemeral.write().await.repository_locked = true;
}

pub async fn unlock_repo() {
    STATE.ephemeral.write().await.repository_locked = false;
}

pub async fn is_repo_locked() -> bool {
    STATE.ephemeral.read().await.repository_locked
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("IO error: {0}")]
    Deserialize(#[from] serde_json::Error),
}
