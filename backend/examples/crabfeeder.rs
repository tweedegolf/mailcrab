/*
 * crabfeeder
 * a SMTP client
 * for injecting (test) emails into mailcrab
 */
use core::str::FromStr;
use lettre::address::Envelope;
use lettre::{Address, SmtpTransport, Transport};
use std::env;

fn usage(name: &str) {
    println!(
        r#"{name} <filenames>

and those filenames will be injected as email into mailcrab.

"#
    )
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        process(args[1..].to_vec());
    } else {
        usage(&args[0]);
    }
}

/// get a configuration from the environment or return default value
fn parse_env_var<T: FromStr>(name: &'static str, default: T) -> T {
    env::var(name)
        .unwrap_or_default()
        .parse::<T>()
        .unwrap_or(default)
}

fn process(filenames: Vec<String>) {
    let smtp_port: u16 = parse_env_var("SMTP_PORT", 1025);
    let mailer = SmtpTransport::builder_dangerous("127.0.0.1".to_string())
        .port(smtp_port)
        .build();

    for f in filenames.iter() {
        let message = std::fs::read_to_string(f).unwrap();
        let lines = message.lines();

        let sender = "carbfeeder@carbfeeder".parse::<Address>().unwrap();
        let recipients = "mailcrab@mailcrab"
            .split(',')
            .map(|r| r.trim().parse::<Address>().unwrap())
            .collect::<Vec<Address>>();
        let envelope = Envelope::new(Some(sender), recipients).unwrap();

        let email = lines.collect::<Vec<&str>>().join("\n");

        mailer.send_raw(&envelope, email.as_bytes()).unwrap();
    }
}
