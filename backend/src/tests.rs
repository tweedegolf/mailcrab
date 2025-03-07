use fake::{
    Fake,
    faker::{
        company::en::{Buzzword, CatchPhrase},
        internet::en::SafeEmail,
        lorem::en::Paragraph,
        name::en::Name,
    },
};
use lettre::{
    Address, AsyncSmtpTransport, AsyncTransport, Message, SmtpTransport, Tokio1Executor, Transport,
    address::Envelope,
    message::{Attachment, MultiPart, SinglePart, header::ContentType},
    transport::smtp::response::Response,
};
use reqwest::Client;
use std::ffi::OsStr;
use tokio::time::{Duration, sleep};

use crate::{parse_env_var, run, types::MailMessageMetadata};

async fn send_message(
    with_html: bool,
    with_plain: bool,
    with_attachment: bool,
) -> Result<Response, Box<dyn std::error::Error>> {
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1".to_string())
        .port(smtp_port)
        .build();

    let to: String = SafeEmail().fake();
    let to_name: String = Name().fake();
    let from: String = SafeEmail().fake();
    let from_name: String = Name().fake();
    let body: String = [
        Paragraph(2..3).fake::<String>(),
        Paragraph(2..3).fake::<String>(),
        Paragraph(2..3).fake::<String>(),
    ]
    .join("\n");
    let html: String = format!(
        "{}\n<p><a href=\"https://github.com/tweedegolf/mailcrab\">external link</a></p>",
        body.replace('\n', "<br>\n")
    );

    let builder = Message::builder()
        .from(format!("{from_name} <{from}>",).parse()?)
        .to(format!("{to_name} <{to}>").parse()?)
        .subject(CatchPhrase().fake::<String>());

    let mut multipart = MultiPart::mixed().build();

    match (with_html, with_plain) {
        (true, true) => {
            multipart = multipart.multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(body))
                    .singlepart(SinglePart::html(html)),
            );
        }
        (false, true) => {
            multipart = multipart.singlepart(SinglePart::plain(body));
        }
        (true, false) => {
            multipart = multipart.singlepart(SinglePart::html(html));
        }
        _ => panic!("Email should have html or plain body"),
    };

    if with_attachment {
        let filebody = std::fs::read("blank.pdf")?;
        let content_type = ContentType::parse("application/pdf")?;
        let filename = format!("{}.pdf", Buzzword().fake::<&str>().to_ascii_lowercase());
        let attachment = Attachment::new(filename).body(filebody.clone(), content_type.clone());
        multipart = multipart.singlepart(attachment);
    }

    let email = builder.multipart(multipart)?;

    let response = mailer.send(email).await?;

    Ok(response)
}

async fn get_messages_metadata() -> Result<Vec<MailMessageMetadata>, Box<dyn std::error::Error>> {
    let http_port: u16 = parse_env_var("HTTP_PORT", 1080);

    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    let mails: Vec<MailMessageMetadata> = client
        .get(format!("http://127.0.0.1:{http_port}/api/messages"))
        .send()
        .await?
        .json()
        .await?;

    Ok(mails)
}

async fn test_receive_messages() -> Result<Vec<Response>, Box<dyn std::error::Error>> {
    let mut responses = vec![];

    responses.push(send_message(true, true, false).await?);
    responses.push(send_message(true, false, false).await?);
    responses.push(send_message(false, true, true).await?);

    Ok(responses)
}

#[tokio::test]
async fn receive_message() {
    let join = tokio::task::spawn(run());

    // wait for mailcrab to startup
    for _i in 0..60 {
        if get_messages_metadata().await.is_ok() {
            break;
        }

        sleep(Duration::from_millis(100)).await;
    }

    // send messages and retrieve the message id from mailcrab
    let responses = test_receive_messages()
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            r.message()
                .next()
                .unwrap_or_default()
                .split_ascii_whitespace()
                .last()
                .unwrap_or_default()
                .to_owned()
        })
        .collect::<Vec<String>>();

    // fetch message metadata from mailcrab
    let messages = get_messages_metadata().await.unwrap();

    // sorted fetched message metadata from mailcrab and sort them by sent message ids
    let mut sorted_messages = vec![];
    for id in &responses {
        if let Some(message) = messages.iter().find(|m| m.id.to_string() == *id) {
            sorted_messages.push(message);
        }
    }

    // stop the server
    join.abort();

    assert_eq!(sorted_messages.len(), 3);
    assert!(sorted_messages[0].has_html);
    assert!(sorted_messages[0].has_plain);
    assert!(sorted_messages[0].attachments.is_empty());

    assert!(sorted_messages[1].has_html);
    assert!(!sorted_messages[1].has_plain);
    assert!(sorted_messages[1].attachments.is_empty());

    assert!(!sorted_messages[2].has_html);
    assert!(sorted_messages[2].has_plain);
    assert_eq!(sorted_messages[2].attachments.len(), 1);
}

#[tokio::test]
#[ignore]
async fn send_sample_messages() {
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    let mut paths = std::fs::read_dir("../samples").unwrap();
    let mailer = SmtpTransport::builder_dangerous("127.0.0.1".to_string())
        .port(smtp_port)
        .build();

    while let Some(Ok(entry)) = paths.next() {
        // skip non *.email files
        if entry.path().extension() != Some(OsStr::new("email")) {
            continue;
        }

        let message = std::fs::read_to_string(entry.path()).unwrap();
        let mut lines = message.lines();

        let sender = lines
            .next()
            .unwrap()
            .trim_start_matches("Sender: ")
            .parse::<Address>()
            .unwrap();
        let recipients = lines
            .next()
            .unwrap()
            .trim_start_matches("Recipients: ")
            .split(',')
            .map(|r| r.trim().parse::<Address>().unwrap())
            .collect::<Vec<Address>>();
        let envelope = Envelope::new(Some(sender), recipients).unwrap();

        let email = lines.collect::<Vec<&str>>().join("\n");

        mailer.send_raw(&envelope, email.as_bytes()).unwrap();
    }
}
