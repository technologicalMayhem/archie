use clap::Parser;
use coordinator::{endpoints, WorkAssignment, WorkOrders};
use reqwest::Client;
use tracing::{error, info, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
struct Arguments {
    packages: Vec<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(LevelFilter::DEBUG)
        .with(fmt::layer().without_time())
        .init();
    let args = Arguments::parse();
    let client = Client::new();

    if args.packages.is_empty() {
        warn!("No packages to build were given.");
    }

    match client
        .post(endpoints::work())
        .json(&WorkOrders {
            packages: args
                .packages
                .into_iter()
                .map(|package| WorkAssignment { package })
                .collect(),
        })
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                info!("Sent request to build packages");
            } else {
                error!(
                    "Got {} from server {:?}",
                    response.status(),
                    response.error_for_status()
                );
            }
        }
        Err(err) => {
            error!("Failed to send request: {err}");
        }
    }
}
