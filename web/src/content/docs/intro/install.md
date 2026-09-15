---
title: Install
description: Build the local Mammoth service with its dashboard.
sidebar:
  order: 3
---

Mammoth has not published its first release. Build the working local application
from the `AI_coded` branch with **Rust 1.85+**, **Node.js 22.12+ (22.x)** and Git.

```bash
git clone --branch AI_coded https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
npm --prefix ui ci
npm --prefix ui run build
cargo build --release --locked -p mammoth-cli
./target/release/mammoth --local-root .mammoth quickstart
```

Open [localhost:8080](http://localhost:8080) for the live dashboard. The service
stores files under `.mammoth`; restarting it preserves your data. Press Ctrl+C
to stop it. Use `target/release/mammoth.exe` on Windows.

Build the UI **before** the Rust binary so it embeds the production dashboard.
Without UI assets the gateway serves a setup page.

## Docker

From the repository root:

```bash
docker compose -f deploy/compose/docker-compose.yml up --build
```

The compose file runs one local development service with a persistent volume.
It is not a multi-machine deployment. See the [quickstart](/intro/quickstart/)
for file operations and the current limits.

## Release packaging

Run `cargo xtask dist` to create a native archive in `target/dist/`.

The draft-release workflow builds Linux x86-64, macOS ARM64 and Windows x86-64
archives when a version tag is pushed. Published installers, Homebrew packages,
container images and multi-node Helm deployment remain future work.
