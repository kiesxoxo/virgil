"""Generate a small, realistic FLAC test library for hifi development.

Silent audio (tiny files) with real tags, mixed formats, folder and embedded
cover art. Usage: python3 -I scripts/gen-test-library.py <music_dir> [flac_binary]
"""

import math
import os
import random
import struct
import subprocess
import sys
import tempfile
import zlib

MUSIC = sys.argv[1]
FLAC = sys.argv[2] if len(sys.argv) > 2 else "flac"
random.seed(7)

# (artist, album, year, genre, bits, rate, art: folder|embedded|none, tracks, album_artist?)
ALBUMS = [
    ("Aurora Fields", "Northern Lights", 2021, "Electronic", 24, 96000, "folder",
     ["Polar Drift", "Magnetosphere", "Kp Index 7", "Solar Wind", "Green Curtain"], True),
    ("The Quiet Harbour", "Paper Boats", 2018, "Indie", 16, 44100, "embedded",
     ["Low Tide Lanterns", "Paper Boats", "Gulls", "Saltwater Letters", "Pier 9", "Fog Horn"], True),
    ("Elena Brandt Trio", "Midnight Standards", 1998, "Jazz", 24, 192000, "folder",
     ["Blue in the Small Hours", "Brushes", "Last Set", "Walking Home"], True),
    ("Ostinato Ensemble", "Glass Cathedral", 2015, "Classical", 24, 48000, "folder",
     ["I. Nave", "II. Clerestory", "III. Apse", "IV. Rose Window", "V. Crypt", "VI. Spire"], True),
    ("Neon Parade", "Static Bloom", 2023, "Synthpop", 24, 88200, "embedded",
     ["Chrome Hearts", "Static Bloom", "Night Drive (Extended)", "Pastel Riot", "Afterglow"], True),
    ("Marlow", "Low Tide", 2012, "Folk", 16, 44100, "none",
     ["Driftwood", "The Ferryman", "Shale", "Home by Spring"], True),
    ("Sana Itō", "Field Recordings, Vol. 2", 2020, "Ambient", 24, 96000, "folder",
     ["Cedar Forest, 5 a.m.", "Rain on Tin", "Harbour Cranes"], True),
    ("Björk Lindqvist", "Ice Palace", 2009, "Electronic", 16, 44100, "folder",
     ["Frost", "Kaldur", "Glacial Pace", "Meltwater"], True),
    ("The Velvet Orchard", "Second Harvest", 1971, "Rock", 16, 44100, "folder",
     ["Orchard Road", "Cider", "Picking Season", "Old Barn Blues", "Harvest Moon"], True),
    ("Kai Moreno", "Concrete Gardens", 2019, "Hip-Hop", 16, 48000, "embedded",
     ["Intro", "Concrete Gardens", "Rooftop Tomatoes", "Bus Stop Philosophy", "Outro"], True),
    ("Lumen Quartet", "String Theory", 2022, "Classical", 24, 192000, "folder",
     ["Allegro con brio", "Adagio", "Scherzo", "Finale"], True),
    ("Mira Solano", "Desert Radio", 2016, "Americana", 16, 44100, "folder",
     ["Highway 62", "Motel Neon", "Coyote Moon", "Dust Devils", "Last Gas for 90 Miles"], True),
    ("Hollow Pines", "Weathervane", 2014, "Indie", 24, 44100, "folder",
     ["Weathervane", "Cardinal Points", "Squall"], True),
    ("Dub Atlas", "Bass Cartography", 2010, "Dub", 16, 44100, "embedded",
     ["Meridian", "Longitude Skank", "Tropic of Echo", "Equator Dub"], True),
]

# A compilation without AlbumArtist tags: grouped by directory, shown as Various Artists.
COMPILATION = ("Summer Sessions 2024", 2024, "Pop", [
    ("Coral Lane", "Sunscreen"), ("Tomo & The Waves", "Postcard"), ("Pia Lux", "Lemonade Stand"),
    ("Coral Lane", "Night Swim"),
])


