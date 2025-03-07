use std::net::IpAddr;
use smtp::mail_server;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

mod error;
mod smtp;
mod types;

/// retrieve the version from Cargo.toml, note that this will yield an error
/// when compiling without cargo
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use types::{MailMessage, MessageId, Address, Attachment};

pub struct TestMailServerHandle {
    pub token: CancellationToken,
    pub rx: Receiver<MailMessage>,
}

/// Start a test mail server, returns a channel on which messages can be received
/// and a token to stop the server
/// This server is NOT intended for production use, it is a development tool
pub async fn development_mail_server(smtp_host: impl Into<IpAddr>, smtp_port: u16) -> TestMailServerHandle {
    let (tx, rx) = tokio::sync::broadcast::channel::<MailMessage>(128);
    let token = CancellationToken::new();

    tokio::spawn(
    mail_server(
        smtp_host.into(),
        smtp_port,
        tx,
        false,
        token.clone(),
    ));

    TestMailServerHandle {
        token,
        rx,
    }
}


#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fake::{faker::{company::en::CatchPhrase, internet::en::SafeEmail}, Fake};
    use lettre::{message::{header, SinglePart}, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    use rand::Rng;

    #[tokio::test]
    async fn test_mail_server() {
        let mut rng = rand::rng();
        let port = rng.random_range(10_000..30_000);

        let mut handle = crate::development_mail_server([127, 0, 0, 1], port).await;

        let mailer: AsyncSmtpTransport::<Tokio1Executor> = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1".to_string())
            .port(port)
            .build();

        let email = Message::builder()
            .from(SafeEmail().fake::<String>().parse().unwrap())
            .to(SafeEmail().fake::<String>().parse().unwrap())
            .subject(CatchPhrase().fake::<String>())
            .singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(CatchPhrase().fake::<String>())
            )
            .expect("failed to build email");

        // try sending message
        for i in 0..=10 {
            match mailer.send(email.clone()).await {
                Ok(_) => break,
                Err(e) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    if i == 10 {
                        panic!("failed to send email: {e}");
                    }
                }
            }
        }

        let received = handle.rx.recv().await.expect("failed to receive email");

        // assert uuid length
        assert_eq!(received.id.to_string().len(), 36);

        let _ = handle.token.cancel();
    }
}
