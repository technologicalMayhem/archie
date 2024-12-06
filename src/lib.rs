use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

pub mod endpoints;

pub fn abort_if_not_in_docker() {
    if !std::fs::exists("/.dockerenv").unwrap_or(false) {
        eprintln!("We are not inside a docker container. Aborting!");
        std::process::exit(1);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkOrders {
    pub packages: Vec<WorkAssignment>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkAssignment {
    pub package: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Artifacts {
    pub package_name: String,
    pub build_time: i64,
    pub files: HashMap<String, Vec<u8>>,
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