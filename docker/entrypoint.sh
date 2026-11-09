#!/bin/bash
# Container entrypoint: runs MPD and virgild side by side. If either one exits,
# the container stops and Docker's restart policy brings both back.
#
# Configuration comes from the environment (VIRGIL_PORT, ALSA_CARD, MPD_DOP,
# AIRPLAY, HOMEKIT, and LIDARR_* / SLSKDPP_* for downloads). To take full control, mount your own files at /config/mpd.conf
# and/or /config/virgild.toml.
set -euo pipefail

# Virgil was called hifi: keep the library database and artwork it built.
if [ -d /data/hifi ] && [ ! -e /data/virgil ]; then
	mv /data/hifi /data/virgil
fi
mkdir -p /data/mpd/playlists /data/virgil /run/mpd /run/virgil

# One MPD output per sound card, named by its ALSA id, so Virgil can offer them
# all (e.g. a USB DAC next to the HiFiBerry). ALSA_CARD is set up for
# bit-perfect playback and enabled; other cards are off until picked in
# Settings, and may convert formats they can't take (the signal path says so).
asound=/host/asound
[ -r "$asound/cards" ] || asound=/proc/asound
cards=$(sed -n 's/^ *[0-9][0-9]* \[\([^] ]*\) *\].*/\1/p' "$asound/cards" 2>/dev/null || true)

alsa_output() { # card enabled strict
	local auto=yes dop=no
	if [ "$3" = strict ]; then auto=no; dop="${MPD_DOP}"; fi
	cat <<EOF

audio_output {
	type            "alsa"
	name            "$1"
	device          "hw:CARD=$1,DEV=0"
	enabled         "$2"
	mixer_type      "none"
	auto_resample   "$auto"
	auto_channels   "$auto"
	auto_format     "$auto"
	dop             "$dop"
}
EOF
}

alsa_outputs() {
	# The configured DAC even when it isn't there yet (MPD retries it).
	alsa_output "${ALSA_CARD}" yes strict
	for card in $cards; do
		[ "$card" = "${ALSA_CARD}" ] || alsa_output "$card" no lenient
	done
}

mpd_conf=/config/mpd.conf
if [ ! -f "$mpd_conf" ]; then
	mpd_conf=/run/virgil/mpd.conf
	cat > "$mpd_conf" <<EOF
music_directory      "/music"
playlist_directory   "/data/mpd/playlists"
db_file              "/data/mpd/database"
state_file           "/data/mpd/state"
sticker_file         "/data/mpd/sticker.sql"
bind_to_address      "/run/mpd/socket"
auto_update          "yes"
restore_paused       "yes"
replaygain           "off"
volume_normalization "no"
max_output_buffer_size "65536"

resampler {
	plugin  "soxr"
	quality "very high"
}
$(alsa_outputs)
EOF
fi

virgil_conf=/config/virgild.toml
if [ ! -f "$virgil_conf" ]; then
	virgil_conf=/run/virgil/virgild.toml
	cat > "$virgil_conf" <<EOF
listen = "0.0.0.0:${VIRGIL_PORT}"
mpd = "/run/mpd/socket"
data_dir = "/data/virgil"
alsa_card = "${ALSA_CARD}"
# Docker hides /proc/asound; compose.yaml mounts the host's copy here.
asound_dir = "/host/asound"
EOF
	if [ "${AIRPLAY}" = "on" ]; then
		cat >> "$virgil_conf" <<'EOF'

[sources.airplay]
preempt = "dbus-send --system --type=method_call --dest=org.gnome.ShairportSync /org/gnome/ShairportSync org.gnome.ShairportSync.DropSession"
EOF
	fi
	if [ "${HOMEKIT}" = "on" ]; then
		cat >> "$virgil_conf" <<'EOF'

# Apple Home: a switch for Siri, advertised through the host's avahi-daemon.
[homekit]
EOF
	fi
	if [ -n "${LIDARR_URL:-}" ]; then
		cat >> "$virgil_conf" <<EOF

# Downloads: albums through Lidarr, which files them into the library.
[lidarr]
url = "${LIDARR_URL}"
api_key = "${LIDARR_API_KEY:-}"
EOF
		if [ -n "${LIDARR_ROOT:-}" ]; then
			echo "root = \"${LIDARR_ROOT}\"" >> "$virgil_conf"
		fi
		if [ -n "${SLSKDPP_URL:-}" ]; then
			# "flac,mp3" → ["flac", "mp3"]
			formats=$(echo "${SLSKDPP_FORMATS:-flac}" | sed 's/ //g; s/[^,][^,]*/"&"/g; s/,/, /g')
			cat >> "$virgil_conf" <<EOF

# Single songs from Soulseek through slskdpp; Lidarr files them into their album.
[slskdpp]
url = "${SLSKDPP_URL}"
api_key = "${SLSKDPP_API_KEY:-}"
downloads = "${SLSKDPP_DOWNLOADS:-}"
formats = [${formats}]
EOF
		fi
		# It holds API keys now.
		chmod 600 "$virgil_conf"
	fi
fi

[ -d /music ] || echo "Virgil: nothing mounted at /music; the library will be empty" >&2

# On any exit, stop the other process cleanly (MPD saves its state on SIGTERM).
trap 'kill $(jobs -p) 2>/dev/null; wait' EXIT

mpd --no-daemon --stderr "$mpd_conf" &
for _ in $(seq 100); do
	[ -S /run/mpd/socket ] && break
	sleep 0.1
done
virgild --config "$virgil_conf" &

set +e
wait -n
status=$?
echo "Virgil: a process exited with status $status; stopping the container" >&2
exit 1
