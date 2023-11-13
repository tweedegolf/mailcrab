use std::io;
use std::net::SocketAddr;
use tokio::{fs, sync::broadcast::Sender};
use tokio_graceful_shutdown::SubsystemHandle;
use tracing::{event, Level};

use crate::{types::MailMessage, VERSION_BE};

use mailin::{Action, AuthMechanism, Response, Session, SessionBuilder};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
use rustls::PrivateKey;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tracing::{debug, error};

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, PartialEq)]
pub enum SessionResult {
    Finished,
    UpgradeTls,
}

async fn write_response<W>(writer: &mut W, res: &Response) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let buf: Vec<u8> = res.buffer().map_err(|e| e.to_string())?;

    debug!("Sending: {}", String::from_utf8_lossy(&buf));

    writer.write_all(&buf).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    Ok(())
}

async fn create_tls_acceptor(name: &str) -> Result<TlsAcceptor> {
    let mut cert_params = CertificateParams::default();
    let mut dis_name = DistinguishedName::new();
    dis_name.push(DnType::CommonName, name);
    cert_params.distinguished_name = dis_name;

    let cert = Certificate::from_params(cert_params).map_err(|e| e.to_string())?;
    let cert_pem = cert.serialize_pem().map_err(|e| e.to_string())?;

    fs::write("cert.pem", cert_pem)
        .await
        .map_err(|e| e.to_string())?;
    fs::write("key.pem", cert.serialize_private_key_pem())
        .await
        .map_err(|e| e.to_string())?;

    let private_key = PrivateKey(cert.serialize_private_key_der());
    let certificate = rustls::Certificate(cert.serialize_der().map_err(|e| e.to_string())?);

    let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private_key)
        .map_err(|e| e.to_string())?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

async fn handle_steam<S>(
    mut stream: &mut BufReader<S>,
    session: &mut Session<MailHandler>,
) -> Result<SessionResult>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(80);

    debug!("Sending 220");
    write_response(&mut stream, &session.greeting()).await?;

    loop {
        line.clear();
        let n = match stream.read_until(b'\n', &mut line).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(format!("SMTP server error {e}")),
        };

        debug!("Received: {}", String::from_utf8_lossy(&line[0..n]));

        let response = session.process(&line);

        match response.action {
            Action::Reply => {
                write_response(&mut stream, &response).await?;
            }
            Action::Close if response.is_error => {
                write_response(&mut stream, &response).await?;

                return Err(format!("SMT server error code {}", response.code));
            }
            Action::Close => {
                write_response(&mut stream, &response).await?;

                return Ok(SessionResult::Finished);
            }
            Action::UpgradeTls => {
                write_response(&mut stream, &response).await?;

                return Ok(SessionResult::UpgradeTls);
            }
            Action::NoReply => {}
        };
    }

    debug!("Connection closed");

    Ok(SessionResult::Finished)
}

async fn upgrade_connection(
    stream: TcpStream,
    acceptor: &TlsAcceptor,
) -> Result<BufReader<TlsStream<TcpStream>>> {
    let accept_buffer = acceptor.accept(stream).await.map_err(|e| e.to_string())?;

    Ok(BufReader::new(accept_buffer))
}

async fn handle_connection(
    socket: TcpStream,
    session_builder: SessionBuilder,
    tls: TlsConfig,
    handler: MailHandler,
) -> Result<()> {
    let peer_addr = socket.peer_addr().map_err(|e| e.to_string())?;
    let mut stream: BufReader<TcpStream> = BufReader::new(socket);
    let mut session: Session<MailHandler> = session_builder.build(peer_addr.ip(), handler);

    match &tls {
        TlsConfig::None => {
            handle_steam(&mut stream, &mut session).await?;
        }
        TlsConfig::Wrapped(acceptor) => {
            let mut stream = upgrade_connection(stream.into_inner(), acceptor).await?;
            session.tls_active();
            handle_steam(&mut stream, &mut session).await?;
        }
        TlsConfig::StartTls(acceptor) => {
            let session_result = handle_steam(&mut stream, &mut session).await?;
            if session_result == SessionResult::UpgradeTls {
                let mut stream = upgrade_connection(stream.into_inner(), acceptor).await?;
                session.tls_active();
                handle_steam(&mut stream, &mut session).await?;
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct MailHandler {
    // internal broadcast queue
    tx: Sender<MailMessage>,

    // incoming message buffer
    buffer: Vec<u8>,
    envelope_from: String,
    envelope_recipients: Vec<String>,
}

impl MailHandler {
    fn create(tx: Sender<MailMessage>) -> Self {
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
        // Remote end told us about itself, time to tell more about our self.
        mailin::response::Response::custom(
            250,
            format!("Pleased to meet you! This is Mailcrab version {VERSION_BE}",),
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
        event!(
            Level::INFO,
            "Incoming message on {} from {} to {:?}",
            domain,
            from,
            to
        );
        mailin::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin::Response {
        match self.parse_mail() {
            Err(e) => {
                event!(Level::WARN, "Error parsing email: {}", e);

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

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum TlsMode {
    None,
    StartTls,
    Wrapped,
}

#[derive(Clone)]
pub enum TlsConfig {
    None,
    StartTls(TlsAcceptor),
    Wrapped(TlsAcceptor),
}

pub struct MailServer {
    address: SocketAddr,
    server_name: &'static str,
    session_builder: SessionBuilder,
    tls: TlsConfig,
    handler: MailHandler,
}

impl MailServer {
    pub fn new(tx: Sender<MailMessage>) -> Self {
        let server_name = env!("CARGO_PKG_NAME");

        Self {
            address: ([0, 0, 0, 0], 2525).into(),
            server_name: env!("CARGO_PKG_NAME"),
            session_builder: SessionBuilder::new(server_name),
            tls: TlsConfig::None,
            handler: MailHandler::create(tx),
        }
    }

    pub fn with_address(mut self, address: SocketAddr) -> Self {
        self.address = address;

        self
    }

    pub async fn with_tls(mut self, tls_mode: TlsMode) -> Result<Self> {
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

    pub fn with_authentication(mut self) -> Self {
        self.session_builder.enable_auth(AuthMechanism::Plain);

        self
    }

    async fn serve(&self, listener: TcpListener, handle: SubsystemHandle) -> Result<()> {
        loop {
            let (socket, peer_addr) = tokio::select! {
                result = listener.accept() => result.map_err(|e| e.to_string())?,
                _ = handle.on_shutdown_requested() => return Ok(()),
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

    pub async fn listen(self, handle: SubsystemHandle) -> Result<JoinHandle<Result<()>>> {
        let listener = TcpListener::bind(&self.address)
            .await
            .map_err(|e| e.to_string())?;

        let join = tokio::task::spawn(async move {
            if let Err(e) = self.serve(listener, handle).await {
                error!("SMTP server error {e}");
            }

            Ok(())
        });

        Ok(join)
    }
}
