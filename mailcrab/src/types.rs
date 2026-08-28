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
    pub filename: String,
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
    #[serde(default)]
    pub parse_warnings: Vec<String>,
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
            parse_warnings,
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
            parse_warnings,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Attachment {
    filename: String,
    content_id: Option<String>,
    mime: String,
    size: String,
    #[serde(skip)]
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
    pub attachments: Vec<Attachment>,
    #[serde(skip)]
    raw: String,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
    pub parse_warnings: Vec<String>,
}

impl MailMessage {
    pub fn open(&mut self) {
        self.opened = true;
    }

    pub fn is_opened(&self) -> bool {
        self.opened
    }

    pub fn set_opened(&mut self, opened: bool) {
        self.opened = opened;
    }

    pub fn raw_bytes(&self) -> Option<Vec<u8>> {
        base64ct::Base64::decode_vec(&self.raw).ok()
    }

    pub fn attachment_content(&self, index: usize) -> Option<(String, String, Vec<u8>)> {
        let a = self.attachments.get(index)?;
        let bytes = base64ct::Base64::decode_vec(&a.content).ok()?;
        Some((a.filename.clone(), a.mime.clone(), bytes))
    }

    pub fn render(&self, prefix: &str) -> String {
        if self.html.is_empty() {
            return self.text.clone();
        }

        let prefix = prefix.trim_end_matches('/');
        let mut html = self.html.clone();

        for (index, attachment) in self.attachments.iter().enumerate() {
            if let Some(content_id) = &attachment.content_id {
                let cid = format!("cid:{}", content_id.trim_start_matches("cid:"));
                let url = format!("{}/api/message/{}/attachment/{}", prefix, self.id, index);
                html = html.replace(&cid, &url);
            }
        }

        html
    }
}

/// Check whether a raw message contains the given byte sequence
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Detect MIME format problems that mail_parser recovers from silently,
/// so they can be surfaced in the interface as warnings
fn mime_format_warnings(message: &mail_parser::Message) -> Vec<String> {
    let raw: &[u8] = message.raw_message.as_ref();
    let mut warnings = Vec::new();

    for part in &message.parts {
        let Some(content_type) = part.content_type() else {
            continue;
        };

        if !content_type.c_type.eq_ignore_ascii_case("multipart") {
            continue;
        }

        let subtype = format!(
            "multipart/{}",
            content_type.c_subtype.as_deref().unwrap_or("mixed")
        );

        let Some(boundary) = content_type.attribute("boundary") else {
            warnings.push(format!(
                "{subtype} part is missing a boundary attribute in its Content-Type header"
            ));
            continue;
        };

        let delimiter = format!("--{boundary}");
        let terminator = format!("--{boundary}--");

        if !contains(raw, terminator.as_bytes()) {
            if contains(raw, delimiter.as_bytes()) {
                warnings.push(format!(
                    "{subtype} part is missing its terminating boundary \"{terminator}\""
                ));
            } else {
                warnings.push(format!(
                    "{subtype} part declares boundary \"{boundary}\", but it never occurs in the message body"
                ));
            }
        }
    }

    warnings
}

