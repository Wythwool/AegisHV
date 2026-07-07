FROM rust:1.81-slim AS build
WORKDIR /src
COPY . .
RUN cargo metadata --locked --format-version 1 >/tmp/aegishv.metadata.json \
    && cargo build --locked --release

FROM debian:bookworm-slim
ARG AEGISHV_VERSION="0.4.0"
ARG AEGISHV_REVISION="unknown"
ARG AEGISHV_CREATED="1970-01-01T00:00:00Z"
LABEL org.opencontainers.image.title="AegisHV" \
      org.opencontainers.image.description="Host-side KVM telemetry sensor" \
      org.opencontainers.image.source="https://github.com/Nullbit1/AegisHV" \
      org.opencontainers.image.url="https://github.com/Nullbit1/AegisHV" \
      org.opencontainers.image.documentation="https://github.com/Nullbit1/AegisHV" \
      org.opencontainers.image.version="${AEGISHV_VERSION}" \
      org.opencontainers.image.revision="${AEGISHV_REVISION}" \
      org.opencontainers.image.created="${AEGISHV_CREATED}" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.authors="https://github.com/Wythwool" \
      org.opencontainers.image.vendor="https://github.com/Nullbit1"
RUN useradd --system --home /nonexistent --shell /usr/sbin/nologin aegishv \
    && mkdir -p /var/lib/aegishv/dumps /var/log/aegishv \
    && chown -R aegishv:aegishv /var/lib/aegishv /var/log/aegishv
COPY --from=build /src/target/release/aegishv /usr/local/bin/aegishv
COPY config.example.toml /etc/aegishv/config.toml
USER aegishv
ENTRYPOINT ["/usr/local/bin/aegishv"]
CMD ["run", "--config", "/etc/aegishv/config.toml", "--jsonl", "/var/log/aegishv/events.jsonl"]
