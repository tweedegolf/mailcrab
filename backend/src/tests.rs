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
use mailcrab::MailMessageMetadata;
use reqwest::Client;
use std::ffi::OsStr;
use tokio::time::{Duration, sleep};

use crate::{parse_env_var, run};

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
async fn functional() {
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
            sorted_messages.push(message.clone());
        }
    }

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

    // send a large attachment and verify it can be downloaded via the URL endpoint
    const SIZE: usize = 75 * 1024 * 1024; // 75 MiB
    send_large_file(SIZE).await.expect("send failed");

    let mut large_meta = None;
    for _ in 0..300 {
        let messages = get_messages_metadata().await.unwrap();
        if let Some(m) = messages
            .into_iter()
            .find(|m| m.attachments.iter().any(|a| a.filename == "large.bin"))
        {
            large_meta = Some(m);
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    let meta = large_meta.expect("large attachment message not received within timeout");

    assert_eq!(meta.attachments.len(), 1);
    assert_eq!(meta.attachments[0].filename, "large.bin");

    let http_port: u16 = parse_env_var("HTTP_PORT", 1080);
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let attachment_bytes = client
        .get(format!(
            "http://127.0.0.1:{http_port}/api/message/{}/attachment/0",
            meta.id
        ))
        .send()
        .await
        .expect("attachment request failed")
        .bytes()
        .await
        .expect("reading body failed");

    assert_eq!(attachment_bytes.len(), SIZE);

    let expected: Vec<u8> = (0..SIZE).map(|i| (i % 251) as u8).collect();
    assert_eq!(attachment_bytes.as_ref(), expected.as_slice());

    // stop the server
    join.abort();
}

#[cfg(feature = "imap")]
mod imap {
    use mailcrab::MailMessage;
    use std::sync::Arc;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpStream,
        time::{Duration, sleep},
    };
    use tokio_util::sync::CancellationToken;

    use crate::AppState;

    const IMAP_PORT: u16 = 41143;

    const RAW_MESSAGE: &str = concat!(
        "Subject: Test message\r\n",
        "From: Sender <sender@example.com>\r\n",
        "To: Receiver <receiver@example.com>\r\n",
        "Date: Sat, 01 Aug 2026 23:20:55 +0200\r\n",
        "Message-ID: <test-1@example.com>\r\n",
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
    );

    /// send a command and collect all response lines up to and including the
    /// tagged response
    async fn command(stream: &mut BufReader<TcpStream>, tag: &str, command: &str) -> Vec<String> {
        stream
            .get_mut()
            .write_all(format!("{tag} {command}\r\n").as_bytes())
            .await
            .expect("failed to send command");

        let mut lines = Vec::new();

        loop {
            let mut line = String::new();
            stream.read_line(&mut line).await.expect("failed to read");
            let done = line.starts_with(&format!("{tag} "));
            lines.push(line);

            if done {
                return lines;
            }
        }
    }

    fn assert_contains(lines: &[String], needle: &str) {
        assert!(
            lines.iter().any(|line| line.contains(needle)),
            "expected {needle:?} in response: {lines:?}"
        );
    }

    #[tokio::test]
    async fn imap_functional() {
        let (tx, rx) = tokio::sync::broadcast::channel::<MailMessage>(16);
        let storage_rx = rx.resubscribe();
        let state = Arc::new(AppState {
            rx,
            storage: Default::default(),
            prefix: String::new(),
            index: None,
            retention_period: Duration::from_secs(0),
        });
        let token = CancellationToken::new();

        tokio::spawn(crate::storage::storage(
            storage_rx,
            state.clone(),
            token.clone(),
        ));
        tokio::spawn(crate::imap::imap_server(
            [127, 0, 0, 1].into(),
            IMAP_PORT,
            state.clone(),
            token.clone(),
        ));

        // deliver a message and wait for the storage task to pick it up
        let message: MailMessage = mail_parser::MessageParser::default()
            .parse(RAW_MESSAGE.as_bytes())
            .expect("failed to parse message")
            .try_into()
            .expect("failed to convert message");
        tx.send(message).expect("failed to queue message");

        for _ in 0..100 {
            if state.storage.read().map(|s| s.len()).unwrap_or(0) == 1 {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(state.storage.read().unwrap().len(), 1);

        // connect and read the greeting
        let mut stream = None;
        for _ in 0..100 {
            if let Ok(socket) = TcpStream::connect(("127.0.0.1", IMAP_PORT)).await {
                stream = Some(BufReader::new(socket));
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        let mut stream = stream.expect("failed to connect to IMAP server");

        let mut greeting = String::new();
        stream.read_line(&mut greeting).await.unwrap();
        assert!(greeting.starts_with("* OK"), "greeting: {greeting:?}");

        let lines = command(&mut stream, "a0", "CAPABILITY").await;
        assert_contains(&lines, "IMAP4REV1");
        assert_contains(&lines, "a0 OK");

        let lines = command(&mut stream, "a1", "LOGIN test test").await;
        assert_contains(&lines, "a1 OK");

        let lines = command(&mut stream, "a2", "LIST \"\" \"*\"").await;
        assert_contains(&lines, "INBOX");

        let lines = command(&mut stream, "a3", "SELECT INBOX").await;
        assert_contains(&lines, "* 1 EXISTS");
        assert_contains(&lines, "UIDVALIDITY");
        assert_contains(&lines, "a3 OK [READ-WRITE");

        let lines = command(
            &mut stream,
            "a4",
            "FETCH 1 (FLAGS UID RFC822.SIZE ENVELOPE BODYSTRUCTURE)",
        )
        .await;
        assert_contains(&lines, "UID 1");
        assert_contains(&lines, "Test message");
        assert_contains(&lines, "sender");
        // the multipart/alternative structure with both body parts
        assert_contains(&lines, "\"alternative\"");
        assert_contains(&lines, "\"html\"");
        assert_contains(&lines, "a4 OK");

        // fetching the full message returns the raw content and marks it seen
        let lines = command(&mut stream, "a5", "UID FETCH 1 (BODY[])").await;
        assert_contains(&lines, "Hello world!");
        assert_contains(&lines, "a5 OK");

        let lines = command(&mut stream, "a6", "FETCH 1 (FLAGS)").await;
        assert_contains(&lines, "\\Seen");

        let lines = command(&mut stream, "a7", "UID SEARCH UNSEEN").await;
        assert_contains(&lines, "* SEARCH\r\n");

        // fetch only the plain text part (part 1 of the multipart)
        let lines = command(&mut stream, "a8", "FETCH 1 (BODY.PEEK[1])").await;
        assert_contains(&lines, "Hello world!");

        // delete the message
        let lines = command(&mut stream, "a9", "STORE 1 +FLAGS (\\Deleted)").await;
        assert_contains(&lines, "\\Deleted");

        let lines = command(&mut stream, "a10", "EXPUNGE").await;
        assert_contains(&lines, "* 1 EXPUNGE");
        assert_contains(&lines, "a10 OK");

        assert_eq!(state.storage.read().unwrap().len(), 0);

        let lines = command(&mut stream, "a11", "LOGOUT").await;
        assert_contains(&lines, "* BYE");

        token.cancel();
    }
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

async fn send_large_file(size_bytes: usize) -> Result<Response, Box<dyn std::error::Error>> {
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1".to_string())
        .port(smtp_port)
        .build();

    // generates pseudo-random bytes without any added dependencies
    let body: Vec<u8> = (0..size_bytes).map(|i| (i % 251) as u8).collect();

    let email = Message::builder()
        .from("sender@example.com".parse()?)
        .to("recipient@example.com".parse()?)
        .subject(format!("Large attachment test ({size_bytes} bytes)"))
        .multipart(
            MultiPart::mixed()
                .singlepart(SinglePart::plain("See attached.".to_owned()))
                .singlepart(
                    Attachment::new("large.bin".to_owned())
                        .body(body, ContentType::parse("application/octet-stream")?),
                ),
        )?;

    Ok(mailer.send(email).await?)
}