impl TryFrom<mail_parser::Message<'_>> for MailMessage {
    type Error = Error;

    fn try_from(message: mail_parser::Message) -> Result<Self, Self::Error> {
        let mut parse_warnings = mime_format_warnings(&message);

        let from = match message.from().and_then(|f| f.first()) {
            Some(addr) => addr.into(),
            _ => {
                warn!("Could not parse 'From' address header, setting placeholder address.");
                parse_warnings.push("could not parse the 'From' address header".to_string());

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
                parse_warnings.push("could not parse the 'To' address header".to_string());

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
            parse_warnings,
            ..MailMessage::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MailMessage;
    use mail_parser::MessageParser;

    fn parse(raw: &str) -> MailMessage {
        MessageParser::default()
            .parse(raw.as_bytes())
            .expect("failed to parse message")
            .try_into()
            .expect("failed to convert message")
    }

    #[test]
    fn well_formed_multipart_message_has_no_warnings() {
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "From: Sender <sender@example.com>\r\n",
            "To: Receiver <receiver@example.com>\r\n",
            "Date: Sat, 01 Aug 2026 23:20:55 +0200\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/alternative; boundary=test-boundary-123\r\n",
            "\r\n",
            "--test-boundary-123\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Hello world!\r\n",
            "\r\n",
            "--test-boundary-123\r\n",
            "Content-Type: text/html\r\n",
            "\r\n",
            "<html><body><p>Hello world!</p></body></html>\r\n",
            "\r\n",
            "--test-boundary-123--\r\n",
        ));

        assert!(
            message.parse_warnings.is_empty(),
            "unexpected warnings: {:?}",
            message.parse_warnings
        );
        assert!(message.text.contains("Hello world!"));
        assert!(message.html.contains("<p>Hello world!</p>"));
    }

    #[test]
    fn missing_terminating_boundary_yields_warning() {
        // multipart/alternative message that ends after the last body part,
        // without the required "--boundary--" terminator
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "From: Sender <sender@example.com>\r\n",
            "To: Receiver <receiver@example.com>\r\n",
            "Date: Sat, 01 Aug 2026 23:20:55 +0200\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/alternative; boundary=test-boundary-123\r\n",
            "\r\n",
            "--test-boundary-123\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "Content-Transfer-Encoding: quoted-printable\r\n",
            "\r\n",
            "Hello world!\r\n",
            "\r\n",
            "--test-boundary-123\r\n",
            "Content-Type: text/html\r\n",
            "Content-Transfer-Encoding: quoted-printable\r\n",
            "\r\n",
            "<html><body><p>Hello world!</p></body></html>\r\n",
        ));

        assert_eq!(message.parse_warnings.len(), 1);
        assert_eq!(
            message.parse_warnings[0],
            "multipart/alternative part is missing its terminating boundary \"--test-boundary-123--\""
        );

        // the plain text body is still recovered, but mail_parser does not
        // classify the truncated trailing part as an HTML body: the HTML
        // version of the message is silently lost — which is exactly why the
        // warning should be shown in the interface
        assert!(message.text.contains("Hello world!"));
        assert!(message.html.is_empty());
    }

    #[test]
    fn unused_boundary_yields_warning() {
        // the declared boundary never occurs in the body
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "From: Sender <sender@example.com>\r\n",
            "To: Receiver <receiver@example.com>\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=test-boundary-123\r\n",
            "\r\n",
            "Hello world!\r\n",
        ));

        assert_eq!(
            message.parse_warnings,
            vec![
                "multipart/mixed part declares boundary \"test-boundary-123\", but it never occurs in the message body"
                    .to_string()
            ]
        );
    }

    #[test]
    fn missing_inner_terminating_boundary_yields_warning() {
        // nested multipart where only the inner terminator is missing
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "From: Sender <sender@example.com>\r\n",
            "To: Receiver <receiver@example.com>\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=outer-boundary\r\n",
            "\r\n",
            "--outer-boundary\r\n",
            "Content-Type: multipart/alternative; boundary=inner-boundary\r\n",
            "\r\n",
            "--inner-boundary\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Hello world!\r\n",
            "\r\n",
            "--outer-boundary--\r\n",
        ));

        assert_eq!(
            message.parse_warnings,
            vec![
                "multipart/alternative part is missing its terminating boundary \"--inner-boundary--\""
                    .to_string()
            ]
        );
        assert!(message.text.contains("Hello world!"));
    }

    #[test]
    fn missing_from_and_to_yields_warnings_and_placeholders() {
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Hello world!\r\n",
        ));

        assert_eq!(
            message.parse_warnings,
            vec![
                "could not parse the 'From' address header".to_string(),
                "could not parse the 'To' address header".to_string(),
            ]
        );
        assert_eq!(
            message.from.email.as_deref(),
            Some("no-from-header@example.com")
        );
    }

    #[test]
    fn warnings_are_included_in_metadata() {
        let message = parse(concat!(
            "Subject: Test message\r\n",
            "From: Sender <sender@example.com>\r\n",
            "To: Receiver <receiver@example.com>\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=test-boundary-123\r\n",
            "\r\n",
            "--test-boundary-123\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Hello world!\r\n",
        ));

        let metadata: super::MailMessageMetadata = message.into();

        assert_eq!(metadata.parse_warnings.len(), 1);
        assert!(metadata.parse_warnings[0].contains("terminating boundary"));
    }
}
