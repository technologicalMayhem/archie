use crate::config::Config;
use crate::Error;
use clap::Args;
use colored::Colorize;
use coordinator::endpoints::Endpoints;
use coordinator::{RemovePackages, Status, WorkAssignment, WorkOrders};
use tracing::warn;
use ureq::Agent;

#[derive(Clone, Args)]
pub struct Add {
    /// The packages to add
    packages: Vec<String>,
}

pub fn add(config: &Config, add: Add) -> Result<(), Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    if add.packages.is_empty() {
        warn!("No packages to build were given.");
    }

    client
        .post(&endpoints.add_packages())
        .send_json(&WorkOrders {
            packages: add
                .packages
                .into_iter()
                .map(|package| WorkAssignment { package })
                .collect(),
        })
        .map_err(Box::new)?;
    Ok(())
}

#[derive(Clone, Args)]
pub struct Remove {
    /// The packages to remove
    packages: Vec<String>,
}

pub fn remove(config: &Config, remove: Remove) -> Result<(), Error> {
    let client = Agent::new();
    let endpoints: Endpoints = config.server.to_endpoints();

    if remove.packages.is_empty() {
        warn!("No packages to remove were given.");
    }

    client
        .post(&endpoints.remove_packages())
        .send_json(RemovePackages {
            packages: remove.packages,
        })
        .map_err(Box::new)?;

    Ok(())
}

pub fn status(config: &Config) -> Result<(), Error> {
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

    Ok(())
}
