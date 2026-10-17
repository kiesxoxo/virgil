# syntax=docker/dockerfile:1
#
# hifi: hifid + MPD in one image (linux/arm64 for the Pi, linux/amd64 for testing).
# The web UI and hifid are built on the build machine's own architecture and
# cross-compiled. MPD and FFmpeg are compiled for the target, so CI builds each
# architecture on a native runner (a local cross build works, but under QEMU).
#
# MPD is built from source with only what hifi uses: Debian's package pulls in
# a full FFmpeg with Mesa, LLVM, ICU, Samba, PipeWire and more (~800 MB).

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
# The registry cache is shared by the parallel arm64/amd64 builds; lock it so
# they don't unpack the same crates at once.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
	--mount=type=cache,target=/src/target,id=hifi-target-$TARGETARCH \
	. /cross.env \
	&& cargo build --release --locked -p hifid --target "$TRIPLE" \
	&& install -D "target/$TRIPLE/release/hifid" /out/hifid

# --- MPD ---------------------------------------------------------------------
FROM debian:trixie-slim AS mpd
RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		build-essential pkgconf meson ninja-build nasm xz-utils \
		libasound2-dev libflac-dev libmpg123-dev libvorbis-dev libopus-dev libogg-dev \
		libwavpack-dev libsoxr-dev libid3tag0-dev libsqlite3-dev zlib1g-dev libpcre2-dev libfmt-dev \
	&& rm -rf /var/lib/apt/lists/*
WORKDIR /src
# Bump a version together with its checksum below.
ARG FFMPEG_VERSION=8.1.3 \
	MPD_VERSION=0.24.15
ADD --checksum=sha256:7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3 \
	https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz ./
ADD --checksum=sha256:524c70bce3fcd268016156bdc97abcf5be1904f0c6fd622688999123e3c4c457 \
	https://www.musicpd.org/download/mpd/0.24/mpd-${MPD_VERSION}.tar.xz ./
# FFmpeg only for the formats MPD has no native decoder for: AAC and ALAC
# (.m4a), WAV/AIFF, APE and TTA. No filters, no resampler, no network.
RUN tar -xf ffmpeg-${FFMPEG_VERSION}.tar.xz \
	&& cd ffmpeg-${FFMPEG_VERSION} \
	&& ./configure --prefix=/usr/local \
		--enable-shared --disable-static \
		--disable-everything --disable-autodetect --disable-programs --disable-doc \
		--disable-network --disable-avdevice --disable-avfilter --disable-swscale --disable-swresample \
		--enable-demuxer=mov,aac,wav,w64,aiff,ape,tta \
		--enable-decoder=aac,alac,ape,tta,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be,pcm_f32le,pcm_f32be,pcm_f64le,pcm_f64be,pcm_u8,pcm_s8 \
		--enable-parser=aac \
	&& make -j"$(nproc)" \
	&& make install
# Every optional feature is off unless listed. zlib reads the compressed
# database Debian's MPD wrote; sqlite backs sticker_file; DSD (DSF/DFF, DoP)
# is built in. No ICU or iconv: hifid doesn't use MPD's search, and the
# filesystem charset stays UTF-8.
RUN tar -xf mpd-${MPD_VERSION}.tar.xz \
	&& cd mpd-${MPD_VERSION} \
	&& meson setup build --prefix=/usr/local --buildtype=release -Db_ndebug=true \
		-Dauto_features=disabled \
		-Dalsa=enabled -Dflac=enabled -Dmpg123=enabled -Dvorbis=enabled -Dopus=enabled \
		-Dwavpack=enabled -Dffmpeg=enabled -Did3tag=enabled -Dsoxr=enabled \
		-Dsqlite=enabled -Dzlib=enabled -Dpcre=enabled -Dipv6=enabled \
		-Ddsd=true -Ddatabase=true -Dinotify=true -Dcue=true \
		-Dhttpd=false -Dsnapcast=false -Drecorder=false -Dfifo=false -Dpipe=false \
		-Dwave_encoder=false -Dneighbor=false -Dupnp=disabled -Dzeroconf=disabled \
		-Ddocumentation=disabled -Dhtml_manual=false -Dmanpages=false \
	&& meson compile -C build \
	&& meson install -C build --strip \
	&& install -D -t /out/bin /usr/local/bin/mpd \
	&& mkdir -p /out/lib && cp -a /usr/local/lib/lib*.so.* /out/lib/

# --- root filesystem -----------------------------------------------------------
FROM debian:trixie-slim AS rootfs
COPY --from=mpd /out/ /usr/local/
RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		tini dbus-bin \
		libasound2t64 libflac14 libmpg123-0t64 libvorbis0a libopus0 libogg0 libwavpack1 \
		libsoxr0 libid3tag0 libsqlite3-0 libpcre2-8-0 libfmt10 zlib1g \
	&& ldconfig \
	&& mpd --version \
	&& rm -rf /var/lib/apt/lists/*
# Nothing installs packages in the running container, so drop the package
# manager and what only it needs (Perl, debconf), and glibc's charset
# converters (20 MB on arm64; MPD is built without iconv). Deleting files only
# shrinks the image because the next stage copies the result into one layer.
RUN dpkg --purge --force-remove-essential --force-depends \
		apt libapt-pkg7.0 sqv debconf perl-base > /dev/null \
	&& rm -rf /etc/apt /var/lib/apt /var/cache/* /var/log/* /usr/lib/*-linux-gnu/gconv \
	&& mpd --version > /dev/null

# --- runtime -----------------------------------------------------------------
FROM scratch
COPY --from=rootfs / /
COPY --from=build /out/hifid /usr/local/bin/hifid
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/hifi-entrypoint
ENV PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
	HIFI_PORT=80 \
	ALSA_CARD=sndrpihifiberry \
	MPD_DOP=yes \
	AIRPLAY=on
VOLUME ["/data"]
EXPOSE 80
ENTRYPOINT ["tini", "-g", "--", "hifi-entrypoint"]
