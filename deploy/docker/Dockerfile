FROM node:22-bookworm-slim AS ui
WORKDIR /src/ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ ./
RUN npm run check && npm test && npm run build

FROM rust:1.85-bookworm AS builder
WORKDIR /build
COPY . .
COPY --from=ui /src/ui/build ./ui/build
RUN cargo build --release --locked -p mammoth-cli

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --create-home mammoth && mkdir -p /data && chown mammoth:mammoth /data
COPY --from=builder /build/target/release/mammoth /usr/local/bin/mammoth
USER mammoth
ENV MAMMOTH_LOCAL_ROOT=/data
EXPOSE 8080 9000
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/mammoth"]
CMD ["quickstart", "--ui-listen", "0.0.0.0:8080", "--s3-listen", "0.0.0.0:9000", "--allow-remote"]
