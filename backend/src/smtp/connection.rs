use mailin::{Action, Response, Session, SessionBuilder};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tracing::debug;

use super::{handler::MailHandler, server::TlsConfig, Result};

#[derive(Debug, PartialEq)]
enum SessionResult {
    Finished,
    UpgradeTls,
}

/// write message to client
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

// handle SMTP messages over a stream
async fn handle_steam<S>(
    mut stream: &mut BufReader<S>,
    session: &mut Session<MailHandler>,
) -> Result<SessionResult>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(80);
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

// convert a TCP stream to a TLS stream
async fn upgrade_connection(
    stream: TcpStream,
    acceptor: &TlsAcceptor,
) -> Result<BufReader<TlsStream<TcpStream>>> {
    let accept_buffer = acceptor.accept(stream).await.map_err(|e| e.to_string())?;

    Ok(BufReader::new(accept_buffer))
}

/// handle SMTP connections, optionally upgrade to TLS, either directly or after negotiation
pub(super) async fn handle_connection(
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
