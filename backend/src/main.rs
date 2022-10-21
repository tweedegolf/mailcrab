use crate::web_server::http_serve;
use std::{
    collections::HashMap,
    env,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast::Receiver;
use tracing::{event, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use types::{MailMessage, MessageId};

mod mail_server;
mod types;
mod web_server;
pub struct AppState {
    rx: Receiver<MailMessage>,
    storage: RwLock<HashMap<MessageId, MailMessage>>,
}

/// get a port number from the environment or return default value
fn get_env_port(name: &'static str, default: u16) -> u16 {
    env::var(name)
        .unwrap_or_default()
        .parse()
        .unwrap_or(default)
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

    let smtp_port: u16 = get_env_port("SMTP_PORT", 2525);
    let http_port: u16 = get_env_port("HTTP_PORT", 8080);

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
    });

    // receive and broadcast mail messages
    let smtp_join = tokio::spawn(async move {
        if let Err(e) = mail_server::smtp_listen(("0.0.0.0", smtp_port), tx) {
            eprintln!("MailCrab error: {}", e);
        }
    });

    // store broadcasted messages in a key/value store
    let state = app_state.clone();
    let storage_join = tokio::spawn(async move {
        loop {
            if let Ok(message) = storage_rx.recv().await {
                if let Ok(mut storage) = state.storage.write() {
                    storage.insert(message.id, message);
                }
            }
        }
    });

    // serve a web application to retrieve and view mail messages
    http_serve(app_state, http_port).await;

    smtp_join.abort();
    storage_join.abort();
}

#[cfg(test)]
mod test {
    use fake::faker::company::en::CatchPhase;
    use fake::faker::internet::en::FreeEmail;
    use fake::faker::lorem::en::Paragraph;
    use fake::faker::name::en::Name;
    use fake::Fake;
    use lettre::{ClientSecurity, SmtpClient, Transport};
    use lettre_email::{mime, EmailBuilder};
    use std::{path::Path, thread, time};

    #[test]
    fn receive_email() {
        let addr = "127.0.0.1:2525";

        let mut mailer = SmtpClient::new(addr, ClientSecurity::None)
            .unwrap()
            .transport();

        loop {
            let to: (String, String) = (FreeEmail().fake(), Name().fake());
            let from: (String, String) = (FreeEmail().fake(), Name().fake());
            let subject: String = CatchPhase().fake();
            let body: String = Paragraph(2..3).fake();

            println!("Sending mail to {}", &to.0);

            let email = EmailBuilder::new()
                .to(to)
                .from(from)
                .subject(subject)
                .text(body)
                .attachment_from_file(Path::new("blank.pdf"), None, &mime::APPLICATION_PDF)
                .unwrap()
                .build()
                .unwrap();

            if let Err(e) = mailer.send(email.into()) {
                eprintln!("{}", e);
            }

            thread::sleep(time::Duration::from_secs(5));
        }
    }
}
