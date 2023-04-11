FROM alpine:3.16
ARG TARGETARCH
ENV HOST="0.0.0.0"
WORKDIR /app
COPY --chmod=755 "./bin/$TARGETARCH" /app/mailcrab
CMD ["/app/mailcrab"]
EXPOSE 1080 1025
