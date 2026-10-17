# syntax=docker/dockerfile:1
#
# hifi: hifid + MPD in one image (linux/arm64 for the Pi, linux/amd64 for testing).
# The web UI and hifid are built on the build machine's own architecture and
# cross-compiled, so only the final `apt-get install` runs under emulation.

# --- web UI ------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM node:24-bookworm-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# --- hifid -------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS build
ARG TARGETARCH
WORKDIR /src
RUN <<EOF
set -eu
case "$TARGETARCH" in
	arm64) triple=aarch64-unknown-linux-gnu; gcc=aarch64-linux-gnu-gcc; pkgs="gcc-aarch64-linux-gnu libc6-dev-arm64-cross" ;;
	amd64) triple=x86_64-unknown-linux-gnu; gcc=x86_64-linux-gnu-gcc; pkgs="gcc-x86-64-linux-gnu libc6-dev-amd64-cross" ;;
	*) echo "unsupported TARGETARCH $TARGETARCH" >&2; exit 1 ;;
esac
if [ "$(dpkg --print-architecture)" = "$TARGETARCH" ]; then gcc=gcc; pkgs=""; fi
if [ -n "$pkgs" ]; then apt-get update && apt-get install -y --no-install-recommends $pkgs && rm -rf /var/lib/apt/lists/*; fi
rustup target add "$triple"
upper=$(echo "$triple" | tr 'a-z-' 'A-Z_')
lower=$(echo "$triple" | tr '-' '_')
{
	echo "export TRIPLE=$triple"
	echo "export CARGO_TARGET_${upper}_LINKER=$gcc"
	echo "export CC_${lower}=$gcc"
} > /cross.env
EOF
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY crates crates
COPY --from=web /src/web/build web/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/src/target,id=hifi-target-$TARGETARCH \
	. /cross.env \
	&& cargo build --release --locked -p hifid --target "$TRIPLE" \
	&& install -D "target/$TRIPLE/release/hifid" /out/hifid

# --- runtime -----------------------------------------------------------------
FROM debian:trixie-slim
RUN apt-get update \
	&& apt-get install -y --no-install-recommends mpd dbus-bin tini \
	&& rm -rf /var/lib/apt/lists/* /etc/mpd.conf
COPY --from=build /out/hifid /usr/local/bin/hifid
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/hifi-entrypoint
ENV HIFI_PORT=80 \
	ALSA_CARD=sndrpihifiberry \
	MPD_DOP=yes \
	AIRPLAY=on
VOLUME ["/data"]
EXPOSE 80
ENTRYPOINT ["tini", "-g", "--", "hifi-entrypoint"]
