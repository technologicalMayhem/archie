use crate::env_or;
use std::str::FromStr;
use std::sync::LazyLock;
use tracing::info;

static CONFIG: LazyLock<Config> = LazyLock::new(load);

fn base() -> String {
    format!("{}{}:{}/", protocol(), address(), port())
}

fn url(path: &str) -> String {
    let mut base = base();
    base.push_str(path);
    base
}

pub fn work() -> String {
    url("work")
}

pub fn artifacts() -> String {
    url("artifacts")
}

#[derive(Debug)]
struct Config {
    address: String,
    port: u32,
    https: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3200,
            address: String::new(),
            https: true,
        }
    }
}

fn load() -> Config {
    let config = load_from_env();
    info!("Loaded config: {config:#?}");
    config
}

fn load_from_env() -> Config {
    let default = Config::default();

    Config {
        address: env_or("ADDRESS", default.address),
        port: env_or("PORT", default.port),
        https: env_or("HTTPS", default.https),
    }
}

pub fn protocol() -> &'static str {
    match CONFIG.https {
        true => "https://",
        false => "http://",
    }
}

pub fn port() -> u32 {
    CONFIG.port
}

pub fn address() -> String {
    CONFIG.address.clone()
}
