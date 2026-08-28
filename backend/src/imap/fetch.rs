use imap_next::imap_types::{
    body::{BasicFields, Body, BodyStructure, SpecificFields},
    core::{IString, Literal, NString, Vec1},
    datetime::DateTime as ImapDateTime,
    envelope::{Address, Envelope},
    fetch::{MessageDataItem, MessageDataItemName, Section},
};
use mail_parser::{Message, MessageParser, MessagePart, MimeHeaders, PartType};
use mailcrab::MailMessage;
use std::num::NonZeroU32;

use super::session::flag_fetch;

/// build the data items of a FETCH response for a single message
pub(super) fn fetch_items(
    names: &[MessageDataItemName<'static>],
    mail: &MailMessage,
    uid: NonZeroU32,
    deleted: bool,
    include_uid: bool,
) -> Vec1<MessageDataItem<'static>> {
    let raw = mail.raw_bytes().unwrap_or_default();
    let parsed = MessageParser::default().parse(&raw);

    let mut items = Vec::with_capacity(names.len() + 1);
    let mut has_uid = false;

    for name in names {
        match name {
            MessageDataItemName::Flags => {
                items.push(MessageDataItem::Flags(flag_fetch(
                    mail.is_opened(),
                    deleted,
                )));
            }
            MessageDataItemName::Uid => {
                has_uid = true;
                items.push(MessageDataItem::Uid(uid));
            }
            MessageDataItemName::Rfc822Size => {
                items.push(MessageDataItem::Rfc822Size(raw.len() as u32));
            }
            MessageDataItemName::InternalDate => {
                items.push(MessageDataItem::InternalDate(internal_date(mail.time)));
            }
            MessageDataItemName::Envelope => {
                items.push(MessageDataItem::Envelope(envelope(parsed.as_ref())));
            }
            MessageDataItemName::Body => {
                items.push(MessageDataItem::Body(body_structure_root(parsed.as_ref())));
            }
            MessageDataItemName::BodyStructure => {
                items.push(MessageDataItem::BodyStructure(body_structure_root(
                    parsed.as_ref(),
                )));
            }
            MessageDataItemName::Rfc822 => {
                items.push(MessageDataItem::Rfc822(nstring_bytes(raw.clone())));
            }
            MessageDataItemName::Rfc822Header => {
                items.push(MessageDataItem::Rfc822Header(nstring_bytes(
                    header_block(parsed.as_ref(), &raw).to_vec(),
                )));
            }
            MessageDataItemName::Rfc822Text => {
                items.push(MessageDataItem::Rfc822Text(nstring_bytes(
                    text_block(parsed.as_ref(), &raw).to_vec(),
                )));
            }
            MessageDataItemName::BodyExt {
                section, partial, ..
            } => {
                let mut data = section_bytes(parsed.as_ref(), &raw, section.as_ref());

                let origin = partial.map(|(start, _)| start);
                if let Some((start, length)) = partial {
                    let start = *start as usize;
                    let length = length.get() as usize;
                    data = if start >= data.len() {
                        Vec::new()
                    } else {
                        data[start..(start + length).min(data.len())].to_vec()
                    };
                }

                items.push(MessageDataItem::BodyExt {
                    section: section.clone(),
                    origin,
                    data: nstring_bytes(data),
                });
            }
            // binary fetches (RFC 3516) are not advertised
            _ => {}
        }
    }

    if include_uid && !has_uid {
        items.push(MessageDataItem::Uid(uid));
    }

    if items.is_empty() {
        items.push(MessageDataItem::Flags(flag_fetch(
            mail.is_opened(),
            deleted,
        )));
    }

    Vec1::unvalidated(items)
}

fn internal_date(timestamp: i64) -> ImapDateTime {
    let date = chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_default()
        .fixed_offset();

    ImapDateTime::unvalidated(date)
}

/// an NString containing arbitrary (non-NUL) bytes, sent as a literal
fn nstring_bytes(mut value: Vec<u8>) -> NString<'static> {
    if value.contains(&0) {
        value.retain(|byte| *byte != 0);
    }

    match Literal::try_from(value) {
        Ok(literal) => NString::from(literal),
        Err(_) => NString::NIL,
    }
}

