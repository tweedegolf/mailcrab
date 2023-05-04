FROM alpine:3.16
ARG TARGETARCH
ENV HTTP_HOST="0.0.0.0"
WORKDIR /app
COPY --chmod=755 "./bin/$TARGETARCH" /app/mailcrab
CMD ["/app/mailcrab"]
EXPOSE 10801 1025
