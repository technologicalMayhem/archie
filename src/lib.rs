use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

pub mod endpoints;

pub fn abort_if_not_in_docker() {
    if !std::fs::exists("/.dockerenv").unwrap_or(false) {
        eprintln!("We are not inside a docker container. Aborting!");
        std::process::exit(1);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddPackages {
    pub packages: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddPackagesResponse {
    pub added: HashSet<String>,
    pub already_tracked: HashSet<String>,
    pub not_found: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Artifacts {
    pub package_name: String,
    pub build_time: i64,
    pub files: HashMap<String, Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub packages: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemovePackages {
    pub packages: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemovePackagesResponse {
    pub removed: HashSet<String>,
    pub not_tracked: HashSet<String>,
}

pub fn env_or<T>(var: &str, or: T) -> T
where
    T: FromStr,
{
    std::env::var(var)
        .ok()
        .and_then(|val| val.parse::<T>().ok())
        .unwrap_or(or)
}