fn nstring(value: Option<&str>) -> NString<'static> {
    match value {
        Some(value) => NString::try_from(value.trim().replace('\0', "")).unwrap_or(NString::NIL),
        None => NString::NIL,
    }
}

fn istring(value: &str) -> IString<'static> {
    IString::try_from(value.replace('\0', ""))
        .unwrap_or_else(|_| IString::try_from("unknown".to_string()).expect("valid string"))
}

fn raw_header<'x>(message: &'x Message, name: &str) -> Option<&'x str> {
    message
        .headers_raw()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

fn address_list(address: Option<&mail_parser::Address>) -> Vec<Address<'static>> {
    let Some(list) = address.and_then(|address| address.as_list()) else {
        return Vec::new();
    };

    list.iter()
        .map(|addr| {
            let (mailbox, host) = match addr.address.as_deref() {
                Some(email) => match email.split_once('@') {
                    Some((mailbox, host)) => (Some(mailbox), Some(host)),
                    None => (Some(email), None),
                },
                None => (None, None),
            };

            Address {
                name: nstring(addr.name.as_deref()),
                adl: NString::NIL,
                mailbox: nstring(mailbox),
                host: nstring(host),
            }
        })
        .collect()
}

fn envelope(message: Option<&Message>) -> Envelope<'static> {
    let Some(message) = message else {
        return Envelope {
            date: NString::NIL,
            subject: NString::NIL,
            from: Vec::new(),
            sender: Vec::new(),
            reply_to: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            in_reply_to: NString::NIL,
            message_id: NString::NIL,
        };
    };

    let from = address_list(message.from());
    let sender = match address_list(message.sender()) {
        list if list.is_empty() => from.clone(),
        list => list,
    };
    let reply_to = match address_list(message.reply_to()) {
        list if list.is_empty() => from.clone(),
        list => list,
    };

    Envelope {
        date: nstring(raw_header(message, "date")),
        subject: nstring(message.subject()),
        from,
        sender,
        reply_to,
        to: address_list(message.to()),
        cc: address_list(message.cc()),
        bcc: address_list(message.bcc()),
        in_reply_to: nstring(raw_header(message, "in-reply-to")),
        message_id: nstring(raw_header(message, "message-id")),
    }
}

fn slice(raw: &[u8], from: u32, to: u32) -> &[u8] {
    let from = (from as usize).min(raw.len());
    let to = (to as usize).clamp(from, raw.len());

    &raw[from..to]
}

fn count_lines(data: &[u8]) -> u32 {
    data.iter().filter(|byte| **byte == b'\n').count() as u32
}

/// the header section of a message: everything up to and including the blank
/// line that separates the header from the body
fn header_block<'x>(message: Option<&Message>, raw: &'x [u8]) -> &'x [u8] {
    let end = message
        .and_then(|message| message.parts.first())
        .map(|part| part.offset_body as usize)
        .filter(|end| *end > 0 && *end <= raw.len())
        .or_else(|| {
            raw.windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| position + 4)
        })
        .unwrap_or(raw.len());

    &raw[..end]
}

fn text_block<'x>(message: Option<&Message>, raw: &'x [u8]) -> &'x [u8] {
    let start = header_block(message, raw).len();

    &raw[start..]
}

/// keep (or drop) the named header fields from a raw header block
fn filter_header_fields(block: &[u8], fields: &[impl AsRef<[u8]>], keep: bool) -> Vec<u8> {
    let names = fields
        .iter()
        .map(|field| field.as_ref().to_ascii_lowercase())
        .collect::<Vec<_>>();

    let mut output = Vec::new();
    let mut keep_current = false;

    for line in block.split_inclusive(|byte| *byte == b'\n') {
        // the blank line ends the header block
        if line == b"\r\n" || line == b"\n" {
            break;
        }

        // continuation lines belong to the previous header field
        let is_continuation = matches!(line.first(), Some(b' ') | Some(b'\t'));
        if !is_continuation {
            let name = line
                .split(|byte| *byte == b':')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            keep_current = names.contains(&name) == keep;
        }

        if keep_current {
            output.extend_from_slice(line);
        }
    }

    output.extend_from_slice(b"\r\n");

    output
}

