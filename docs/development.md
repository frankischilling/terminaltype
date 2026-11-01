# Development

The application is Rust. Python is used only for the integration and screenshot tools. Build and run from the repository root:

```sh
cargo build --release --locked
./target/release/monkeytype-tui
```

`./run` locates Cargo on `PATH` or under `~/.cargo/bin` and starts the release build. Assets are embedded with `include_bytes!` and `include_str!`; the executable can run outside the checkout.

## Source map

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Arguments, event loop, previews, and CPU benchmark |
| `src/app.rs` | Pages, commands, settings changes, and input dispatch |
| `src/model.rs` | Test generation, timing, scoring, and local storage |
| `src/ui.rs` | Screen layout, drawing, hit targets, and visual caches |
| `src/gfx.rs` | RGB canvas, font masks, and PNG output |
| `src/terminal.rs` | Kitty tiles, graphics encoding, and terminal cleanup |
| `src/data.rs` | Embedded themes, dictionaries, quotes, and fonts |

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 tools/smoke_pty.py
```

The unit tests cover scoring, deadlines, corrections, Unicode character counts, word generation, typography, tile updates, screen layouts, and cached rendering. The Unix PTY test checks typing, paste rejection, resizing during input, saved results, and cleanup after Ctrl+C and SIGTERM. These checks run in CI on Linux.

Performance tools and their measurement boundaries are described in [rendering.md](rendering.md). Timing assertions are separate from CI's correctness checks.

## Previews and screenshots

For a PNG without opening a terminal:

```sh
./run --preview /tmp/test.png --seed 42
./run --preview /tmp/result.png --screen result
./run --preview /tmp/settings.png --screen settings --size 1280x800
```

The result preview uses synthetic fixture data. It does not save a result.

The README images are screenshots of actual running Kitty sessions. To replace them, build the release binary and run this from a desktop session:

```sh
python3 tools/capture_screenshots.py
```

The script opens minimized windows, uses isolated temporary profiles, types the examples through Kitty remote control, and saves PNGs under `docs/images`. It closes the windows afterward. Remote control is enabled only for those temporary Kitty instances. Capture dimensions depend on what the window manager allows.

## Assets and licensing

`assets/` contains fonts, the logo, theme colors, dictionaries, and quotes. Asset attributions and license files are listed in [NOTICE](../NOTICE). The upstream Monkeytype reference checkout is excluded from this repository and from Cargo packages. Preserve the credits and font licenses when changing or redistributing assets.
