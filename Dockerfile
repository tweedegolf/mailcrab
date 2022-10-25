FROM alpine:3.16
ARG TARGETARCH
WORKDIR /app
COPY ./frontend/dist /app/dist
COPY "./backend/target/$TARGETARCH" /app/mailcrab
CMD ["/app/mailcrab"]
EXPOSE 8080 2525