#!/bin/sh

cd backend

cargo build --release
cross build --target aarch64-unknown-linux-gnu --release

cp target/aarch64-unknown-linux-gnu/release/mailcrab-backend target/arm64
cp target/release/mailcrab-backend target/amd64

cd ../frontend

# rustup target add wasm32-unknown-unknown
trunk build

cd ..

docker buildx build --push --platform=linux/amd64,linux/arm64 . -t marlonb/mailcrab:latest