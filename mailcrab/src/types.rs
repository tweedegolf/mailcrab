use base64ct::Encoding;
use chrono::{DateTime, Local};
use mail_parser::MimeHeaders;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;
use uuid::Uuid;

use crate::error::Error;

pub type MessageId = Uuid;

#[derive(Deserialize, Debug)]
pub enum Action {
    RemoveAll,
    #[allow(unused)]
    Remove(MessageId),
    #[allow(unused)]
    Open(MessageId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentMetadata {
    filename: String,
    mime: String,
    size: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailMessageMetadata {
    pub id: MessageId,
    from: Address,
    to: Vec<Address>,
    subject: String,
    pub time: i64,
    date: String,
    size: String,
    opened: bool,
    pub has_html: bool,
    pub has_plain: bool,
    pub attachments: Vec<AttachmentMetadata>,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

impl From<MailMessage> for MailMessageMetadata {
    fn from(message: MailMessage) -> Self {
        let MailMessage {
            id,
            from,
            to,
            subject,
            time,
            date,
            size,
            html,
            text,
            opened,
            attachments,
            envelope_from,
            envelope_recipients,
            ..
        } = message;
        MailMessageMetadata {
            id,
            from,
            to,
            subject,
            time,
            date,
            size,
            has_html: !html.is_empty(),
            has_plain: !text.is_empty(),
            opened,
            attachments: attachments
                .into_iter()
                .map(|a| AttachmentMetadata {
                    filename: a.filename,
                    mime: a.mime,
                    size: a.size,
                })
                .collect::<Vec<AttachmentMetadata>>(),
            envelope_from,
            envelope_recipients,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Attachment {
    filename: String,
    content_id: Option<String>,
    mime: String,
    size: String,
    content: String,
}

impl From<&mail_parser::MessagePart<'_>> for Attachment {
    fn from(part: &mail_parser::MessagePart) -> Self {
        let filename = part.attachment_name().unwrap_or_default().to_string();
        let mime = match part.content_type() {
            Some(content_type) => match &content_type.c_subtype {
                Some(subtype) => format!("{}/{}", content_type.c_type, subtype),
                None => content_type.c_type.to_string(),
            },
            None => "application/octet-stream".to_owned(),
        };

        Attachment {
            filename,
            mime,
            content_id: part.content_id().map(|s| s.to_owned()),
            size: humansize::format_size(part.contents().len(), humansize::DECIMAL),
            content: base64ct::Base64::encode_string(part.contents()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Address {
    name: Option<String>,
    email: Option<String>,
}

impl From<&mail_parser::Addr<'_>> for Address {
    fn from(addr: &mail_parser::Addr) -> Self {
        Address {
            name: addr.name.clone().map(|v| v.to_string()),
            email: addr.address.clone().map(|v| v.to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct MailMessage {
    pub id: MessageId,
    pub time: i64,
    from: Address,
    to: Vec<Address>,
    subject: String,
    date: String,
    size: String,
    opened: bool,
    headers: HashMap<String, String>,
    text: String,
    html: String,
    attachments: Vec<Attachment>,
    raw: String,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

impl MailMessage {
    pub fn open(&mut self) {
        self.opened = true;
    }

    pub fn render(&self) -> String {
        if self.html.is_empty() {
            self.text.clone()
        } else {
            let mut html = self.html.clone();

            for attachement in &self.attachments {
                if let Some(content_id) = &attachement.content_id {
                    let from = format!("cid:{}", content_id.trim_start_matches("cid:"));
                    let encoded: String = attachement
                        .content
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    let to = format!("data:{};base64,{}", attachement.mime, encoded);

                    html = html.replace(&from, &to);
                }
            }

            html
        }
    }
}

impl TryFrom<mail_parser::Message<'_>> for MailMessage {
    type Error = Error;

    fn try_from(message: mail_parser::Message) -> Result<Self, Self::Error> {
        let from = match message.from().and_then(|f| f.first()) {
            Some(addr) => addr.into(),
            _ => {
                warn!("Could not parse 'From' address header, setting placeholder address.");

                Address {
                    name: Some("No from header".to_string()),
                    email: Some("no-from-header@example.com".to_string()),
                }
            }
        };

        let to = match message.to().and_then(|a| a.as_list()) {
            Some(list) => list
                .iter()
                .map(|addr| addr.into())
                .collect::<Vec<Address>>(),
            _ => {
                warn!("Could not parse 'To' address header, setting placeholder address.");

                vec![Address {
                    name: Some("No to header".to_string()),
                    email: Some("no-to-header@example.com".to_string()),
                }]
            }
        };

        let subject = message.subject().unwrap_or_default().to_owned();

        let text = match message
            .text_bodies()
            .find(|p| p.is_text() && !p.is_text_html())
        {
            Some(item) => item.to_string(),
            _ => Default::default(),
        };

        let html = match message.html_bodies().find(|p| p.is_text_html()) {
            Some(item) => item.to_string(),
            _ => Default::default(),
        };

        let attachments = message
            .attachments()
            .map(|attachement| attachement.into())
            .collect::<Vec<Attachment>>();

        let date: DateTime<Local> = match message.date() {
            Some(date) => match DateTime::parse_from_rfc2822(date.to_rfc3339().as_str()) {
                Ok(date_time) => date_time.into(),
                _ => Local::now(),
            },
            None => Local::now(),
        };

        let raw = base64ct::Base64::encode_string(&message.raw_message);

        let mut headers = HashMap::<String, String>::new();

        for (key, value) in message.headers_raw() {
            headers.insert(key.to_string(), value.to_string());
        }

        let size = humansize::format_size(message.raw_message.len(), humansize::DECIMAL);

        Ok(MailMessage {
            id: Uuid::new_v4(),
            from,
            to,
            subject,
            time: date.timestamp(),
            date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
            size,
            text,
            html,
            opened: false,
            attachments,
            raw,
            headers,
            ..MailMessage::default()
        })
    }
}
