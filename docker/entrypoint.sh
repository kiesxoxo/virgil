#!/bin/bash
# Container entrypoint: runs MPD and hifid side by side. If either one exits,
# the container stops and Docker's restart policy brings both back.
#
# Configuration comes from the environment (HIFI_PORT, ALSA_CARD, MPD_DOP,
# AIRPLAY). To take full control, mount your own files at /config/mpd.conf
# and/or /config/hifid.toml.
set -euo pipefail

mkdir -p /data/mpd/playlists /data/hifi /run/mpd /run/hifi

mpd_conf=/config/mpd.conf
if [ ! -f "$mpd_conf" ]; then
	mpd_conf=/run/hifi/mpd.conf
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

audio_output {
	type            "alsa"
	name            "${ALSA_CARD}"
	device          "hw:CARD=${ALSA_CARD},DEV=0"
	mixer_type      "none"
	auto_resample   "no"
	auto_channels   "no"
	auto_format     "no"
	dop             "${MPD_DOP}"
}
EOF
fi

hifi_conf=/config/hifid.toml
if [ ! -f "$hifi_conf" ]; then
	hifi_conf=/run/hifi/hifid.toml
	cat > "$hifi_conf" <<EOF
listen = "0.0.0.0:${HIFI_PORT}"
mpd = "/run/mpd/socket"
data_dir = "/data/hifi"
# Downloaded covers are saved next to the music.
music_dir = "/music"
alsa_card = "${ALSA_CARD}"
# Docker hides /proc/asound; compose.yaml mounts the host's copy here.
asound_dir = "/host/asound"
EOF
	if [ "${AIRPLAY}" = "on" ]; then
		cat >> "$hifi_conf" <<'EOF'

[sources.airplay]
preempt = "dbus-send --system --type=method_call --dest=org.gnome.ShairportSync /org/gnome/ShairportSync org.gnome.ShairportSync.DropSession"
EOF
	fi
fi

[ -d /music ] || echo "hifi: nothing mounted at /music; the library will be empty" >&2

# On any exit, stop the other process cleanly (MPD saves its state on SIGTERM).
trap 'kill $(jobs -p) 2>/dev/null; wait' EXIT

mpd --no-daemon --stderr "$mpd_conf" &
for _ in $(seq 100); do
	[ -S /run/mpd/socket ] && break
	sleep 0.1
done
hifid --config "$hifi_conf" &

set +e
wait -n
status=$?
echo "hifi: a process exited with status $status; stopping the container" >&2
exit 1
