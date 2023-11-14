use tokio::sync::broadcast::Sender;
use tracing::{info, warn};

use crate::{types::MailMessage, VERSION};

use super::Result;

#[derive(Clone, Debug)]
pub(super) struct MailHandler {
    // internal broadcast queue
    tx: Sender<MailMessage>,

    // incoming message buffer
    buffer: Vec<u8>,
    envelope_from: String,
    envelope_recipients: Vec<String>,
}

impl MailHandler {
    pub(super) fn create(tx: Sender<MailMessage>) -> Self {
        MailHandler {
            tx,
            buffer: Vec::new(),
            envelope_from: String::new(),
            envelope_recipients: Vec::new(),
        }
    }
}

impl MailHandler {
    fn parse_mail(&mut self) -> Result<MailMessage> {
        // parse the email and convert it to a internal data structure
        let parsed = mail_parser::Message::parse(&self.buffer)
            .ok_or("Could not parse email using mail_parser")?;
        let mut message: MailMessage = parsed.try_into()?;
        message.envelope_from = std::mem::take(&mut self.envelope_from);
        message.envelope_recipients = std::mem::take(&mut self.envelope_recipients);

        // clear the message buffer
        self.buffer.clear();

        // send the message to a internal queue
        self.tx
            .send(message.clone())
            .map_err(|_| "Could not send email to own broadcast channel")?;

        Ok(message)
    }
}

impl mailin::Handler for MailHandler {
    fn helo(&mut self, _ip: std::net::IpAddr, _domain: &str) -> mailin::Response {
        // NOTE that response is more as just '250 OK'
        mailin::response::OK
    }

    fn mail(&mut self, _ip: std::net::IpAddr, _domain: &str, from: &str) -> mailin::Response {
        self.envelope_from = from.to_string();

        // introductions
        mailin::response::Response::custom(
            250,
            format!("Pleased to meet you! This is Mailcrab version {VERSION}",),
        )
    }

    fn rcpt(&mut self, to: &str) -> mailin::Response {
        // RCPT may be repeated any number of times, so store every value.
        self.envelope_recipients.push(to.to_string());

        mailin::response::OK
    }

    fn data_start(
        &mut self,
        domain: &str,
        from: &str,
        _is8bit: bool,
        to: &[String],
    ) -> mailin::Response {
        info!("Incoming message on {} from {} to {:?}", domain, from, to);

        mailin::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin::Response {
        match self.parse_mail() {
            Err(e) => {
                warn!("Error parsing email: {}", e);

                mailin::response::Response::custom(500, "Error parsing message".to_string())
            }
            Ok(message) => mailin::response::Response::custom(
                250,
                format!("2.0.0 Ok: queued as {}", message.id),
            ),
        }
    }

    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        _authentication_id: &str,
        _password: &str,
    ) -> mailin::Response {
        mailin::response::AUTH_OK
    }
}
