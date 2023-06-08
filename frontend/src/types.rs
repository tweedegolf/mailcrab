use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Address {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct AttachmentMetadata {
    pub filename: String,
    pub mime: String,
    pub size: String,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct MailMessageMetadata {
    pub id: String,
    pub from: Address,
    pub to: Vec<Address>,
    pub subject: String,
    pub time: u64,
    pub date: String,
    pub size: String,
    pub opened: bool,
    pub attachments: Vec<AttachmentMetadata>,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_id: Option<String>,
    pub mime: String,
    pub size: String,
    pub content: String,
}

#[derive(Clone, PartialEq, Eq, Deserialize, Default)]
pub struct MailMessage {
    pub id: String,
    pub from: Address,
    pub to: Vec<Address>,
    pub subject: String,
    pub time: u64,
    pub date: String,
    pub size: String,
    pub opened: bool,
    pub text: String,
    pub html: String,
    pub attachments: Vec<Attachment>,
    pub raw: String,
    pub headers: HashMap<String, String>,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

#[derive(Serialize, Debug)]
pub enum Action {
    RemoveAll,
    Remove(String),
    Open(String),
}
