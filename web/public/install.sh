#!/bin/sh
# Mammoth installer.
#
#   curl -fsSL https://sakib-dalal.github.io/mammoth/install.sh | sh
#
# Replaced at release time by the installer cargo-dist generates from
# dist-workspace.toml. Until the first tagged release, build from source.
set -eu

echo "mammoth: no released binaries yet."
echo
echo "build from source instead:"
echo "  git clone https://github.com/Sakib-Dalal/mammoth"
echo "  cd mammoth && cargo build --release -p mammoth-cli"
echo "  ./target/release/mammoth quickstart"
exit 1
