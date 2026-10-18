#!/usr/bin/env bash
# Copies virgild and its config to the Pi and (re)starts it.
#   scripts/deploy.sh pi@virgil.local            # binary only
#   scripts/deploy.sh pi@virgil.local --configs  # also mpd.conf, virgild.toml, systemd unit, boot config
set -euo pipefail
host="${1:?usage: deploy.sh user@host [--configs]}"
cd "$(dirname "$0")/.."

bin=target/aarch64-unknown-linux-gnu/release/virgild
[ -x "$bin" ] || { echo "run scripts/build-pi.sh first" >&2; exit 1; }

scp -q "$bin" "$host:/tmp/virgild"
if [ "${2:-}" = "--configs" ]; then
	scp -q deploy/virgild.toml deploy/virgild.service deploy/mpd.conf deploy/config.txt "$host:/tmp/"
	ssh "$host" 'set -e
		sudo install -d /etc/virgil
		sudo install -m644 /tmp/virgild.toml /etc/virgil/virgild.toml
		sudo install -m644 /tmp/virgild.service /etc/systemd/system/virgild.service
		sudo install -m644 /tmp/mpd.conf /etc/mpd.conf
		grep -q "hifiberry-digi-pro" /boot/firmware/config.txt || echo "NOTE: add deploy/config.txt to /boot/firmware/config.txt and reboot"
		sudo systemctl daemon-reload
		sudo systemctl enable virgild
		sudo systemctl restart mpd'
fi
ssh "$host" 'sudo install -m755 /tmp/virgild /usr/local/bin/virgild && sudo systemctl restart virgild && systemctl --no-pager --lines=5 status virgild'
