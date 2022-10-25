#!/bin/sh

cargo install cross --git https://github.com/cross-rs/cross

cd backend

cargo build --release
cross build --target aarch64-unknown-linux-gnu --release

cargo install trunk
rustup target add wasm32-unknown-unknown

cp target/aarch64-unknown-linux-gnu/release/mailcrab-backend target/arm64
cp target/release/mailcrab-backend target/amd64

cd ../frontend

trunk build

cd ..

docker buildx create --use
docker buildx build --push --platform=linux/amd64,linux/arm64 . -t marlonb/mailcrab:latest