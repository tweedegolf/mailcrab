/*
 * multiple_rcpt_to
 * a SMTP client
 * that injects a (test) email into mailcrab
 * for multiple recipients
 */
use core::str::FromStr;
use lettre::address::Envelope;
use lettre::{Address, SmtpTransport, Transport};
use std::env;
use std::net::IpAddr;

/// get a configuration from the environment or return default value
fn parse_env_var<T: FromStr>(name: &'static str, default: T) -> T {
    env::var(name)
        .unwrap_or_default()
        .parse::<T>()
        .unwrap_or(default)
}

fn main() {
    let smtp_server: IpAddr = parse_env_var("SMTP_SERVER", [127, 0, 0, 1].into());
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    println!("I: Will connecting {:?}:{:?}", smtp_server, smtp_port);
    let mailer = SmtpTransport::builder_dangerous(smtp_server.to_string())
        .port(smtp_port)
        .build();

    let sender = "multi_rcpt_to@examples".parse::<Address>().unwrap();
    let recipients = "many@mailcrab,foo@bar,foo@baz,dupli@ca.te,dupli@ca.te"
        .split(',')
        .map(|r| r.trim().parse::<Address>().unwrap())
        .collect::<Vec<Address>>();
    let envelope = Envelope::new(Some(sender), recipients).unwrap();

    let email = r#"From: Many RCPT TO <recipient@metadata>
To: See Envelope <not@these.headers>
Subject: Multiple RCPT TO in one SMTP session

Hi,

This message is for checking
how mailcrab handles multiple RCPT TO in one SMTP session.

Inspirated by the commit you can see with
  git log --patch 0699315cb2509^1..0699315cb2509

Bye
"#;

    mailer.send_raw(&envelope, email.as_bytes()).unwrap();
    println!("I: Having send one email");
}
