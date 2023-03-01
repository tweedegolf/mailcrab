use axum::{
    extract::{
        ws::{self, WebSocket},
        Path, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Extension, Json, Router,
};
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
        let mut active = true;

        while active {
            tokio::select! {
                internal_received = receive.recv() => {
                    match internal_received {
                        Ok(message) => {
                            let metadata: MailMessageMetadata = message.into();
                            match serde_json::to_string(&metadata) {
                                Ok(json) => {
                                    if socket.send(ws::Message::Text(json)).await.is_err() {
                                        event!(Level::INFO, "WS client disconnected");
                                        active = false;
                                    }
                                },
                                Err(e) => {
                                    event!(Level::ERROR, "could not convert message to json {:?}", e);
                                }
                            }
                        },
                        Err(e) => {
                            event!(Level::ERROR, "event pipeline error {:?}", e);
                            active = false;
                        }
                    }
                },
                socket_received = socket.recv() => {
                    match socket_received {
                        Some(Ok(ws::Message::Text(action))) => {
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
                        },
                        Some(Ok(ws::Message::Close(_))) | None => {
                            event!(Level::INFO, "websocket closed");
                            active = false;
                        },
                        Some(Err(e)) => {
                            event!(Level::WARN, "websocket error {:?}", e);
                            active = false;
                        },
                        Some(Ok(other_message)) => {
                            event!(Level::INFO, "received unexpected message {:?}", other_message);
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

/// return message body (html/text)
async fn message_body_handler(
    Path(id): Path<Uuid>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    if let Ok(storage) = state.storage.read() {
        match storage.get(&id) {
            Some(message) => Ok(Html(message.body())),
            _ => Err(StatusCode::NOT_FOUND),
        }
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

async fn index(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    Html(state.index.clone())
}

pub async fn http_server(app_state: Arc<AppState>, port: u16) {
    let serve_dir = ServeDir::new("dist");

    let router = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/api/messages", get(messages_handler))
        .route("/api/message/:id", get(message_handler))
        .route("/api/message/:id/body", get(message_body_handler))
        .nest_service("/static", serve_dir);

    let mut app = Router::new()
        .nest(app_state.prefix.as_str(), router.clone())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    if &app_state.prefix != "/" {
        app = app.nest("/", router);
    }

    app = app.layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    if let Err(e) = server.await {
        event!(Level::ERROR, "Server error :{:?}", e);
    }
}
