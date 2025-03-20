//! Miscellaneous utilities

use std::net::SocketAddr;

use crate::Config;

/// Default address srvr binds to
const DEFAULT_ADDRESS: &str = "127.0.0.1:12234";

/// Get the value of ENV var, or a default
///
/// Only when:
/// - It is set
/// - It is not empty
pub fn env_var_or_else(var_name: &'static str, or_else: fn() -> String) -> String {
    use std::env::var;

    if let Ok(value) = var(var_name) {
        if !value.is_empty() {
            return value;
        }
    }

    or_else()
}

/// Setup tracing based on the environment
pub fn setup_tracing() {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::fmt::SubscriberBuilder;
    use tracing_subscriber::EnvFilter;

    let builder = SubscriberBuilder::default()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_target(false);

    // Only show log targets in debug builds
    #[cfg(debug_assertions)]
    let builder = builder.with_target(true);

    builder
        .try_init()
        .expect("Setting default subscriber failed");
}

/// Setup the address Jobber will bind to
pub fn setup_address(config: &Config) -> anyhow::Result<SocketAddr> {
    let mut address = config
        .address
        .as_ref()
        .map_or_else(
            || env_var_or_else("ADDRESS", || String::from(DEFAULT_ADDRESS)),
            Clone::clone,
        )
        .parse::<SocketAddr>()?;

    // optional override of just the port
    if let Some(port) = config.port {
        address.set_port(port);
    } else if let Ok(port) = std::env::var("PORT") {
        // only check non-empty strings
        if !port.is_empty() {
            let port = port.parse::<u16>()?;

            address.set_port(port);
        }
    }

    Ok(address)
}

/// Handler for graceful shutdown
///
/// Will listen to Ctrl+C to and initiate a shutdown
pub async fn graceful_shutdown() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c().await.expect("Valid CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Valid terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("Terminate signal received, starting graceful shutdown");
}
