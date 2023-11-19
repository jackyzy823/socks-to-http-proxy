FROM rust:alpine as builder
WORKDIR /build
COPY . .
RUN apk add musl-dev
RUN cargo install --path .

FROM alpine
COPY --from=builder /usr/local/cargo/bin/sthp /usr/local/bin/sthp

ENV RUST_LOG=info
ENV STHP_PORT=8080
ENV STHP_LISTEN_IP=0.0.0.0
ENV STHP_SOCKS_ADDRESS=127.0.0.1:1080

CMD ["sthp"]