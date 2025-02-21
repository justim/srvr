#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![doc = include_str!("../README.md")]

use std::process::exit;

use tokio::net::TcpListener;

use crate::app::app;
use crate::app::ServerState;
use crate::config::Config;
use crate::utils::graceful_shutdown;
use crate::utils::setup_address;
use crate::utils::setup_tracing;

mod app;
mod config;
mod encoding;
mod file_cache;
mod partial;
mod paths;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("Could not handle arguments: {err}");
            exit(1);
        }
    };

    let address = match setup_address(&config) {
        Ok(address) => address,
        Err(err) => {
            tracing::error!("Could not process address: {err}");
            exit(1);
        }
    };

    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("Could not listen on address {address}: {err}");
            exit(1);
        }
    };

    tracing::info!("                                   ");
    tracing::info!(" ███████╗██████╗ ██╗   ██╗██████╗  ");
    tracing::info!(" ██╔════╝██╔══██╗██║   ██║██╔══██╗ ");
    tracing::info!(" ███████╗██████╔╝██║   ██║██████╔╝ ");
    tracing::info!(" ╚════██║██╔══██╗╚██╗ ██╔╝██╔══██╗ ");
    tracing::info!(" ███████║██║  ██║ ╚████╔╝ ██║  ██║ is starting");
    tracing::info!(" ╚══════╝╚═╝  ╚═╝  ╚═══╝  ╚═╝  ╚═╝ ");
    tracing::info!("                                   ");
    tracing::info!("Serving {:?} on http://{address}", &config.base_dir);

    let state = ServerState::from_config(config);

    axum::serve(listener, app(state).into_make_service())
        .with_graceful_shutdown(graceful_shutdown())
        .await?;

    Ok(())
}
