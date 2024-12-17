#![warn(clippy::pedantic)]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::str::FromStr;
use itertools::Itertools;
use tracing::info;

pub mod endpoints;

const VERSION: &str = env!("APP_VERSION");

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ForceRebuild {
    pub packages: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ForceRebuildResponse {
    pub not_found: HashSet<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddPackageUrl {
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AddPackageUrlResponse {
    Ok(String),
    AlreadyAdded(String),
    Error(String),
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

pub fn env_or_none<T>(var: &str) -> Option<T>
where
    T: FromStr,
{
    std::env::var(var)
        .ok()
        .and_then(|val| val.parse::<T>().ok())
}

pub fn print_version() {
    info!("Version built from {VERSION}");
}

pub fn combine_for_display<S, I>(list: S) -> String
where
    S: IntoIterator<Item = I>,
    I: AsRef<str> + Display,
{
    let list = list.into_iter().collect::<Vec<I>>();
    match list.len() {
        0 => String::new(),
        1 => list[0].to_string(),
        2 => format!("{} and {}", list[0], list[1]),
        _ => {
            let all_but_last = &list[..list.len() - 1];
            let last_part = list.last().unwrap();
            let all_but_last = all_but_last.iter().join(", ");
            format!("{all_but_last} and {last_part}")
        }
    }
}