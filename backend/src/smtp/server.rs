use mailin::{AuthMechanism, SessionBuilder};
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::broadcast::Sender};
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::{error::Result, smtp::connection::handle_connection, types::MailMessage};

use super::{handler::MailHandler, tls::create_tls_acceptor};

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(super) enum TlsMode {
    None,
    StartTls,
    Wrapped,
}

#[derive(Clone)]
pub(super) enum TlsConfig {
    None,
    StartTls(TlsAcceptor),
    Wrapped(TlsAcceptor),
}

pub(super) struct MailServer {
    address: SocketAddr,
    server_name: &'static str,
    session_builder: SessionBuilder,
    tls: TlsConfig,
    handler: MailHandler,
}

impl MailServer {
    pub(super) fn new(tx: Sender<MailMessage>) -> Self {
        let server_name = env!("CARGO_PKG_NAME");

        Self {
            address: ([0, 0, 0, 0], 2525).into(),
            server_name: env!("CARGO_PKG_NAME"),
            session_builder: SessionBuilder::new(server_name),
            tls: TlsConfig::None,
            handler: MailHandler::create(tx),
        }
    }

    pub(super) fn with_address(mut self, address: SocketAddr) -> Self {
        self.address = address;

        self
    }

    pub(super) async fn with_tls(mut self, tls_mode: TlsMode) -> Result<Self> {
        self.tls = match tls_mode {
            TlsMode::None => TlsConfig::None,
            TlsMode::StartTls => {
                self.session_builder.enable_start_tls();

                TlsConfig::StartTls(create_tls_acceptor(self.server_name).await?)
            }
            TlsMode::Wrapped => TlsConfig::Wrapped(create_tls_acceptor(self.server_name).await?),
        };

        Ok(self)
    }

    pub(super) fn with_authentication(mut self) -> Self {
        self.session_builder.enable_auth(AuthMechanism::Plain);

        self
    }

    pub(super) async fn serve(&self, token: CancellationToken) -> Result<()> {
        let listener = TcpListener::bind(&self.address).await?;
        info!(
            "SMTP server ready to accept connections on {}",
            &self.address
        );

        loop {
            let (socket, peer_addr) = tokio::select! {
                result = listener.accept() => result?,
                _ = token.cancelled() => {
                    info!("Shutting down mail server");
                    return Ok(());
                },
            };

            debug!("Connection from {peer_addr:?}");

            tokio::spawn({
                let session_builder = self.session_builder.clone();
                let tls = self.tls.clone();
                let handler = self.handler.clone();

                handle_connection(socket, session_builder, tls, handler)
            });
        }
    }
}
