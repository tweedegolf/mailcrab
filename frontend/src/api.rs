use gloo_net::http::Request;

use crate::types::{MailMessage, MailMessageMetadata};

pub fn get_api_path(path: &str) -> String {
    let mut pathname = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();

    pathname.push_str("/api/");
    pathname.push_str(path);

    pathname
}

pub async fn fetch_messages_metadata() -> Vec<MailMessageMetadata> {
    let mut messages: Vec<MailMessageMetadata> = Request::get(&get_api_path("messages"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    messages.sort_by_key(|a| a.time);

    messages
}

pub async fn fetch_message(id: &str) -> MailMessage {
    let mut url = get_api_path("message/");
    url.push_str(id);

    Request::get(&url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn fetch_raw(id: &str) -> String {
    let url = get_api_path(&format!("message/{}/raw", id));

    let response = match Request::get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Failed to load raw message: {e}"),
    };

    response
        .text()
        .await
        .unwrap_or_else(|e| format!("Failed to read raw message: {e}"))
}
