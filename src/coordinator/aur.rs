use crate::Error;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashMap;

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

pub async fn get_last_modified_from_aur(
    packages: &[String],
) -> Result<HashMap<String, i64>, Error> {
    let arguments = packages
        .iter()
        .map(|package| format!("{ARG}{package}"))
        .join("&");
    let url = format!("{URL}{arguments}");

    let response = reqwest::get(&url).await?.text().await?;
    let aur_data: AurRPC = serde_json::de::from_str(&response)?;

    let mut build_date = HashMap::with_capacity(packages.len());
    for pkg in aur_data.results {
        build_date.insert(pkg.name, pkg.last_modified);
    }

    Ok(build_date)
}