/// resolve an IMAP part path (like `1.2`) to a message and part index; a path
/// element addresses the n-th child of a multipart part, descends into an
/// embedded message/rfc822, or - as `1` - the body of a non-multipart message
fn resolve_part<'x>(
    message: &'x Message<'x>,
    path: &[NonZeroU32],
) -> Option<(&'x Message<'x>, usize)> {
    let mut message = message;
    let mut part_id = 0;

    for (position, element) in path.iter().enumerate() {
        let element = element.get() as usize;
        let part = message.parts.get(part_id)?;

        match &part.body {
            PartType::Multipart(children) => {
                part_id = *children.get(element - 1)? as usize;
            }
            PartType::Message(inner) => {
                message = inner;
                let root = message.parts.first()?;

                match &root.body {
                    PartType::Multipart(children) => {
                        part_id = *children.get(element - 1)? as usize;
                    }
                    _ => {
                        if element != 1 {
                            return None;
                        }
                        part_id = 0;
                    }
                }
            }
            _ => {
                // a non-multipart body is addressable as part 1 only
                if element != 1 || position != path.len() - 1 {
                    return None;
                }
            }
        }
    }

    Some((message, part_id))
}

fn part_body<'x>(message: &'x Message<'x>, part_id: usize) -> &'x [u8] {
    let raw = message.raw_message.as_ref();

    match message.parts.get(part_id) {
        Some(part) => slice(raw, part.offset_body, part.offset_end),
        None => &[],
    }
}

fn part_header<'x>(message: &'x Message<'x>, part_id: usize) -> &'x [u8] {
    let raw = message.raw_message.as_ref();

    match message.parts.get(part_id) {
        Some(part) => slice(raw, part.offset_header, part.offset_body),
        None => &[],
    }
}

/// resolve a part path to an embedded message/rfc822 message, for the
/// `BODY[n.HEADER]` and `BODY[n.TEXT]` section forms
fn resolve_embedded_message<'x>(
    message: &'x Message<'x>,
    path: &[NonZeroU32],
) -> Option<&'x Message<'x>> {
    let (message, part_id) = resolve_part(message, path)?;

    match &message.parts.get(part_id)?.body {
        PartType::Message(inner) => Some(inner),
        _ => None,
    }
}

/// the raw bytes of a BODY[<section>] fetch
fn section_bytes(message: Option<&Message>, raw: &[u8], section: Option<&Section>) -> Vec<u8> {
    match section {
        None => raw.to_vec(),
        Some(Section::Header(None)) => header_block(message, raw).to_vec(),
        Some(Section::Text(None)) => text_block(message, raw).to_vec(),
        Some(Section::HeaderFields(None, fields)) => {
            filter_header_fields(header_block(message, raw), fields.as_ref(), true)
        }
        Some(Section::HeaderFieldsNot(None, fields)) => {
            filter_header_fields(header_block(message, raw), fields.as_ref(), false)
        }
        Some(Section::Part(part)) => message
            .and_then(|message| resolve_part(message, part.0.as_ref()))
            .map(|(message, part_id)| part_body(message, part_id).to_vec())
            .unwrap_or_default(),
        Some(Section::Mime(part)) => message
            .and_then(|message| resolve_part(message, part.0.as_ref()))
            .map(|(message, part_id)| part_header(message, part_id).to_vec())
            .unwrap_or_default(),
        Some(Section::Header(Some(part))) => message
            .and_then(|message| resolve_embedded_message(message, part.0.as_ref()))
            .map(|inner| header_block(Some(inner), inner.raw_message.as_ref()).to_vec())
            .unwrap_or_default(),
        Some(Section::Text(Some(part))) => message
            .and_then(|message| resolve_embedded_message(message, part.0.as_ref()))
            .map(|inner| text_block(Some(inner), inner.raw_message.as_ref()).to_vec())
            .unwrap_or_default(),
        Some(Section::HeaderFields(Some(part), fields)) => message
            .and_then(|message| resolve_embedded_message(message, part.0.as_ref()))
            .map(|inner| {
                filter_header_fields(
                    header_block(Some(inner), inner.raw_message.as_ref()),
                    fields.as_ref(),
                    true,
                )
            })
            .unwrap_or_default(),
        Some(Section::HeaderFieldsNot(Some(part), fields)) => message
            .and_then(|message| resolve_embedded_message(message, part.0.as_ref()))
            .map(|inner| {
                filter_header_fields(
                    header_block(Some(inner), inner.raw_message.as_ref()),
                    fields.as_ref(),
                    false,
                )
            })
            .unwrap_or_default(),
    }
}

