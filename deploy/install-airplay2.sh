#!/usr/bin/env bash
# Builds and installs NQPTP + Shairport Sync with AirPlay 2 on Raspberry Pi OS
# (Lite, so no PipeWire/PulseAudio), per shairport-sync's BUILD.md.
# Run on the Pi as a regular user with sudo.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends build-essential git autoconf automake libtool \
	libpopt-dev libconfig-dev libasound2-dev avahi-daemon libavahi-client-dev libssl-dev libsoxr-dev \
	libplist-dev libsodium-dev uuid-dev libgcrypt-dev xxd libplist-utils \
	libavutil-dev libavcodec-dev libavformat-dev libglib2.0-dev systemd-dev

src="${XDG_CACHE_HOME:-$HOME/.cache}/hifi-src"
mkdir -p "$src"

# NQPTP: the PTP clock companion AirPlay 2 needs (UDP 319/320).
if [ -d "$src/nqptp" ]; then git -C "$src/nqptp" pull; else git clone https://github.com/mikebrady/nqptp.git "$src/nqptp"; fi
(cd "$src/nqptp" && autoreconf -fi && ./configure --with-systemd-startup && make && sudo make install)
sudo systemctl enable --now nqptp

# Shairport Sync with AirPlay 2 and the D-Bus interface (hifid uses DropSession).
if [ -d "$src/shairport-sync" ]; then git -C "$src/shairport-sync" pull; else git clone https://github.com/mikebrady/shairport-sync.git "$src/shairport-sync"; fi
(
	cd "$src/shairport-sync"
	autoreconf -fi
	./configure --sysconfdir=/etc --with-alsa --with-soxr --with-avahi --with-ssl=openssl \
		--with-systemd-startup --with-airplay-2 --with-dbus-interface
	make clean
	make
	sudo make install
)

here="$(cd "$(dirname "$0")" && pwd)"
sudo install -m644 "$here/shairport-sync.conf" /etc/shairport-sync.conf
sudo systemctl enable --now shairport-sync
echo "AirPlay 2 receiver installed. It appears as \"$(hostname)\" in AirPlay pickers."
