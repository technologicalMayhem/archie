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
pub struct PackageInfo {
    pub url: Option<String>,
    pub is_dependency: bool,
    pub dependencies: HashSet<Package>,
    pub build: Option<Build>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Build {
    pub time: i64,
    pub files: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Persistent {
    pub package_status: HashMap<Package, PackageInfo>,
}

#[derive(Default)]
struct Ephemeral {
    active_containers: HashSet<String>,
}

#[derive(Clone)]
struct State {
    persistent: Arc<RwLock<Persistent>>,
    ephemeral: Arc<RwLock<Ephemeral>>
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
        ephemeral: Arc::new(RwLock::new(Ephemeral::default())),
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

pub async fn build_package(package: &Package, build_time: i64, files: Vec<String>) {
    let mut state = STATE.persistent.write().await;
    if let Some(status) = state.package_status.get_mut(package) {
        status.build = Some(Build {
            time: build_time,
            files,
        });
    }
    drop(state);
    save_state().await;
}

pub async fn track_package(package: Package, dependencies: HashSet<Package>, is_dependency: bool) {
    track_package_inner(package, None, dependencies, is_dependency).await;
}

pub async fn track_package_url(package: Package, url: String, dependencies: HashSet<Package>) {
    track_package_inner(package, Some(url), dependencies, false).await;
}

async fn track_package_inner(
    package: Package,
    url: Option<String>,
    dependencies: HashSet<Package>,
    is_dependency: bool,
) {
    let mut state = STATE.persistent.write().await;
    state.package_status.insert(
        package,
        PackageInfo {
            url,
            build: None,
            is_dependency,
            dependencies,
        },
    );
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

pub async fn tracked_packages_aur() -> HashSet<Package> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(package, status)| {
            if status.url.is_none() {
                Some(package.clone())
            } else {
                None
            }
        })
        .collect()
}

pub async fn tracked_packages_url() -> HashMap<Package, String> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(package, status)| {
            status
                .url
                .as_ref()
                .map(|url| (package.clone(), url.clone()))
        })
        .collect()
}

pub async fn get_build_url(package: &Package) -> Option<String> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .get(package)
        .map(|x| {
            x.url
                .clone()
                .unwrap_or(format!("https://aur.archlinux.org/{package}.git"))
        })
}

pub async fn are_dependencies_met(package: &Package) -> bool {
    let state = &STATE.persistent.read().await.package_status;
    state
        .get(package)
        .is_some_and(|x| {
            x.dependencies.iter().all(|dep| {
                state
                    .get(dep)
                    .map_or(false, |dep_info| dep_info.build.is_some())
            })
        })
}

async fn all_dependencies() -> HashSet<Package> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(pkg, info)| {
            if info.is_dependency {
                Some(pkg.clone())
            } else {
                None
            }
        })
        .collect()
}

async fn required_dependencies() -> HashSet<Package> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .values()
        .flat_map(|info| info.dependencies.clone())
        .collect()
}

pub async fn unneeded_dependencies() -> HashSet<Package> {
    let all_dependencies = all_dependencies().await;
    let required_dependencies = required_dependencies().await;

    all_dependencies
        .difference(&required_dependencies)
        .map(String::clone)
        .collect()
}

pub async fn get_build_times(packages: &HashSet<Package>) -> HashMap<Package, i64> {
    STATE
        .persistent
        .read()
        .await
        .package_status
        .iter()
        .filter_map(|(pkg, info)| {
            if packages.contains(pkg) {
                if let Some(a) = info.build.as_ref().map(|x| x.time) {
                    return Some((pkg.to_string(), a));
                }
            }
            None
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
                status.build.as_ref().map(|status| status.files.clone())
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
        .filter_map(|(_, info)| info.build.as_ref().map(|status| status.files.clone()))
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

pub async fn add_running_container(id: String) {
    STATE.ephemeral.write().await.active_containers.insert(id);
}

pub async fn remove_running_container(id: &str) {
    STATE.ephemeral.write().await.active_containers.remove(id);
}

pub async fn is_container_running(id: &str) -> bool {
    STATE.ephemeral.read().await.active_containers.contains(id)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("IO error: {0}")]
    Deserialize(#[from] serde_json::Error),
}
