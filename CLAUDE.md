# Virgil

## Quick deploy to the Pi (test build)

Puts a virgild built from the working tree on `mpk@pi-audio`, to try over https://virgil.home.marcinkies.com before a release. It reuses the published image and swaps in the binary, and leaves `compose.yaml` and the `latest` tag alone.

1. Build virgild for arm64 (the web app is built into it):

   ```sh
   docker build --platform linux/arm64 --target build -t virgil-test-build .
   ```

2. Wrap it in the published image:

   ```sh
   dir=$(mktemp -d)
   id=$(docker create --platform linux/arm64 virgil-test-build)
   docker cp -q $id:/out/virgild $dir/virgild && docker rm $id
   printf 'FROM ghcr.io/kiesxoxo/virgil:latest\nCOPY virgild /usr/local/bin/virgild\n' > $dir/Dockerfile
   docker build -q --platform linux/arm64 -t ghcr.io/kiesxoxo/virgil:test $dir
   ```

3. Send it to the Pi and run it through a compose override:

   ```sh
   docker save --platform linux/arm64 ghcr.io/kiesxoxo/virgil:test | ssh mpk@pi-audio docker load
   ssh mpk@pi-audio 'cd ~/srv/virgil &&
     printf "services:\n  virgil:\n    image: ghcr.io/kiesxoxo/virgil:test\n" > compose.test.yaml &&
     docker compose -f compose.yaml -f compose.test.yaml up -d --pull never virgil &&
     docker image prune -f'
   ```

Back to the release: `ssh mpk@pi-audio 'cd ~/srv/virgil && docker compose up -d'`.

## Running the web app against the Pi

To try web changes without deploying, the dev server proxies the API and artwork to the Pi:

```sh
cd web && VIRGIL_URL=https://virgil.home.marcinkies.com npx vite dev --port 5199 --strictPort
```

Run it in the background; the app is then at http://localhost:5199. It plays on the real MPD, so don't press play or change the queue unless asked.

## Debug in Safari, not Chrome

Virgil is used on iPhones and iPads, where every browser is WebKit. Debug, test and check rendering in Safari: native Safari over WebDriver to drive the app and inspect it, and the iOS Simulator to see it on a phone. Don't use Chrome, headless Chrome or Chromium screencasts. Chromium differs from WebKit in exactly what this app leans on: view transitions, WebGL and compositing, image decoding and scroll snapping. A glitch that Chrome shows, or hides, says little about the iPhone.

## iOS Simulator (Mobile Safari)

Xcode is installed, with iPhone and iPad simulators (`xcrun simctl list devices available`). The simulator shares the Mac's network, so `localhost:5199` works there too.

```sh
xcrun simctl boot "iPhone 17"            # if none is booted; `open -a Simulator` shows it
xcrun simctl openurl booted http://localhost:5199/
xcrun simctl io booted screenshot shot.png
xcrun simctl io booted recordVideo --codec=h264 clip.mp4   # stop with Ctrl-C (SIGINT)
```

simctl can't tap or run scripts in the page, so use it to load URLs and capture what Mobile Safari shows. To look at an animation frame by frame, record a clip and split it: `ffmpeg -i clip.mp4 -vf fps=60 frames/%03d.png`. Tilt and other motion-sensor input aren't available in the simulator.

## Native Safari on this Mac (WebDriver)

`safaridriver` drives the real Safari on this Mac, not an emulated one. It needs a one-time switch that only the user can turn on: Safari → Settings → Advanced → "Show features for web developers", then Developer → "Allow remote automation". `sudo safaridriver --enable` does the same. Until then, starting a session fails with "You must enable 'Allow remote automation'"; ask the user rather than working around it.

Start the driver as a background command; a `&` inside a one-off shell dies with that shell:

```sh
safaridriver -p 4444
```

Then talk W3C WebDriver to it:

```sh
S=$(curl -s -X POST localhost:4444/session -H 'content-type: application/json' \
  -d '{"capabilities":{"alwaysMatch":{"browserName":"safari"}}}' | node -pe 'JSON.parse(require("fs").readFileSync(0)).value.sessionId')
B=localhost:4444/session/$S
curl -s -X POST $B/url -H 'content-type: application/json' -d '{"url":"http://localhost:5199/"}'
curl -s -X POST $B/execute/sync -H 'content-type: application/json' -d '{"script":"return innerWidth","args":[]}'
curl -s $B/screenshot | node -e 'process.stdout.write(Buffer.from(JSON.parse(require("fs").readFileSync(0)).value, "base64"))' > shot.png
curl -s -X DELETE $B   # end the session; it closes the automation window
```

If creating a session hangs and then fails with "timed out while connecting to a Safari instance", Safari is blocked: an automation window left over from an interrupted session (orange, striped address bar), or a prompt waiting for the user. Ask the user to close that window or answer the prompt, or to quit Safari; don't quit their Safari yourself. Always end sessions with `DELETE`, even when a script fails.

Safari allows one session at a time, in its own window with a separate profile. The window can be resized (`POST $B/window/rect` with `{"width":390,"height":844}`), but that's still desktop Safari without touch. For a phone, use the simulator. WebDriver screenshots are too slow for animation. To catch per-frame glitches, log state from `requestAnimationFrame` in an `execute/async` script, or record the screen with `screencapture -v clip.mov`.
