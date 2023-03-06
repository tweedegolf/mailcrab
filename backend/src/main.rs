use std::{
    collections::HashMap,
    env, process,
    sync::{Arc, RwLock},
};
use tokio::signal;
use tokio::sync::broadcast::Receiver;
use tracing::{event, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use types::{MailMessage, MessageId};

use crate::web_server::http_server;

mod mail_server;
mod types;
mod web_server;
pub struct AppState {
    rx: Receiver<MailMessage>,
    storage: RwLock<HashMap<MessageId, MailMessage>>,
    prefix: String,
    index: Option<String>,
}

/// get a port number from the environment or return default value
fn get_env_port(name: &'static str, default: u16) -> u16 {
    env::var(name)
        .unwrap_or_default()
        .parse()
        .unwrap_or(default)
}

fn load_index() -> Option<String> {
    let index: String = std::fs::read_to_string("dist/index.html").ok()?;

    // remove slash from start of asset includes, so they are loaded by relative path
    Some(index
        .replace("href=\"/", "href=\"./static/")
        .replace("'/mailcrab-frontend", "'./static/mailcrab-frontend"))
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

    let smtp_port: u16 = get_env_port("SMTP_PORT", 1025);
    let http_port: u16 = get_env_port("HTTP_PORT", 1080);

    // construct path prefix
    let prefix = std::env::var("MAILCRAB_PREFIX").unwrap_or_default();
    let prefix = format!("/{}", prefix.trim_matches('/'));

    event!(
        Level::INFO,
        "MailCrab server starting http on port {} and smtp on port {}",
        http_port,
        smtp_port
    );

    // initialize internal broadcast queue
    let (tx, rx) = tokio::sync::broadcast::channel::<MailMessage>(16);
    let mut storage_rx = rx.resubscribe();
    let app_state = Arc::new(AppState {
        rx,
        storage: Default::default(),
        index: load_index(),
        prefix,
    });

    // receive and broadcast mail messages
    tokio::spawn(async move {
        if let Err(e) = mail_server::smtp_listen(("0.0.0.0", smtp_port), tx) {
            eprintln!("MailCrab error: {}", e);
        }
    });

    // store broadcasted messages in a key/value store
    let state = app_state.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(message) = storage_rx.recv().await {
                if let Ok(mut storage) = state.storage.write() {
                    storage.insert(message.id, message);
                }
            }
        }
    });

    tokio::task::spawn(async move {
        http_server(app_state, http_port).await;
    });

    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");

    process::exit(0);
}

#[cfg(test)]
mod test {
    use fake::faker::company::en::{Buzzword, CatchPhase};
    use fake::faker::internet::en::FreeEmail;
    use fake::faker::lorem::en::Paragraph;
    use fake::faker::name::en::Name;
    use fake::Fake;
    use lettre::message::header::ContentType;
    use lettre::message::{Attachment, MultiPart, SinglePart};
    use lettre::{Message, SmtpTransport, Transport};
    use rand::prelude::*;
    use std::{thread, time};

    #[test]
    fn receive_email() {
        let mailer = SmtpTransport::builder_dangerous("localhost")
            .port(1025)
            .build();
        let mut rng = rand::thread_rng();

        loop {
            let to: String = FreeEmail().fake();
            let to_name: String = Name().fake();
            let from: String = FreeEmail().fake();
            let from_name: String = Name().fake();
            let body: String = Paragraph(2..3).fake();
            let html: String = format!(
                "{}\n<p><a href=\"https://github.com/tweedegolf/mailcrab\">external link</a></p>",
                body
            );

            println!("Sending mail to {}", &to);

            let builder = Message::builder()
                .from(format!("{from_name} <{from}>",).parse().unwrap())
                .to(format!("{to_name} <{to}>").parse().unwrap())
                .subject(CatchPhase().fake::<String>());

            let mut multipart = MultiPart::mixed().multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(body))
                    .singlepart(SinglePart::html(html)),
            );

            let r: u8 = rng.gen();
            let filebody = std::fs::read("blank.pdf").unwrap();
            let content_type = ContentType::parse("application/pdf").unwrap();

            for _ in 0..(r % 3) {
                let filename = format!("{}.pdf", Buzzword().fake::<&str>().to_ascii_lowercase());
                let attachment =
                    Attachment::new(filename).body(filebody.clone(), content_type.clone());
                multipart = multipart.singlepart(attachment);
            }

            let email = builder.multipart(multipart).unwrap();

            if let Err(e) = mailer.send(&email) {
                eprintln!("{}", e);
            }

            thread::sleep(time::Duration::from_secs(5));
        }
    }
}
