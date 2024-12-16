use coordinator::{env_or, env_or_none};
use std::sync::LazyLock;
use tracing::info;

static CONFIG: LazyLock<Config> = LazyLock::new(load);

#[derive(Debug)]
struct Config {
    max_builders: usize,
    max_retries: u8,
    update_check_interval: u32,
    port: u32,
    image: String,
    repo_name: String,
    memory_limit: Option<i64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_builders: 1,
            max_retries: 3,
            update_check_interval: 240,
            port: 3200,
            image: "aur_worker".to_string(),
            repo_name: "aur".to_string(),
            memory_limit: None,
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
        max_builders: env_or("MAX_BUILDERS", default.max_builders),
        max_retries: env_or("MAX_RETRIES", default.max_retries),
        update_check_interval: env_or("UPDATE_CHECK_INTERVAL", default.update_check_interval),
        port: env_or("PORT", default.port),
        image: env_or("BUILDER_IMAGE", default.image),
        repo_name: env_or("REPO_NAME", default.repo_name),
        memory_limit: env_or_none("MEMORY_LIMIT"),
    }
}

pub fn max_builders() -> usize {
    CONFIG.max_builders
}

pub fn max_retries() -> u8 {
    CONFIG.max_retries
}

pub fn update_check_interval() -> u32 {
    CONFIG.update_check_interval
}

pub fn port() -> u32 {
    CONFIG.port
}

pub fn image() -> String {
    CONFIG.image.clone()
}

pub fn repo_name() -> String {
    CONFIG.repo_name.clone()
}

pub fn max_memory() -> Option<i64> { CONFIG.memory_limit }