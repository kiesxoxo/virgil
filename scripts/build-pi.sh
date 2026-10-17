#!/usr/bin/env bash
# Builds the web UI and a release hifid for a 64-bit Raspberry Pi.
# glibc 2.36 keeps the binary compatible with Raspberry Pi OS Bookworm and Trixie.
set -euo pipefail
cd "$(dirname "$0")/.."

(cd web && npm ci --no-audit --no-fund && npm run build)
cargo zigbuild --release -p hifid --target aarch64-unknown-linux-gnu.2.36

echo "built target/aarch64-unknown-linux-gnu/release/hifid"
