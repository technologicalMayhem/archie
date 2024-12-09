use crate::config::Config;
use crate::util::combine_for_display;
use crate::Error;
use clap::Args;
use colored::Colorize;
use coordinator::endpoints::Endpoints;
use coordinator::{
    AddPackages, AddPackagesResponse, RemovePackages, RemovePackagesResponse, Status,
};
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

pub fn status(config: &Config) -> Result<u8, Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    let status: Status = client
        .get(&endpoints.status())
        .call()
        .map_err(Box::new)?
        .into_json()?;

    println!("{}", "Packages:".bold());
    for package in status.packages {
        println!("- {}", package.green());
    }

    Ok(0)
}
