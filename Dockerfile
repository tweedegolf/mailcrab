FROM alpine:3.16
ARG TARGETARCH
WORKDIR /app
COPY "./backend/bin/$TARGETARCH" /app/mailcrab
CMD ["/app/mailcrab"]
EXPOSE 1080 1025
