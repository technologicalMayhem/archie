use crate::messages::Package;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use thiserror::Error;

const URL: &str = "https://aur.archlinux.org/rpc/v5/info?";
const ARG: &str = "arg[]=";

#[derive(Deserialize)]
struct AurRPC {
    results: Vec<PackageInfo>,
}

#[derive(Deserialize)]
struct PackageInfo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "LastModified")]
    last_modified: i64,
}

pub async fn get_last_modified<P, S>(packages: P) -> Result<HashMap<String, i64>, Error>
where
    P: IntoIterator<Item = S>,
    S: AsRef<str> + Display,
{
    let aur_data = get_package_info(packages).await?;

    let mut last_modified = HashMap::new();
    for pkg in aur_data.results {
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
    Ok(aur_data.results.into_iter().map(|info| info.name).collect())
}

async fn get_package_info<P, S>(packages: P) -> Result<AurRPC, Error>
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
    Ok(aur_data)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to make a request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Deserialize error: {0}")]
    Deserialize(#[from] serde_json::Error),
}
