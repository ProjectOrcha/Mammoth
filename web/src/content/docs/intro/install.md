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

## Use mammoth from any folder

On **macOS or Linux**, add a shortcut to the release binary. Run this once from
the `Mammoth` project folder after building:

```bash
mkdir -p "$HOME/.local/bin"
ln -sfn "$PWD/target/release/mammoth" "$HOME/.local/bin/mammoth"
export PATH="$HOME/.local/bin:$PATH"
mammoth --help
```

To keep the command available in new terminals, add this line to `~/.zshrc` for
zsh or `~/.bashrc` for Bash:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

You can now use `mammoth` instead of `./target/release/mammoth`. The shortcut
points to this checkout, so release rebuilds update the command automatically.
If you move the project folder, create the shortcut again from its new location.

From the project folder:

```bash
mammoth logo
mammoth --local-root .mammoth status
mammoth --local-root .mammoth stop
```

`shutdown` is an alias for `stop`. Stopping closes the dashboard and S3 service
after active work finishes and preserves your files. The command works from any
folder, but `.mammoth` is a relative storage path: use an absolute `--local-root`
path when working elsewhere, or set `MAMMOTH_LOCAL_ROOT` to that absolute path.

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
