#!/usr/bin/env bash
# Local dev loop: MPD with a generated test library (null output, plays in real
# time without a sound card) plus hifid on http://localhost:8080.
#
#   scripts/dev.sh                 # needs `mpd`, `flac` and `python3` on PATH
#   MPD=~/bin/mpd scripts/dev.sh   # or point at other binaries (MPD=…, FLAC=…)
#
# Everything lives in $HIFI_DEV (default ~/hifi-dev). MPD stops with this script.
set -euo pipefail
cd "$(dirname "$0")/.."
DEV="${HIFI_DEV:-$HOME/hifi-dev}"
MPD="${MPD:-mpd}"
FLAC="${FLAC:-flac}"

mkdir -p "$DEV/mpd/playlists" "$DEV/data"
[ -d "$DEV/music" ] || python3 -I scripts/gen-test-library.py "$DEV/music" "$FLAC"
[ -f web/build/index.html ] || (cd web && npm ci --no-audit --no-fund && npm run build)

cat > "$DEV/mpd.conf" <<EOF
music_directory     "$DEV/music"
db_file             "$DEV/mpd/db"
state_file          "$DEV/mpd/state"
playlist_directory  "$DEV/mpd/playlists"
log_file            "$DEV/mpd/log"
bind_to_address     "127.0.0.1"
port                "6600"
auto_update         "no"
replaygain          "off"
audio_output {
	type "null"
	name "null (dev)"
	sync "yes"
}
EOF

cat > "$DEV/hifid.toml" <<EOF
listen = "0.0.0.0:8080"
mpd = "127.0.0.1:6600"
data_dir = "$DEV/data"
# No ALSA card here, so the signal path reads "Unverified".
alsa_card = "dev-none"

# Lets you try the AirPlay hand-off: curl -X POST localhost:8080/api/sources/airplay/acquire
[sources.airplay]
preempt = "echo 'dev: AirPlay would be asked to drop its session here'"
EOF

if (exec 3<>/dev/tcp/127.0.0.1/6600) 2>/dev/null; then
	echo "using the MPD already listening on 127.0.0.1:6600"
else
	"$MPD" "$DEV/mpd.conf" --no-daemon &
	mpd_pid=$!
	trap 'kill $mpd_pid 2>/dev/null' EXIT
	for _ in $(seq 50); do (exec 3<>/dev/tcp/127.0.0.1/6600) 2>/dev/null && break; sleep 0.1; done
fi

# Rescan the test library (MPD's `update`; hifid picks up the result by itself).
exec 3<>/dev/tcp/127.0.0.1/6600
printf 'update\nclose\n' >&3
exec 3>&-

cargo run -p hifid -- --config "$DEV/hifid.toml"
