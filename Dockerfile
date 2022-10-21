FROM rust:1.64-slim-bullseye as builder

RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown

WORKDIR /usr/src/frontend
COPY frontend .
RUN mkdir analyzer_target && trunk build

WORKDIR /usr/src/backend
COPY backend .
RUN cargo build --release

FROM debian:bullseye-slim
EXPOSE 3000
WORKDIR /usr/local/bin
COPY --from=builder /usr/src/frontend/dist /usr/local/bin/dist
COPY --from=builder /usr/src/backend/target/release/mailcrab-backend /usr/local/bin/mailcrab
CMD ["/bin/sh", "-c", "/usr/local/bin/mailcrab"]
