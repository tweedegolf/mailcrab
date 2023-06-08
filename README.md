<img src="https://raw.githubusercontent.com/tweedegolf/mailcrab/main/frontend/img/mailcrab.svg" width="400" alt="MailCrab logo" />

# MailCrab

Email test server for development, written in Rust.

Inspired by [MailHog](https://github.com/mailhog/MailHog) and [MailCatcher](https://mailcatcher.me/).

MailCrab was created as an exercise in Rust, trying out [Axum](https://github.com/tokio-rs/axum) and functional components with [Yew](https://yew.rs/), but most of all because it is really enjoyable to write Rust code.

## TLDR

```sh
docker run --rm -p 1080:1080 -p 1025:1025 marlonb/mailcrab:latest
```

## Features

- Accept-all SMTP server
- Web interface to view and inspect all incoming email
- View formatted mail, download attachments, view headers or the complete raw mail contents
- Runs on all `amd64` and `arm64` platforms using docker
- Just a 7.77 MB docker image

![MailCrab screenshot](https://raw.githubusercontent.com/tweedegolf/mailcrab/main/frontend/img/screen.png)

## Technical overview

Both the backend server and the frontend are written in Rust. The backend receives email over an unencrypted connection on a configurable port. All email is stored in memory while the application is running. An API exposes all received email:

- `/api/messages` return all message metadata
- `/api/message/[id]` returns a complete message, given its `id`
- `/api/version` returns version information about the executable
- `/ws` send email metadata to each connected client when a new email is received

The frontend initially performs a call to `/api/messages` to receive all existing email metadata and then subscribes for new messages using the websocket connection. When opening a message, the `/api/message/[id]` endpoint is used to retrieve the complete message body and raw email.

The backend also accepts a few commands over the websocket, to mark a message as opened, to delete a single message or delete all messages.

## How to build

Install [Rust](https://www.rust-lang.org/learn/get-started)

```sh
# Have web bundler "trunk" available
cargo install --locked trunk

# Add WebAssembly as build target
rustup target add wasm32-unknown-unknown

# get the source code, like
git clone https://github.com/tweedegolf/mailcrab.git
# or fetch it from https://github.com/tweedegolf/mailcrab/releases
# and unpack it yourself

# change working directory to mailcrab
cd mailcrab

# bundle the frontend
cd frontend
trunk build --filehash false --release

# make the binary that includes the frontend stuff
cd ../backend
cargo build --release
# there is now target/release/mailcrab-backend

# rename it or copy it into what want. example given
cp target/release/mailcrab-backend ../mailcrab
```

## Installation and usage

To run MailCrab only docker is required. Start MailCrab using the following command:

```sh
docker run --rm -p 1080:1080 -p 1025:1025 marlonb/mailcrab:latest
```

Open a browser and navigate to [http://localhost:1080](http://localhost:1080) to view the web interface.

### Ports

The default SMTP port is 1025, the default HTTP port is 1080. You can configure the SMTP and HTTP port using environment variables (`SMTP_PORT` and `HTTP_PORT`), or by exposing them on different ports using docker:

```sh
docker run --rm -p 3000:1080 -p 2525:1025 marlonb/mailcrab:latest
```
  
## Host

You can specify the host address Mailcrab will listen on for HTTP request using
the `HTTP_HOST` environment variable. In the docker image the default
address is `0.0.0.0`, when running Mailcrab directly using cargo or a binary, the default is `127.0.0.1`.

### TLS

You can enable TLS and authentication by setting the environment variable `ENABLE_TLS_AUTH=true`. MailCrab will generate a key-pair and print the self-signed certificate. Any username/password combination is accepted. For example:

```sh
docker run --rm --env ENABLE_TLS_AUTH=true -p 1080:1080 -p 1025:1025 marlonb/mailcrab:latest
```

It is also possible to provide your own certificate by mounting a key and a certificate to `/app/key.pem` and `/app/cert.pem`:

```sh
docker run --rm --env ENABLE_TLS_AUTH=true -v key.pem:/app/key.pem:ro -v cert.pem:/app/cert.pem:ro -p 1080:1080 -p 1025:1025 marlonb/mailcrab:latest
```

### Path prefix

You can configure a prefix path for the web interface by setting and environment variable named `MAILCRAB_PREFIX`, for example:

```sh
docker run --rm --env MAILCRAB_PREFIX=emails -p 1080:1080 -p 1025:1025 marlonb/mailcrab:latest
```

The web interface will also be served at [http://localhost:1080/emails/](http://localhost:1080/emails/)

### Reverse proxy

See [the reverse proxy guide](./Reverse_proxy.md).

### Retention period

By default messages will be stored in memory until mailcrab is restarted. This might cause an OOM when Mailcrab lives
long enough and receives enough messages.

By setting `MAILCRAB_RETENTION_PERIOD` to a number of seconds, messages older than the provided duration will
be cleared.

### docker compose

Usage in a `docker-compose.yml` file:

```yml
version: '3.8'
services:
  mailcrab:
    image: marlonb/mailcrab:latest
    #        environment:
    #            ENABLE_TLS_AUTH: true # optionally enable TLS for the SMTP server
    #            MAILCRAB_PREFIX: emails # optionally prefix the webinterface with a path
    #        volumes:
    #           key.pem:/app/key.pem:ro # optionally provide your own keypair for TLS, else a pair will be generated
    #           cert.pem:/app/cert.pem:ro
    ports:
      - '1080:1080'
      - '1025:1025'
    networks: [default]
```

## Sample messages

The `samples` directory contains a couple of test messages. These can be sent using by running:

```sh
cd backend/
cargo test send_sample_messages -- --ignored
```

## Development

Web development
```sh
cd ../frontend
trunk serve
```
Connect with a web browser to what is defined as `[serve]`
in the `Trunk.toml`.  Current config is `127.0.0.1:8000`.
In another window you _edit_ `frontend/**` files.
Upon _save_ does `trunk` a _rebuild_ of the frontend. On-the-fly!

# optionally send test messages in an interval
cd ../backend
cargo test
```