def png(path, size, c1, c2, accent):
    """Gradient cover with a soft circle; plain zlib PNG, no dependencies."""
    rows = []
    cx, cy, r = size * random.uniform(0.35, 0.65), size * random.uniform(0.35, 0.65), size * random.uniform(0.18, 0.3)
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            t = (x + y) / (2 * size)
            px = [int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3)]
            d = math.hypot(x - cx, y - cy)
            if d < r:
                a = min(1.0, (r - d) / 6) * 0.85
                px = [int(px[i] + (accent[i] - px[i]) * a) for i in range(3)]
            row += bytes(px)
        rows.append(bytes(row))
    raw = zlib.compress(b"".join(rows), 6)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)))
        f.write(chunk(b"IDAT", raw))
        f.write(chunk(b"IEND", b""))


def palette():
    h = random.random()

    def hsv(h, s, v):
        i = int(h * 6) % 6
        f = h * 6 - int(h * 6)
        p, q, t = v * (1 - s), v * (1 - f * s), v * (1 - (1 - f) * s)
        return [int(255 * c) for c in [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i]]

    return hsv(h, 0.65, 0.9), hsv((h + 0.12) % 1, 0.8, 0.45), hsv((h + 0.5) % 1, 0.35, 0.98)


def encode(path, bits, rate, seconds, tags, picture=None):
    """Streams silence (plus one LSB tick so it isn't all-zero) into flac."""
    frame = 2 * (bits // 8)
    total = int(seconds * rate)
    args = [FLAC, "-s", "-f", "-5", "--force-raw-format", "--endian=little", "--sign=signed",
            "--channels=2", f"--bps={bits}", f"--sample-rate={rate}", "-o", path]
    for k, v in tags.items():
        args += ["-T", f"{k}={v}"]
    if picture:
        args.append(f"--picture={picture}")
    args.append("-")
    proc = subprocess.Popen(args, stdin=subprocess.PIPE)
    chunk = bytes(frame * 4096)
    left = total
    while left > 0:
        n = min(left, 4096)
        proc.stdin.write(chunk[: n * frame])
        left -= n
    proc.stdin.close()
    if proc.wait() != 0:
        raise SystemExit(f"flac failed for {path}")


def main():
    os.makedirs(MUSIC, exist_ok=True)
    for artist, album, year, genre, bits, rate, art, titles, _ in ALBUMS:
        d = os.path.join(MUSIC, artist, f"{year} - {album}")
        os.makedirs(d, exist_ok=True)
        cover = None
        if art != "none":
            cover = os.path.join(d if art == "folder" else tempfile.gettempdir(), "cover.png" if art == "folder" else f"hifi-{year}-{bits}.png")
            png(cover, 360, *palette())
        multidisc = album == "Glass Cathedral"
        for i, title in enumerate(titles, 1):
            disc, num = (1 if i <= 3 else 2, (i - 1) % 3 + 1) if multidisc else (1, i)
            tags = {"TITLE": title, "ARTIST": artist, "ALBUMARTIST": artist, "ALBUM": album, "DATE": str(year),
                    "GENRE": genre, "TRACKNUMBER": str(num), "DISCNUMBER": f"{disc}/2" if multidisc else "1"}
            name = f"{disc}-{num:02d} {title}.flac" if multidisc else f"{num:02d} {title}.flac"
            seconds = random.uniform(150, 330) if rate <= 96000 else random.uniform(150, 260)
            encode(os.path.join(d, name.replace("/", "-")), bits, rate, seconds, tags, cover if art == "embedded" else None)
        print("album", artist, "-", album)

    album, year, genre, tracks = COMPILATION
    d = os.path.join(MUSIC, "Compilations", album)
    os.makedirs(d, exist_ok=True)
    png(os.path.join(d, "cover.png"), 360, *palette())
    for i, (artist, title) in enumerate(tracks, 1):
        tags = {"TITLE": title, "ARTIST": artist, "ALBUM": album, "DATE": str(year), "GENRE": genre, "TRACKNUMBER": str(i)}
        encode(os.path.join(d, f"{i:02d} {artist} - {title}.flac"), 16, 44100, random.uniform(160, 240), tags)
    print("album", album)


main()
