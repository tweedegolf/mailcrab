use rust_embed::{EmbeddedFile, RustEmbed};
use std::{
    collections::HashMap,
    env,
    error::Error,
    net::IpAddr,
    process,
    str::FromStr,
    sync::{Arc, RwLock},
};
use tokio::{sync::broadcast::Receiver, time::Duration};
use tokio_graceful_shutdown::{errors::GracefulShutdownError, Toplevel};
use tracing::{error, event, info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use types::{MailMessage, MessageId};

use crate::{mail_server::mail_server, storage::storage, web_server::http_server};

mod mail_server;
mod storage;
mod types;
mod web_server;

#[cfg(test)]
mod tests;

/// retrieve the version from Cargo.toml, note that this will yield an error
/// when compiling without cargo
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// application state, holds all messages, a message queue and configuration
pub struct AppState {
    rx: Receiver<MailMessage>,
    storage: RwLock<HashMap<MessageId, MailMessage>>,
    prefix: String,
    index: Option<String>,
    retention_period: Duration,
}

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct Asset;

/// get a configuration from the environment or return default value
fn parse_env_var<T: FromStr>(name: &'static str, default: T) -> T {
    env::var(name)
        .unwrap_or_default()
        .parse::<T>()
        .unwrap_or(default)
}

/// preload the HTML for the index, replace dynamic values
fn load_index(path_prefix: &str) -> Option<String> {
    let index: EmbeddedFile = Asset::get("index.html")?;
    let index = String::from_utf8(index.data.to_vec()).ok()?;
    let path_prefix = if path_prefix == "/" { "" } else { path_prefix };

    // add path prefix to asset includes
    Some(
        index
            .replace("href=\"/", &format!("href=\"{path_prefix}/static/"))
            .replace(
                "'/mailcrab-frontend",
                &format!("'{path_prefix}/static/mailcrab-frontend"),
            ),
    )
}

async fn run() -> Result<(), GracefulShutdownError<Box<dyn Error + Send + Sync>>> {
    let smtp_host: IpAddr = parse_env_var("SMTP_HOST", [0, 0, 0, 0].into());
    let http_host: IpAddr = parse_env_var("HTTP_HOST", [127, 0, 0, 1].into());
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    let http_port: u16 = parse_env_var("HTTP_PORT", 1080);

    // Enable auth implicitly enable TLS
    let enable_tls_auth: bool = std::env::var("ENABLE_TLS_AUTH").map_or_else(
        |_| false,
        |v| v.to_ascii_lowercase().parse().unwrap_or(false),
    );

    // construct path prefix
    let prefix = std::env::var("MAILCRAB_PREFIX").unwrap_or_default();
    let prefix = format!("/{}", prefix.trim_matches('/'));

    // optional retention period, the default is 0 - which means messages are kept forever
    let retention_period: u64 = parse_env_var("MAILCRAB_RETENTION_PERIOD", 0);

    event!(
        Level::INFO,
        "MailCrab HTTP server starting on {http_host}:{http_port} and SMTP server on {smtp_host}:{smtp_port}"
    );

    // initialize internal broadcast queue
    let (tx, rx) = tokio::sync::broadcast::channel::<MailMessage>(16);
    let storage_rx = rx.resubscribe();
    let app_state = Arc::new(AppState {
        rx,
        storage: Default::default(),
        index: load_index(&prefix),
        prefix,
        retention_period: Duration::from_secs(retention_period),
    });

    // store broadcasted messages in a key/value store
    let state = app_state.clone();

    Toplevel::new()
        .start("Storage server", move |h| storage(storage_rx, state, h))
        .start("Mail server", move |h| {
            mail_server(smtp_host, smtp_port, tx, enable_tls_auth, h)
        })
        .start("Web server", move |h| {
            http_server(http_host, http_port, app_state, h)
        })
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(5000))
        .await
}

#[tokio::main]
async fn main() {
    // initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "mailcrab_backend=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let result = run().await;

    let exit_code = match result {
        Err(e) => {
            error!("MailCrab error {e}");
            // failure
            1
        }
        _ => {
            info!("Thank you for using MailCrab!");
            // success
            0
        }
    };

    process::exit(exit_code);
}
