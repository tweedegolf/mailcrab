use rust_embed::{EmbeddedFile, RustEmbed};

use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    net::IpAddr,
    process,
    str::FromStr,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::time::Duration;
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};
use tracing::{event, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use types::{MailMessage, MessageId};

use crate::web_server::http_server;

mod mail_server;
mod types;
mod web_server;

pub const VERSION_BE: &'static str = env!("CARGO_PKG_VERSION");
// Please note that above line of code will yield an error
// if the environment variable isn't defined,
// for example if you execute rustc directly without cargo.

pub struct AppState {
    rx: Receiver<MailMessage>,
    storage: RwLock<HashMap<MessageId, MailMessage>>,
    prefix: String,
    index: Option<String>,
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

async fn storage(
    mut storage_rx: Receiver<MailMessage>,
    state: Arc<AppState>,
    handle: SubsystemHandle,
) -> Result<(), Infallible> {
    let mut running = true;

    while running {
        tokio::select! {
            incoming = storage_rx.recv() => {
                if let Ok(message) = incoming {
                    if let Ok(mut storage) = state.storage.write() {
                        storage.insert(message.id, message);
                    }
                }
            },
            _ = handle.on_shutdown_requested() => {
                running = false;
            }
        }
    }

    Ok(())
}

async fn mail_server(
    smtp_host: IpAddr,
    smtp_port: u16,
    tx: Sender<MailMessage>,
    enable_tls_auth: bool,
    handle: SubsystemHandle,
) -> Result<(), Infallible> {
    let task = tokio::task::spawn_blocking(move || {
        if let Err(e) = mail_server::smtp_listen((smtp_host, smtp_port), tx, enable_tls_auth) {
            event!(Level::ERROR, "MailCrab mail server error {e}");
        }
    });

    tokio::select! {
        _ = task => {},
        _ = handle.on_shutdown_requested() => {}
    };

    Ok(())
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
    });

    // store broadcasted messages in a key/value store
    let state = app_state.clone();

    let result = Toplevel::new()
        .start("Storage server", move |h| storage(storage_rx, state, h))
        .start("Mail server", move |h| {
            mail_server(smtp_host, smtp_port, tx, enable_tls_auth, h)
        })
        .start("Web server", move |h| {
            http_server(http_host, http_port, app_state, h)
        })
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(5000))
        .await;

    let exit_code = match result {
        Err(e) => {
            event!(Level::ERROR, "MailCrab error {e}");
            // failure
            1
        }
        _ => {
            event!(Level::INFO, "Thank you for using MailCrab!");
            // success
            0
        }
    };

    process::exit(exit_code);
}

#[cfg(test)]
mod test {
    use crate::parse_env_var;
    use crate::types::MailMessageMetadata;
    use fake::faker::company::en::{Buzzword, CatchPhase};
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::Paragraph;
    use fake::faker::name::en::Name;
    use fake::Fake;
    use lettre::message::header::ContentType;
    use lettre::message::{Attachment, MultiPart, SinglePart};
    use lettre::{Message, SmtpTransport, Transport};
    use std::process::{Command, Stdio};
    use tokio::time::{sleep, Duration};

    fn send_message(
        with_html: bool,
        with_plain: bool,
        with_attachment: bool,
    ) -> Result<Message, Box<dyn std::error::Error>> {
        let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
        let mailer = SmtpTransport::builder_dangerous("127.0.0.1".to_string())
            .port(smtp_port)
            .build();

        let to: String = SafeEmail().fake();
        let to_name: String = Name().fake();
        let from: String = SafeEmail().fake();
        let from_name: String = Name().fake();
        let body: String = [
            Paragraph(2..3).fake::<String>(),
            Paragraph(2..3).fake::<String>(),
            Paragraph(2..3).fake::<String>(),
        ]
        .join("\n");
        let html: String = format!(
            "{}\n<p><a href=\"https://github.com/tweedegolf/mailcrab\">external link</a></p>",
            body.replace("\n", "<br>\n")
        );

        let builder = Message::builder()
            .from(format!("{from_name} <{from}>",).parse()?)
            .to(format!("{to_name} <{to}>").parse()?)
            .subject(CatchPhase().fake::<String>());

        let mut multipart = MultiPart::mixed().build();

        match (with_html, with_plain) {
            (true, true) => {
                multipart = multipart.multipart(
                    MultiPart::alternative()
                        .singlepart(SinglePart::plain(body))
                        .singlepart(SinglePart::html(html)),
                );
            }
            (false, true) => {
                multipart = multipart.singlepart(SinglePart::plain(body));
            }
            (true, false) => {
                multipart = multipart.singlepart(SinglePart::html(html));
            }
            _ => panic!("Email should have html or plain body"),
        };

        if with_attachment {
            let filebody = std::fs::read("blank.pdf")?;
            let content_type = ContentType::parse("application/pdf")?;
            let filename = format!("{}.pdf", Buzzword().fake::<&str>().to_ascii_lowercase());
            let attachment = Attachment::new(filename).body(filebody.clone(), content_type.clone());
            multipart = multipart.singlepart(attachment);
        }

        let email = builder.multipart(multipart)?;

        mailer.send(&email)?;

        Ok(email)
    }

    async fn test_receive_messages() -> Result<Vec<MailMessageMetadata>, Box<dyn std::error::Error>>
    {
        send_message(true, true, false)?;
        sleep(Duration::from_millis(1500)).await;
        send_message(true, false, false)?;
        sleep(Duration::from_millis(1500)).await;
        send_message(false, true, true)?;

        let http_port: u16 = parse_env_var("HTTP_PORT", 1080);
        let mails: Vec<MailMessageMetadata> =
            reqwest::get(format!("http://127.0.0.1:{http_port}/api/messages"))
                .await?
                .json()
                .await?;

        Ok(mails)
    }

    #[tokio::test]
    async fn receive_message() {
        let mut cmd = Command::new("cargo")
            .arg("run")
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        // wait for mailcrab to startup
        sleep(Duration::from_millis(20_000)).await;
        let messages = test_receive_messages().await;
        cmd.kill().unwrap();
        let messages = messages.unwrap();

        assert_eq!(messages.len(), 3);
        assert!(messages[0].has_html);
        assert!(messages[0].has_plain);
        assert!(messages[0].attachments.is_empty());
        assert!(messages[1].has_html);
        assert!(!messages[1].has_plain);
        assert!(messages[1].attachments.is_empty());
        assert!(!messages[2].has_html);
        assert!(messages[2].has_plain);
        assert_eq!(messages[2].attachments.len(), 1);
    }
}
