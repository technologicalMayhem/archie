use coordinator::env_or;
use std::sync::LazyLock;
use tracing::info;

static CONFIG: LazyLock<Config> = LazyLock::new(load);

#[derive(Debug)]
struct Config {
    max_builders: usize,
    max_retries: u8,
    port: u32,
    image: String,
    repo_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_builders: 1,
            max_retries: 3,
            port: 3200,
            image: "aur_worker".to_string(),
            repo_name: "aur".to_string(),
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
        port: env_or("PORT", default.port),
        image: env_or("BUILDER_IMAGE", default.image),
        repo_name: env_or("REPO_NAME", default.repo_name),
    }
}

pub fn max_builders() -> usize {
    CONFIG.max_builders
}

pub fn max_retries() -> u8 {
    CONFIG.max_retries
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
