FROM rust:1.82-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
      clang cmake protobuf-compiler nodejs npm && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo xtask build-ui && cargo build --release --locked -p mammoth-cli

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/mammoth /usr/local/bin/mammoth
EXPOSE 7000 8080 9000 9100
VOLUME ["/data", "/var/lib/mammoth"]
ENTRYPOINT ["/usr/local/bin/mammoth"]
CMD ["serve", "--role", "all"]