fn body_structure_root(message: Option<&Message>) -> BodyStructure<'static> {
    match message {
        Some(message) => body_structure(message, 0),
        None => empty_text_structure(),
    }
}

fn empty_text_structure() -> BodyStructure<'static> {
    BodyStructure::Single {
        body: Body {
            basic: BasicFields {
                parameter_list: Vec::new(),
                id: NString::NIL,
                description: NString::NIL,
                content_transfer_encoding: istring("7bit"),
                size: 0,
            },
            specific: SpecificFields::Text {
                subtype: istring("plain"),
                number_of_lines: 0,
            },
        },
        extension_data: None,
    }
}

fn basic_fields(part: &MessagePart, size: u32) -> BasicFields<'static> {
    let mut parameter_list = Vec::new();

    if let Some(content_type) = part.content_type() {
        for name in ["charset", "name"] {
            if let Some(value) = content_type.attribute(name) {
                parameter_list.push((istring(name), istring(value)));
            }
        }
    }

    // clients expect the content id including its angle brackets
    let id = part.content_id().map(|id| {
        if id.starts_with('<') {
            id.to_string()
        } else {
            format!("<{id}>")
        }
    });

    BasicFields {
        parameter_list,
        id: nstring(id.as_deref()),
        description: nstring(part.content_description()),
        content_transfer_encoding: istring(part.content_transfer_encoding().unwrap_or("7bit")),
        size,
    }
}

/// derive the IMAP BODYSTRUCTURE from a parsed message part
fn body_structure(message: &Message, part_id: usize) -> BodyStructure<'static> {
    let Some(part) = message.parts.get(part_id) else {
        return empty_text_structure();
    };

    let raw = message.raw_message.as_ref();
    let body_slice = slice(raw, part.offset_body, part.offset_end);

    match &part.body {
        PartType::Multipart(children) => {
            let bodies = children
                .iter()
                .map(|child| body_structure(message, *child as usize))
                .collect::<Vec<_>>();
            let subtype = part
                .content_type()
                .and_then(|content_type| content_type.c_subtype.as_deref())
                .unwrap_or("mixed");

            match Vec1::try_from(bodies) {
                Ok(bodies) => BodyStructure::Multi {
                    bodies,
                    subtype: istring(subtype),
                    extension_data: None,
                },
                Err(_) => empty_text_structure(),
            }
        }
        PartType::Message(inner) => BodyStructure::Single {
            body: Body {
                basic: basic_fields(part, body_slice.len() as u32),
                specific: SpecificFields::Message {
                    envelope: Box::new(envelope(Some(inner))),
                    body_structure: Box::new(body_structure(inner, 0)),
                    number_of_lines: count_lines(body_slice),
                },
            },
            extension_data: None,
        },
        PartType::Text(_) | PartType::Html(_) => {
            let subtype = part
                .content_type()
                .and_then(|content_type| content_type.c_subtype.as_deref())
                .unwrap_or(if matches!(part.body, PartType::Html(_)) {
                    "html"
                } else {
                    "plain"
                });

            BodyStructure::Single {
                body: Body {
                    basic: basic_fields(part, body_slice.len() as u32),
                    specific: SpecificFields::Text {
                        subtype: istring(subtype),
                        number_of_lines: count_lines(body_slice),
                    },
                },
                extension_data: None,
            }
        }
        PartType::Binary(_) | PartType::InlineBinary(_) => {
            let (main_type, subtype) = match part.content_type() {
                Some(content_type) => (
                    content_type.c_type.to_string(),
                    content_type
                        .c_subtype
                        .as_deref()
                        .unwrap_or("octet-stream")
                        .to_string(),
                ),
                None => ("application".to_string(), "octet-stream".to_string()),
            };

            BodyStructure::Single {
                body: Body {
                    basic: basic_fields(part, body_slice.len() as u32),
                    specific: SpecificFields::Basic {
                        r#type: istring(&main_type),
                        subtype: istring(&subtype),
                    },
                },
                extension_data: None,
            }
        }
    }
}
