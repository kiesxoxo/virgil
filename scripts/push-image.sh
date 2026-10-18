#!/usr/bin/env bash
# Copies the published arm64 image to a Pi that cannot reach the internet:
# fetches it here with crane (no Docker needed) and pipes it into `docker load`
# over SSH. Then restart the stack on the Pi with `docker compose up -d --pull never`.
#
#   scripts/push-image.sh mpk@pi-audio [ghcr.io/kiesxoxo/virgil:latest]
#   SSH=/mnt/c/Windows/System32/OpenSSH/ssh.exe scripts/push-image.sh …   # WSL + Windows ssh-agent
set -euo pipefail
host="${1:?usage: push-image.sh user@host [image]}"
image="${2:-ghcr.io/kiesxoxo/virgil:latest}"
ssh="${SSH:-ssh}"
crane_version=v0.22.1
crane="${CRANE:-$HOME/.local/bin/crane}"

if [ ! -x "$crane" ]; then
	echo "installing crane $crane_version to $crane"
	tmp=$(mktemp -d)
	base=https://github.com/google/go-containerregistry/releases/download/$crane_version
	asset=go-containerregistry_Linux_x86_64.tar.gz
	curl -sSfL "$base/$asset" -o "$tmp/$asset"
	curl -sSfL "$base/checksums.txt" -o "$tmp/checksums.txt"
	(cd "$tmp" && grep " $asset\$" checksums.txt | sha256sum -c - >/dev/null)
	tar -xzf "$tmp/$asset" -C "$tmp" crane
	install -D -m755 "$tmp/crane" "$crane"
	rm -rf "$tmp"
fi

tar=$(mktemp --suffix=.tar)
trap 'rm -f "$tar"' EXIT
echo "fetching $image (linux/arm64) $("$crane" digest --platform linux/arm64 "$image")"
"$crane" pull --platform linux/arm64 "$image" "$tar"
"$ssh" "$host" docker load < "$tar"
