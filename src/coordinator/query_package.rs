use crate::messages::Package;
use crate::stop_token::StopToken;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::FileType;
use std::sync::LazyLock;
use std::time::Duration;
use tempfile::tempdir;
use thiserror::Error;
use tokio::fs::try_exists;
use tokio::sync::RwLock;
use tracing::{debug, error, Level};

const URL: &str = "https://aur.archlinux.org/rpc/v5/info?";
const ARG: &str = "arg[]=";

static PACKAGE_CACHE: LazyLock<RwLock<HashSet<Package>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

#[derive(Deserialize)]
struct AurRPC {
    results: Vec<PackageInfo>,
}

#[derive(Deserialize)]
struct PackageInfo {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "LastModified")]
    pub last_modified: i64,
    #[serde(rename = "Depends")]
    pub depends: HashSet<Package>,
}

#[derive(Clone)]
pub struct PackageData {
    pub name: Package,
    pub last_modified: i64,
    pub depends: HashSet<Package>,
}

// TODO: This is really ugly right now, but it will do
pub async fn check_pkgbuild<U: AsRef<str>>(url: U) -> Result<PackageData, Error> {
    let dir = tempdir()?;
    let path = dir.path().to_str().ok_or(Error::TempDirPath)?;

    debug!("Cloning git repository {}", url.as_ref());
    let output = tokio::process::Command::new("git")
        .args(["clone", url.as_ref(), path])
        .output()
        .await?;

    if !output.status.success() {
        return Err(Error::FailedToClone(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    if !try_exists(dir.path().join("PKGBUILD")).await? {
        return Err(Error::PkgbuildMissing);
    }

    debug!("Reading package build");
    let output = tokio::process::Command::new("bash")
        .current_dir(path)
        .args([
            "-c",
            r#"
            source PKGBUILD
            echo $pkgname
            echo "${depends[@]} ${makedepends[@]}"
            "#,
        ])
        .output()
        .await?;

    let cache = PACKAGE_CACHE.read().await;
    let output = String::from_utf8_lossy(&output.stdout);
    let mut lines = output.lines();
    let name = lines
        .next()
        .map(String::from)
        .ok_or(Error::PkgbuildNameMissing)?;
    let depends = lines
        .next()
        .map(|text| {
            text.split(' ')
                .filter_map(|pkg| {
                    if cache.contains(pkg) || pkg.contains(['<', '>', '=']) {
                        None
                    } else {
                        Some(pkg.to_string())
                    }
                })
                .collect::<HashSet<Package>>()
        })
        .unwrap_or_default();

    debug!("Fetching timestamp");
    let output = tokio::process::Command::new("git")
        .current_dir(path)
        .args(["show", "-s", "--format=%ct", "HEAD"])
        .output()
        .await?;

    let last_modified: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| Error::FailedToParseTimestamp)?;

    Ok(PackageData {
        name,
        last_modified,
        depends,
    })
}

pub async fn update_non_aur_packages(mut stop_token: StopToken) {
    loop {
        match run_pacman().await {
            Ok(out) => {
                let cache: HashSet<String> = String::from_utf8_lossy(&out)
                    .split('\n')
                    .map(String::from)
                    .collect();
                *PACKAGE_CACHE.write().await = cache;
                debug!("Updated package cache");
            }
            Err(err) => {
                error!("Failed to update cache: {err}");
            }
        }

        stop_token.sleep(Duration::from_secs(60 * 60)).await;
        if stop_token.stopped() {
            break;
        }
    }
}

async fn run_pacman() -> Result<Vec<u8>, Error> {
    tokio::process::Command::new("pacman")
        .arg("-Syy")
        .output()
        .await?;
    Ok(tokio::process::Command::new("pacman")
        .arg("-Slq")
        .output()
        .await?
        .stdout)
}

pub async fn get_last_modified<P, S>(packages: P) -> Result<HashMap<String, i64>, Error>
where
    P: IntoIterator<Item = S>,
    S: AsRef<str> + Display,
{
    let aur_data = get_package_info(packages).await?;

    let mut last_modified = HashMap::new();
    for pkg in aur_data {
        last_modified.insert(pkg.name, pkg.last_modified);
    }

    Ok(last_modified)
}

pub async fn do_packages_exist<P, S>(packages: P) -> Result<HashSet<Package>, Error>
where
    P: IntoIterator<Item = S>,
    S: AsRef<str> + Display,
{
    let aur_data = get_package_info(packages).await?;
    Ok(aur_data.into_iter().map(|info| info.name).collect())
}

pub async fn get_dependencies<P, S>(
    packages: P,
) -> Result<HashMap<Package, HashSet<Package>>, Error>
where
    P: IntoIterator<Item = S>,
    S: AsRef<str> + Display,
{
    let cache = PACKAGE_CACHE.read().await;
    let info = get_package_info(packages).await?;
    Ok(info
        .into_iter()
        .map(|info| {
            (
                info.name,
                info.depends
                    .into_iter()
                    .filter_map(|pkg| {
                        if cache.contains(&pkg) || pkg.contains(['<', '>', '=']) {
                            None
                        } else {
                            Some(pkg)
                        }
                    })
                    .collect(),
            )
        })
        .collect())
}

pub async fn get_package_info<P, S>(packages: P) -> Result<Vec<PackageInfo>, Error>
where
    P: IntoIterator<Item = S>,
    S: AsRef<str> + Display,
{
    let arguments = packages
        .into_iter()
        .map(|package| format!("{ARG}{package}"))
        .join("&");
    let url = format!("{URL}{arguments}");

    let response = reqwest::get(&url).await?.text().await?;
    let aur_data: AurRPC = serde_json::de::from_str(&response)?;
    Ok(aur_data.results)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to make a request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Deserialize error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to get path to temp dir")]
    TempDirPath,
    #[error("Could not find a PKGBUILD file")]
    PkgbuildMissing,
    #[error("Could not find a name in the PKGBUILD file")]
    PkgbuildNameMissing,
    #[error("Could not parse unix timestamp")]
    FailedToParseTimestamp,
    #[error("Failed to clone repository: {0}")]
    FailedToClone(String),
}
