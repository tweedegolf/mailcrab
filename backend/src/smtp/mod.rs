use std::net::IpAddr;
use tokio::sync::broadcast::Sender;
use tokio_graceful_shutdown::SubsystemHandle;
use tracing::error;

use crate::{error::Result, types::MailMessage};

use self::server::{MailServer, TlsMode};

mod connection;
mod handler;
mod server;
mod tls;

pub(crate) async fn mail_server(
    smtp_host: IpAddr,
    smtp_port: u16,
    tx: Sender<MailMessage>,
    enable_tls_auth: bool,
    handle: SubsystemHandle,
) -> Result<()> {
    let mut server = MailServer::new(tx).with_address((smtp_host, smtp_port).into());

    if enable_tls_auth {
        server = match server
            .with_authentication()
            .with_tls(TlsMode::Wrapped)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!("MailCrab mail server error {e}");

                return Ok(());
            }
        }
    }

    if let Err(e) = server.listen(handle).await {
        error!("MailCrab mail server error {e}");
    }

    Ok(())
}
