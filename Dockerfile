FROM alpine:3.16
ARG TARGETARCH
WORKDIR /app
COPY ./frontend/dist /app/dist
COPY "./backend/bin/$TARGETARCH" /app/mailcrab
CMD ["/app/mailcrab"]
EXPOSE 1080 1025