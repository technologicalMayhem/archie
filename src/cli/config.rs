use coordinator::endpoints::Endpoints;
use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;
use thiserror::Error;
use tracing::error;

const CONFIG_DIR: &str = ".config/archie";
const CONFIG_FILE: &str = "config.toml";

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    /// Has config init been run?
    pub initialized: bool,
    /// Options relating to the server to communicate with
    pub server: Server,
}

/// Settings relating to the server
#[derive(Serialize, Deserialize)]
pub struct Server {
    /// Address of the server
    pub address: String,
    /// Port the server is using
    pub port: u16,
    /// Use HTTP instead of HTTPS
    pub https: bool,
}

impl Server {
    pub fn to_endpoints(&self) -> Endpoints {
        Endpoints {
            address: self.address.clone(),
            port: self.port,
            https: self.https,
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: 3200,
            address: "localhost".to_string(),
            https: false,
        }
    }
}

fn config_dir() -> PathBuf {
    let Some(home_dir) = home::home_dir() else {
        error!("Could not locate home directory");
        std::process::exit(1);
    };
    home_dir.join(CONFIG_DIR)
}

fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILE)
}

pub fn load() -> Config {
    let figment = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(config_path()))
        .merge(Env::prefixed("ARCHIE_"))
        .extract::<Config>();
    match figment {
        Ok(config) => config,
        Err(err) => {
            error!("Encountered an error whilst retrieving app config: {err}");
            std::process::exit(1);
        }
    }
}

pub fn save(config: &Config) -> Result<(), Error> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .create(config_dir())?;
    std::fs::write(config_path(), toml::ser::to_string_pretty(&config)?)?;
    Ok(())
}

pub fn init(config: &mut Config) -> Result<(), Error> {
    let stdin = stdin();
    let mut stdout = stdout();

    println!("This seems to be the first running archie. Let's set thing up!");
    println!("What is the address of the coordinator?");
    print!("Address [{}]: ", config.server.address);
    stdout.flush()?;
    let mut buffer = String::new();
    stdin.read_line(&mut buffer)?;
    let buffer = buffer.trim_end();
    if !buffer.is_empty() {
        config.server.address = buffer.to_string();
    }

    println!("What is the port of the coordinator?");
    loop {
        print!("Port [{}]: ", config.server.port);
        stdout.flush()?;
        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;
        let buffer = buffer.trim_end();
        if buffer.is_empty() {
            break;
        }
        match buffer.parse::<u16>() {
            Ok(port) => {
                config.server.port = port;
                break;
            }
            Err(_) => println!("Not a valid port number."),
        }
    }

    println!("Use HTTPS?");
    loop {
        let current = if config.server.https { "Y" } else { "N" };
        print!("Y/N [{current}]: ");
        stdout.flush()?;
        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;
        let buffer = buffer.trim_end();
        if buffer.is_empty() {
            break
        }
        match buffer.chars().next() {
            Some('N' | 'n') => {
                config.server.https = false;
                break;
            }
            Some('Y' | 'y') => {
                config.server.https = true;
                break;
            }
            _ => println!("Please provide either Y (Yes) or N (No) as an answer."),
        }
    }

    config.initialized = true;
    save(config)?;
    println!("Setup complete!");

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}
