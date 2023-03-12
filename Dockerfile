FROM alpine:3.16 AS downloader

ARG TAG
ARG TARGETPLATFORM
RUN apk add --no-cache wget tar unzip
RUN <<EOT
url="https://github.com/EmilSodergren/rust-tuya-mqtt/releases/download"
if [ "linux/amd64" = "$TARGETPLATFORM" ]; then
wget $url/$TAG/rust-tuya-mqtt-x86_64-unknown-linux-gnu.tar.gz -O /rtm.tar.gz
elif [ "linux/arm64" = "$TARGETPLATFORM" ]; then
wget $url/$TAG/rust-tuya-mqtt-aarch64-unknown-linux-musl.tar.gz -O /rtm.tar.gz
elif [ "linux/arm/v7" = "$TARGETPLATFORM" ]; then
wget $url/$TAG/rust-tuya-mqtt-armv7-unknown-linux-gnueabihf.tar.gz -O /rtm.tar.gz
fi
EOT
RUN tar xzf /rtm.tar.gz

FROM alpine:3.16

LABEL maintainer="Emil Sodergren <EmilSodergren@users.noreply.github.com>" description="Rust-tuya-mqtt in a container"

WORKDIR /rust-tuya-mqtt/config

VOLUME ["/rust-tuya-mqtt/config"]

COPY --from=downloader rust-tuya-mqtt /rtm
RUN chmod +x /rtm

EXPOSE 1883
EXPOSE 8886

ENV TUYA_LOG=debug

ENTRYPOINT ["/rtm"]

