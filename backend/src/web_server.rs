use axum::{
    extract::{
        ws::{self, WebSocket},
        Path, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Extension, Json, Router,
};
use std::io;
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{event, Level};
use uuid::Uuid;

use crate::{
    types::{Action, MailMessage, MailMessageMetadata},
    AppState,
};

/// send mail message metadata to websocket clients when broadcaster by the SMTP server
async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|mut socket: WebSocket| async move {
        let mut receive = state.rx.resubscribe();

        loop {
            tokio::select! {
                Ok(message) = receive.recv() => {
                    let metadata: MailMessageMetadata = message.into();
                    match serde_json::to_string(&metadata) {
                        Ok(json) => {
                            if socket.send(ws::Message::Text(json)).await.is_err() {
                                event!(Level::INFO, "WS client disconnected");
                                return;
                            }
                        },
                        Err(e) => {
                            event!(Level::ERROR, "Could not convert message to json {:?}", e);
                        }
                    }
                },
                Some(Ok(ws::Message::Text(action))) = socket.recv() => {
                    match serde_json::from_str(action.as_str()) {
                        Ok(Action::RemoveAll) => if let Ok(mut storage) = state.storage.write() {
                            storage.clear();
                            event!(Level::INFO, "storage cleared");
                        },
                        Ok(Action::Open(id)) => if let Ok(mut storage) = state.storage.write() {
                            if let Some(message) = storage.get_mut(&id) {
                                message.open();
                                event!(Level::INFO, "message {} opened", &id);
                            }
                        },
                        Ok(Action::Remove(id)) => if let Ok(mut storage) = state.storage.write() {
                            if storage.remove(&id).is_some() {
                                event!(Level::INFO, "message {} removed", &id);
                            }
                        },
                        msg => {
                            event!(Level::WARN, "unknown action {:?}", msg);
                        },
                    }
                }
            };
        }
    })
}

/// return metadata of all currently stored messages
async fn messages_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<Vec<MailMessageMetadata>>, StatusCode> {
    if let Ok(storage) = state.storage.read() {
        let messages = storage
            .iter()
            .map(|(_, message)| message.clone().into())
            .collect::<Vec<MailMessageMetadata>>();

        Ok(Json(messages))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// return full message with attachments
async fn message_handler(
    Path(id): Path<Uuid>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<MailMessage>, StatusCode> {
    if let Ok(storage) = state.storage.read() {
        match storage.get(&id) {
            Some(message) => Ok(Json(message.clone())),
            _ => Err(StatusCode::NOT_FOUND),
        }
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// static serve error handler
async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Error")
}

pub async fn http_serve(app_state: Arc<AppState>, port: u16) {
    let static_serve = ServeDir::new("dist").append_index_html_on_directories(true);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/messages", get(messages_handler))
        .route("/api/message/:id", get(message_handler))
        .fallback(get_service(static_serve).handle_error(handle_error))
        .layer(Extension(app_state))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    if let Err(e) = server.await {
        event!(Level::ERROR, "Server error :{:?}", e);
    }
}
