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

    messages.sort_by(|a, b| a.time.cmp(&b.time));

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
