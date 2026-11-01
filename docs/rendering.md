# Rendering and input

Terminaltype draws RGB pixels with cached font glyphs and sends them through the Kitty graphics protocol. The typing text uses the bundled Roboto Mono Regular at 32 px by default. Letters share a baseline; word margins follow Monkeytype's layout. Font rasterization can still differ slightly from a browser.

## Input and painting

A dedicated thread blocks on terminal input and timestamps events when it receives them. The main loop processes queued events, updates the typing test, and paints. A key press bypasses the animation frame deadline. Event draining has a time limit so a continuous stream of mouse events cannot hold off a paint.

The caret uses a 25 ms glide. The default animation cap is 120 Hz; `--fps` accepts values from 30 to 240. Static pages do not repaint continuously. The idle test updates its blinking caret, and timed tests use a monotonic clock.

Crossterm uses its level-triggered TTY backend. The PTY smoke test covers input arriving alongside a resize, as well as paste rejection and terminal cleanup.

## Tiles and caches

Each tile spans 16 terminal columns and 4 rows. Tile boundaries follow cell coordinates, and each tile has a reusable Kitty image ID. The encoder compares pixels in the affected area, compresses changed tiles, and replaces those images in a synchronized batch. Unchanged tiles send no data. Resize and exit free the old images.

This keeps terminal work proportional to tile size. Editing a frame inside a single full-screen image makes Kitty reconstruct, cache, and upload that image even when only a few pixels changed. Separate textures avoid that full-screen update for each letter or caret movement. See Kitty's [frame-loading implementation](https://github.com/kovidgoyal/kitty/blob/master/kitty/graphics.c) and [graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/).

The renderer caches glyph masks, static interface layers, and the visible words. Caret animation reuses the word layer. Typing, corrections, a changed theme, or a changed viewport invalidates the relevant cache.

## Measure the right stage

Build a release binary before testing:

```sh
cargo build --release --locked
./target/release/monkeytype-tui --benchmark --seed 42
./target/release/monkeytype-tui --benchmark --size 2560x1440 --seed 42
```

The CPU benchmark simulates 20 keys per second and 120 animation updates per second. It warms the caches for 120 frames, then measures 600 frames. The report includes median, p95, p99, maximum update time, and transmitted bytes. It excludes terminal decoding, GPU work, transport delay, and display scanout.

```sh
python3 tools/input_latency.py
```

The PTY test sends input to the release process, decodes its actual graphics packets, and waits for the new glyph pixels. It also queues mouse events ahead of typing. This measures the app's output path, including scheduling, but not what the terminal displays. Its timing assertions can fail on a busy machine.

```sh
python3 tools/kitty_latency.py target/release/monkeytype-tui
```

The Kitty test needs a desktop session and Pillow. It opens a minimized Kitty window and checks GPU screenshots after injecting characters. Its times include remote-control process startup, key injection, GPU readback, PNG encoding, and decoding. They are upper bounds, not hardware key-to-photon measurements. You can pass two binary paths to compare builds under the same conditions.

Keep these measurements separate when reporting results. The project does not claim identical end-to-end latency to the Monkeytype website.
