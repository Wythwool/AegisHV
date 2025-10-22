# Multi-stage build
FROM rust:1.78 as build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:stable-slim
RUN useradd -m hv && mkdir -p /data && chown -R hv:hv /data
COPY --from=build /src/target/release/aegishv /usr/local/bin/aegishv
USER hv
EXPOSE 9108
ENTRYPOINT ["/usr/local/bin/aegishv","run","--tracefs","/sys/kernel/tracing","--listen","0.0.0.0:9108","--quiet"]
