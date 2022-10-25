FROM debian:bullseye-slim
ARG TARGETARCH
WORKDIR /usr/local/bin
COPY ./frontend/dist /usr/local/bin/dist
COPY "./backend/target/$TARGETARCH" /usr/local/bin/mailcrab
CMD ["/usr/local/bin/mailcrab"]