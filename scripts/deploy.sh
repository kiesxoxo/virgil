#!/usr/bin/env bash
# Copies hifid and its config to the Pi and (re)starts it.
#   scripts/deploy.sh pi@hifi.local            # binary only
#   scripts/deploy.sh pi@hifi.local --configs  # also mpd.conf, hifid.toml, systemd unit, boot config
set -euo pipefail
host="${1:?usage: deploy.sh user@host [--configs]}"
cd "$(dirname "$0")/.."

bin=target/aarch64-unknown-linux-gnu/release/hifid
[ -x "$bin" ] || { echo "run scripts/build-pi.sh first" >&2; exit 1; }

scp -q "$bin" "$host:/tmp/hifid"
if [ "${2:-}" = "--configs" ]; then
	scp -q deploy/hifid.toml deploy/hifid.service deploy/mpd.conf deploy/config.txt "$host:/tmp/"
	ssh "$host" 'set -e
		sudo install -d /etc/hifi
		sudo install -m644 /tmp/hifid.toml /etc/hifi/hifid.toml
		sudo install -m644 /tmp/hifid.service /etc/systemd/system/hifid.service
		sudo install -m644 /tmp/mpd.conf /etc/mpd.conf
		grep -q "hifiberry-digi-pro" /boot/firmware/config.txt || echo "NOTE: add deploy/config.txt to /boot/firmware/config.txt and reboot"
		sudo systemctl daemon-reload
		sudo systemctl enable hifid
		sudo systemctl restart mpd'
fi
ssh "$host" 'sudo install -m755 /tmp/hifid /usr/local/bin/hifid && sudo systemctl restart hifid && systemctl --no-pager --lines=5 status hifid'
