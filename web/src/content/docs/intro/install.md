---
title: Install
description: Four ways in, all under 30 seconds.
sidebar:
  order: 3
---

:::caution[Pre-release]
Mammoth has not cut its first release yet. Only the build-from-source path below
works today. The others land with `v0.1.0` — see the [roadmap](https://github.com/Sakib-Dalal/mammoth/blob/main/docs/ROADMAP.md).
:::

## From source

```bash
git clone https://github.com/Sakib-Dalal/mammoth
cd mammoth
cargo build --release -p mammoth-cli
./target/release/mammoth quickstart
```

## Planned, at v0.1.0

```bash
curl -fsSL https://sakib-dalal.github.io/mammoth/install.sh | sh
cargo install mammoth-cli --locked
brew install Sakib-Dalal/tap/mammoth
docker run -p 8080:8080 -p 9000:9000 ghcr.io/sakib-dalal/mammoth quickstart
```

Linux binaries are static `musl` builds — no glibc version hell, and they run on
any kernel back to 3.2.
