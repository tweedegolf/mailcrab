use gloo_net::http::Request;

use crate::types::{MailMessage, MailMessageMetadata};

pub async fn fetch_messages_metadata() -> Vec<MailMessageMetadata> {
    let mut messages: Vec<MailMessageMetadata> = Request::get("/api/messages")
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
    let mut url = String::from("/api/message/");
    url.push_str(id);

    Request::get(&url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
