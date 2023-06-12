/*
 * crabfeeder
 * a SMTP client
 * for injecting (test) emails into mailcrab
 */
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

fn process(filenames: Vec<String>) {
    for f in filenames.iter() {
        println!("{:?}", f);
    }
}
