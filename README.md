# hifi

A headless, bit-perfect music player for a Raspberry Pi with a HiFiBerry Digi2 Pro:
a fast web UI for your local library, plus AirPlay 2 (multi-room, no subscription).

```
 phone / laptop ──HTTP + WebSocket──▶ hifid (Rust) ──MPD protocol──▶ MPD ──hw:──▶ Digi2 Pro ──S/PDIF──▶ DAC
                                        │    ▲
                    shairport-sync ─────┘    └── /proc/asound/…/hw_params (signal path)
                    (AirPlay 2, hooks)
```

- **MPD** decodes and plays: gapless playback, per-track format switching, DoP, straight to `hw:`.
- **hifid** owns everything else: the library index (SQLite + FTS5), artwork thumbnails, the
  queue UI, live state over WebSocket, and who gets the DAC (local playback vs AirPlay).
- **Web UI**: SvelteKit 3 (Svelte 5) SPA, embedded in the hifid binary. Light and dark mode,
  built for phones first; installable to the home screen.

## Bit-perfect, verified

MPD sends audio to `hw:CARD=sndrpihifiberry` with no mixer, resampler or format conversion
([deploy/mpd.conf](deploy/mpd.conf)). hifid then *checks* it: twice a second it compares MPD's
decoder format with what ALSA reports the card is receiving and shows the verdict in the UI:

| Badge | Meaning |
|---|---|
| **Bit-perfect** | Same rate and channels; bit depth equal or zero-padded (16-bit in a 32-bit frame is still exact). |
| **Bit-perfect · DoP** | DSD64 carried as DoP at 176.4 kHz. |
| **Resampled** | The file exceeds the Digi2 Pro (above 192 kHz), so MPD's soxr downsampled it. |
| **Converted** | Float source, channel change or truncation. |
| **AirPlay** | AirPlay owns the DAC: synchronized across rooms, by design not bit-perfect. |

Volume is never touched in software: set it on your DAC or amp.

## AirPlay and the DAC

`hw:` devices are exclusive, so exactly one source may hold the DAC:

1. An iPhone starts AirPlay → shairport-sync runs `hifid ctl acquire airplay` → hifid stops MPD
   (remembering the position), waits until ALSA reports the device closed, and returns.
2. The session ends → `hifid ctl release airplay`.
3. You press play in the web UI during AirPlay → hifid calls shairport-sync's D-Bus
   `DropSession`, waits for the DAC, then plays (resuming where AirPlay interrupted).

Each Pi is an ordinary AirPlay 2 speaker; group rooms from Control Center on the iPhone or Mac.

## Layout

```
crates/core        wire types; `cargo test -p hifi-core` regenerates web/src/lib/types/*.ts
crates/engine-mpd  MPD protocol client and typed commands (the playback-engine seam)
crates/library     MPD database → SQLite projection, search, artwork cache
crates/hifid       the daemon: HTTP API, WebSocket, DAC arbitration, signal path, embedded UI
web/               SvelteKit 3 app (Tailwind 4, Lucide icons)
deploy/            mpd.conf, shairport-sync.conf, hifid.toml, systemd unit, boot config
scripts/           cross-build and deploy helpers
```

## Develop (WSL or Linux)

Needs Rust (stable), Node 22.17+, and for Pi builds [zig](https://ziglang.org) +
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild).

```bash
cargo test --workspace                  # also regenerates the TypeScript types
scripts/dev.sh                          # MPD + a generated test library + hifid on :8080
```

`scripts/dev.sh` needs `mpd`, `flac` and `python3` (`sudo apt install mpd flac`). It builds a
small FLAC library in `~/hifi-dev` (silent audio, real tags, 16/44.1 up to 24/192, folder and
embedded artwork, a multi-disc album and an untagged compilation) and plays through MPD's null
output. Without a DAC the signal path reads *Unverified*; everything else behaves as on the Pi.

For UI work, run `npm run dev` in `web/` with hifid running (it proxies `/api` and `/art`).
Point it at the Pi instead with `HIFI_URL=http://hifi.local npm run dev`.

Debug builds read `web/build` from disk, so `npm run build` + refresh is enough; release builds
embed it.

## Install on the Pi

On **Raspberry Pi OS Lite (64-bit)**:

1. Append [deploy/config.txt](deploy/config.txt) to `/boot/firmware/config.txt`, reboot, and check
   `cat /proc/asound/cards` lists `sndrpihifiberry`.
2. `sudo apt install mpd` and mount your music at `/mnt/music` (or change `music_directory`).
3. AirPlay 2: copy `deploy/` to the Pi and run `deploy/install-airplay2.sh` (builds NQPTP and
   shairport-sync with AirPlay 2 and D-Bus support, installs the config, enables both).
4. From your dev machine:

   ```bash
   scripts/build-pi.sh
   scripts/deploy.sh pi@hifi.local --configs
   ```

5. Open `http://<hostname>.local/` on your phone and *Add to Home Screen*.

Use wired Ethernet: Wi-Fi power saving causes AirPlay dropouts.

## API

All commands are plain HTTP; state changes arrive on `GET /api/ws` (`hello` first, then deltas).

```
POST   /api/player/{play,pause,toggle,stop,next,previous}
POST   /api/player/seek            {"seconds": 42}
POST   /api/player/modes           {"repeat": true, "random": false, "single": false, "consume": false}
GET    /api/queue
POST   /api/queue                  {"album": "<id>", "uris": [], "mode": "append|next|replace", "play": true, "start": "<uri>"}
DELETE /api/queue                  (clear)
DELETE /api/queue/{id}
POST   /api/queue/{id}/play
POST   /api/queue/{id}/move        {"to": 3}
GET    /api/library                 counts, scan status, revision
GET    /api/library/albums          all albums (ETag = revision)
GET    /api/library/albums/{id}
GET    /api/library/search?q=…
POST   /api/library/rescan
POST   /api/sources/{name}/{acquire,release}
GET    /art/{key}-{300,600}.jpg     immutable
```

The API has no authentication; keep the Pi on a trusted network.
