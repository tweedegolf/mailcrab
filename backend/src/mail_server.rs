/// Receives email over SMTP, parses and broadcasts messages over an internal queue
use mailin_embedded::{Server, SslConfig};
use std::{
    io::{self},
    net::ToSocketAddrs,
};
use tokio::sync::broadcast::Sender;
use tracing::{event, Level};

use crate::types::MailMessage;

#[derive(Clone, Debug)]
struct MailHandler {
    // internal broadcast queue
    tx: Sender<MailMessage>,

    // incoming message buffer
    buffer: Vec<u8>,
}

impl MailHandler {
    fn create(tx: Sender<MailMessage>) -> Self {
        MailHandler {
            tx,
            buffer: Vec::new(),
        }
    }
}

impl MailHandler {
    fn parse_mail(&mut self) -> Result<(), &'static str> {
        // parse the email and convert it to a internal data structure
        let parsed = mail_parser::Message::parse(&self.buffer)
            .ok_or("Could not parse email using mail_parser")?;
        let message = parsed.try_into()?;

        // clear the message buffer
        self.buffer.clear();

        // send the message to a internal queue
        self.tx
            .send(message)
            .map_err(|_| "Could not send email to own broadcast channel")?;

        Ok(())
    }
}

impl mailin::Handler for MailHandler {
    fn helo(&mut self, _ip: std::net::IpAddr, _domain: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn mail(&mut self, _ip: std::net::IpAddr, _domain: &str, _from: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn rcpt(&mut self, _to: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn data_start(
        &mut self,
        domain: &str,
        from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> mailin::Response {
        event!(Level::INFO, "New email on {} from {}", domain, from);
        mailin::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin::Response {
        if let Err(e) = self.parse_mail() {
            event!(Level::WARN, "Error parsing email: {}", e);
        }

        mailin::response::OK
    }

    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        _authentication_id: &str,
        _password: &str,
    ) -> mailin::Response {
        mailin::response::INVALID_CREDENTIALS
    }
}

pub fn smtp_listen<A: ToSocketAddrs>(
    addr: A,
    tx: Sender<MailMessage>,
) -> Result<(), mailin_embedded::err::Error> {
    let handler = MailHandler::create(tx);
    let mut server = Server::new(handler);

    let name = env!("CARGO_PKG_NAME");

    server
        .with_name(name)
        .with_ssl(SslConfig::None)?
        .with_addr(addr)?;

    server.serve()?;

    Ok(())
}
