use crate::config::Config;
use crate::util::wrap_text;
use crate::Error;
use clap::Args;
use colored::Colorize;
use coordinator::endpoints::Endpoints;
use coordinator::{combine_for_display, ForceRebuild, ForceRebuildResponse};
use coordinator::{
    AddPackages, AddPackagesResponse, RemovePackages, RemovePackagesResponse, Status,
};
use std::fs::read_to_string;
use tracing::{error, info, warn};
use ureq::Agent;

#[derive(Clone, Args)]
pub struct Add {
    /// The packages to add
    packages: Vec<String>,
}

pub fn add(config: &Config, add: Add) -> Result<u8, Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    if add.packages.is_empty() {
        error!("No packages to build were given.");
        return Ok(1);
    }

    let add_packages = AddPackages {
        packages: add.packages.into_iter().collect(),
    };
    let response: AddPackagesResponse = client
        .post(&endpoints.add_packages())
        .send_json(add_packages)
        .map_err(Box::new)?
        .into_json()?;

    if !response.already_tracked.is_empty() {
        let is_are = if response.already_tracked.len() > 1 {
            "are"
        } else {
            "is"
        };
        warn!(
            "{} {is_are} already tracked",
            combine_for_display(&response.already_tracked)
        );
    }
    if !response.not_found.is_empty() {
        error!(
            "Could not find {}",
            combine_for_display(&response.not_found)
        );
    }
    if response.added.is_empty() || !response.not_found.is_empty() {
        error!("No changes have been made");
        return Ok(1);
    }
    info!(
        "Added {} successfully",
        combine_for_display(&response.added)
    );
    Ok(0)
}

#[derive(Clone, Args)]
pub struct Remove {
    /// The packages to remove
    packages: Vec<String>,
}

pub fn remove(config: &Config, remove: Remove) -> Result<u8, Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    if remove.packages.is_empty() {
        error!("No packages to remove were given.");
        return Ok(1);
    }

    let remove = RemovePackages {
        packages: remove.packages.into_iter().collect(),
    };

    let response: RemovePackagesResponse = client
        .post(&endpoints.remove_packages())
        .send_json(remove)
        .map_err(Box::new)?
        .into_json()?;

    if !response.not_tracked.is_empty() {
        let were_was = if response.not_tracked.len() > 1 {
            "were"
        } else {
            "was"
        };
        warn!(
            "{} {were_was} never tracked",
            combine_for_display(&response.not_tracked)
        );
    }
    if response.removed.is_empty() {
        error!("No changes have been made");
        Ok(1)
    } else {
        info!("Removed {}", combine_for_display(&response.removed));
        Ok(0)
    }
}

#[derive(Clone, Args)]
pub struct Rebuild {
    /// The packages to remove
    packages: Vec<String>,
}

pub fn rebuild(config: &Config, rebuild: Rebuild) -> Result<u8, Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    if rebuild.packages.is_empty() {
        error!("No packages to remove were given.");
        return Ok(1);
    }

    let remove = ForceRebuild {
        packages: rebuild.packages.clone().into_iter().collect(),
    };

    let response: ForceRebuildResponse = client
        .post(&endpoints.rebuilt_packages())
        .send_json(remove)
        .map_err(Box::new)?
        .into_json()?;

    if !response.not_found.is_empty() {
        let are_is = if response.not_found.len() > 1 {
            "are"
        } else {
            "is"
        };
        warn!(
            "{} {are_is} not tracked",
            combine_for_display(&response.not_found)
        );
    }
    if response.not_found.is_empty() {
        info!(
            "Started rebuild for {}",
            combine_for_display(&rebuild.packages)
        );
        Ok(0)
    } else {
        error!("No builds have been started");
        Ok(1)
    }
}

pub fn status(config: &Config) -> Result<u8, Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    let status: Status = client
        .get(&endpoints.status())
        .call()
        .map_err(Box::new)?
        .into_json()?;

    let mut warnings = Vec::new();
    let package_text_block = wrap_text(&combine_for_display(&status.packages), 80);

    match check_for_repository(config) {
        Ok(true) => (),
        Ok(false) => {
            warnings.push("Your pacman.conf does not seem to contain the servers repository");
        }
        Err(_) => warnings
            .push("Could not check if pacman is set up correctly. Could not read pacman.conf"),
    };

    info!(
        "Querying {} on port {}",
        config.server.address.green(),
        config.server.port.to_string().green()
    );
    info!("");
    if warnings.is_empty() {
        info!("No issues found!");
    } else {
        warn!("Warnings:");
        for warning in warnings {
            warn!("{warning}");
        }
    }
    info!("");
    info!("{}", "Tracked packages:".bold());
    info!("{package_text_block}");

    Ok(0)
}

fn check_for_repository(config: &Config) -> Result<bool, std::io::Error> {
    let pacman_conf = read_to_string("/etc/pacman.conf")?;
    let port = if config.server.port == 80 && !config.server.https
        || config.server.port == 443 && config.server.https
    {
        String::new()
    } else {
        format!(":{}", config.server.port)
    };

    let directive = format!("{}{port}/repo", config.server.address);

    Ok(pacman_conf.lines().any(|line| line.contains(&directive)))
}
