use imap_next::{
    imap_types::response::{CommandContinuationRequest, Greeting, Status},
    server::{Error as ServerError, Event, Options, ResponseHandle, Server},
    stream::{Error as StreamError, Stream},
};
use mailcrab::Result;
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::AppState;

use self::session::{CommandOutcome, Session, UidRegistry};

mod fetch;
mod session;

/// IMAP server task: accepts connections and serves the message store as a
/// single read/write INBOX, any credentials are accepted
pub(crate) async fn imap_server(
    host: IpAddr,
    port: u16,
    state: Arc<AppState>,
    token: CancellationToken,
) -> Result<()> {
    let address = SocketAddr::from((host, port));
    let listener = TcpListener::bind(&address).await?;
    let uids = Arc::new(Mutex::new(UidRegistry::new()));

    info!("IMAP server ready to accept connections on {address}");

    loop {
        let (socket, peer_addr) = tokio::select! {
            result = listener.accept() => result?,
            _ = token.cancelled() => {
                info!("Shutting down IMAP server");
                return Ok(());
            },
        };

        debug!("IMAP connection from {peer_addr:?}");

        tokio::spawn(handle_connection(
            socket,
            state.clone(),
            uids.clone(),
            token.clone(),
        ));
    }
}

async fn handle_connection(
    socket: TcpStream,
    state: Arc<AppState>,
    uids: Arc<Mutex<UidRegistry>>,
    token: CancellationToken,
) {
    let greeting = match Greeting::ok(None, "MailCrab IMAP4rev1 service ready") {
        Ok(greeting) => greeting,
        Err(_) => return,
    };

    let mut stream = Stream::insecure(socket);
    let mut server = Server::new(Options::default(), greeting);
    let mut session = Session::new(state, uids);

    // handle of the tagged response that concludes a LOGOUT, the connection
    // is closed once it has been flushed to the client
    let mut logout_handle: Option<ResponseHandle> = None;

    loop {
        let event = tokio::select! {
            event = stream.next(&mut server) => event,
            _ = token.cancelled() => break,
        };

        match event {
            Ok(Event::GreetingSent { .. }) => {}
            Ok(Event::CommandReceived { command }) => {
                if let CommandOutcome::Logout(handle) = session.handle_command(&mut server, command)
                {
                    logout_handle = Some(handle);
                }
            }
            Ok(Event::CommandAuthenticateReceived {
                command_authenticate,
            }) => {
                // any mechanism and any credentials are accepted; when the
                // client sent no initial response, request one continuation
                if command_authenticate.initial_response.is_some() {
                    session.authenticate(&mut server, command_authenticate.tag);
                } else {
                    session.pending_auth = Some(command_authenticate.tag);
                    let request = CommandContinuationRequest::basic(None, "continue")
                        .expect("static continuation request");
                    let _ = server.authenticate_continue(request);
                }
            }
            Ok(Event::AuthenticateDataReceived { authenticate_data }) => {
                match session.pending_auth.take() {
                    Some(tag) => match authenticate_data {
                        imap_next::imap_types::auth::AuthenticateData::Continue(_) => {
                            session.authenticate(&mut server, tag);
                        }
                        imap_next::imap_types::auth::AuthenticateData::Cancel => {
                            let status = Status::bad(Some(tag), None, "authentication cancelled")
                                .expect("static status");
                            let _ = server.authenticate_finish(status);
                        }
                    },
                    None => break,
                }
            }
            Ok(Event::IdleCommandReceived { tag }) => {
                let status =
                    Status::no(Some(tag), None, "IDLE not supported").expect("static status");
                let _ = server.idle_reject(status);
            }
            Ok(Event::IdleDoneReceived) => {}
            Ok(Event::ResponseSent { handle, .. }) => {
                if logout_handle == Some(handle) {
                    break;
                }
            }
            Err(StreamError::Closed) => break,
            Err(StreamError::Io(e)) => {
                debug!("IMAP connection error: {e}");
                break;
            }
            Err(StreamError::Tls(e)) => {
                debug!("IMAP TLS error: {e}");
                break;
            }
            Err(StreamError::State(e)) => match e {
                ServerError::ExpectedCrlfGotLf { .. } | ServerError::MalformedMessage { .. } => {
                    warn!("Received malformed IMAP command");
                    let status =
                        Status::bad(None, None, "malformed command").expect("static status");
                    server.enqueue_status(status);
                }
                ServerError::LiteralTooLong { .. } | ServerError::CommandTooLong { .. } => {
                    warn!("Received too long IMAP command");
                    let status =
                        Status::bad(None, None, "command too long").expect("static status");
                    server.enqueue_status(status);
                }
            },
        }
    }
}
