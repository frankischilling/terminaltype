#!/usr/bin/env python3
"""Capture actual Kitty sessions for the README. Requires a desktop and Kitty."""
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target/release/monkeytype-tui"
OUTPUT = ROOT / "docs/images"


def capture(name, arguments, text=None):
    with tempfile.TemporaryDirectory(prefix="terminaltype-screenshot-") as directory:
        socket = f"{directory}/kitty.sock"
        address = f"unix:{socket}"
        process = subprocess.Popen([
            "kitty", "--config", "NONE", "--start-as", "minimized",
            "--listen-on", address, "-o", "allow_remote_control=yes",
            "-o", "remember_window_size=no", "-o", "initial_window_width=1400",
            "-o", "initial_window_height=900", "-o", "font_size=12",
            str(BINARY), "--data-dir", f"{directory}/data", "--seed", "42",
            *arguments,
        ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

        def remote(*args):
            result = subprocess.run(["kitty", "@", "--to", address, *args], capture_output=True, timeout=5)
            if result.returncode:
                raise RuntimeError(result.stderr.decode())

        try:
            deadline = time.monotonic() + 8
            while not os.path.exists(socket) and process.poll() is None and time.monotonic() < deadline:
                time.sleep(.02)
            if not os.path.exists(socket):
                raise RuntimeError(process.stderr.read().decode())
            remote("resize-os-window", "--unit", "pixels", "--width", "1400", "--height", "900")
            time.sleep(.4)
            if text:
                for char in text:
                    remote("send-text", "--bracketed-paste", "disable", char)
                    time.sleep(.09)
                time.sleep(.3)
            OUTPUT.mkdir(parents=True, exist_ok=True)
            remote("screenshot", "--match", "id:1", str(OUTPUT / name))
            print(OUTPUT / name, flush=True)
        finally:
            if process.poll() is None:
                try:
                    remote("send-key", "ctrl+c")
                    process.wait(timeout=3)
                except (RuntimeError, subprocess.TimeoutExpired):
                    process.terminate()
                    process.wait(timeout=3)


if __name__ == "__main__":
    if not BINARY.exists():
        raise SystemExit("Build first: cargo build --release --locked")
    capture("serika-time.png", ["--time", "30", "--theme", "serika_dark"])
    capture("nord-words.png", ["--words", "50", "--theme", "nord"], "I long some ")
    phrase = "The quick brown fox jumps over the lazy dog. Small steps make steady progress."
    capture("custom-result.png", ["--custom", phrase, "--theme", "serika_dark"], phrase)
