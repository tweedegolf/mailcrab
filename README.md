![MailCrab Logo](https://github.com/tweedegolf/mailcrab/blob/master/frontend/img/mailcrab.svg?raw=true)

# MailCrab

Email test server for development, written in Rust.

Inspired by [MailHog](https://github.com/mailhog/MailHog) and [MailCatcher](https://mailcatcher.me/).

MailCrab was created as an exercise in Rust, trying out [Axum](https://axum.rs/) and functional components with [Yew](https://yew.rs/), but most of all because it is really enjoyable to write Rust code.

## Features

- Accept-all SMTP server
- Web interface to view and inspect all incoming email
- View formatted email, download attachments, view headers or the complete raw email data
- Runs on multiple platforms

![MailCrab Logo](https://github.com/tweedegolf/mailcrab/blob/master/frontend/img/screen.png?raw=true)

## Technical overview

Both the backend server and the fontend are written in Rust. The backend receives email over an unencrypted connection on a configurable port. All email is stored in memory while the application is running. An API exposes all received email:

- `/api/messages` return all message metadata
- `/api/message/[id]` returns a complete message, given its `id`
- `/ws` send email metadata to each connected client when a new email is received

The frontend initially performs a call to `/api/messages` to receive all existing email metadata and then subscribes for new messages using the websocket connection. When opening a message, the `/api/message/[id]` endpoint is used to retrieve the complete message body and raw email.

The backend also accepts a few commands over the websocket, to mark a message as opened, to delete a single message or delete all messages.

## Installation and usage

To run MailCrab only docker is required. Start MailCrab using the following command:

```
docker run --rm -p 127.0.0.1:8080:8080 -p 127.0.0.1:2525:2525 marlonb/mailcrab:latest
```

Open a browser and navigate to [http://localhost:8080](http://localhost:8080) to view the web interface.

The default SMTP port is 2525, the default HTTP port is 8080. You can configure the SMTP and HTTP port using environment variables (`SMTP_PORT` and `HTTP_PORT`), or by exposing them on different ports using docker:

```
docker run --rm -p 127.0.0.1:3000:8080 -p 127.0.0.1:1025:2525 marlonb/mailcrab:latest
```

Usage in a `docker-compose.yml` file:

```
version: "3.8"
services:
    mailcrab:
        image: marlonb/mailcrab:latest
        ports:
            - "127.0.0.1:8080:8080"
            - "127.0.0.1:2525:2525"
        networks: [default]
```