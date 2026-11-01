#!/usr/bin/env python3
"""Measure real PTY input-to-result latency, including queued mouse traffic."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import tempfile
import termios
import time
import struct
import base64
import re
import zlib

BINARY = Path(__file__).resolve().parent.parent / "target/release/monkeytype-tui"


class Graphics:
    """Apply the actual Kitty RGB packets, rather than timing model changes."""
    def __init__(self):
        self.buffer = bytearray()
        self.metadata = None
        self.chunks = []
        self.width = 0
        self.pixels = bytearray()
        self.cursor = (0, 0)

    def feed(self, data):
        self.buffer.extend(data)
        while True:
            match = re.search(rb"\x1b_G([^;]*);(.*?)\x1b\\", self.buffer, re.S)
            if not match:
                return
            control, payload = match.groups()
            for row, col in re.findall(rb"\x1b\[(\d+);(\d+)H", self.buffer[:match.start()]):
                self.cursor = ((int(col)-1)*10, (int(row)-1)*20)
            del self.buffer[:match.end()]
            fields = dict(part.split("=", 1) for part in control.decode().split(",") if "=" in part)
            if fields.get("a") in ("T", "f") and "s" in fields:
                self.metadata = fields
                self.metadata["origin"] = self.cursor
                self.chunks = []
            if payload:
                self.chunks.append(payload)
            if fields.get("m", "0") != "0" or not self.chunks or not self.metadata:
                continue
            metadata = self.metadata
            pixels = zlib.decompress(base64.b64decode(b"".join(self.chunks)))
            width, height = int(metadata["s"]), int(metadata["v"])
            if metadata["a"] == "T":
                if not self.pixels:
                    self.width, self.pixels = 1200, bytearray(1200*800*3)
                x, y = metadata["origin"]
                for row in range(height):
                    start = ((y + row) * self.width + x) * 3
                    self.pixels[start:start + width * 3] = pixels[row * width * 3:(row + 1) * width * 3]
            else:
                x, y = int(metadata["x"]), int(metadata["y"])
                for row in range(height):
                    start = ((y + row) * self.width + x) * 3
                    self.pixels[start:start + width * 3] = pixels[row * width * 3:(row + 1) * width * 3]
            self.metadata, self.chunks = None, []

    def letter_visible(self, column):
        # Fixed 1200x800, original 32px Roboto Mono, .3em word margin.
        left = int(24 + 9.6 + column * 19.203125)
        return any(
            self.pixels[(y * self.width + x) * 3 + 2] > 150
            for y in range(340, 390)
            for x in range(left + 3, left + 16)
        ) if self.width else False


def render_latency():
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 1200, 800))
    def attach():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    with tempfile.TemporaryDirectory() as directory:
        process = subprocess.Popen(
            [str(BINARY), "--custom", "mmmmmmmmmmmm end", "--data-dir", directory, "--fps", "30"],
            stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach,
        )
        graphics = Graphics()
        def pump_until(predicate, timeout):
            deadline = time.monotonic() + timeout
            while not predicate() and time.monotonic() < deadline:
                if select.select([master], [], [], 0.001)[0]:
                    graphics.feed(os.read(master, 65536))
            assert predicate(), "newly typed glyph was not rendered"
        try:
            # Wait until the complete initial tile batch has been emitted.
            pump_until(lambda: graphics.width and any(graphics.pixels[(y*1200+50)*3] for y in range(790,800)), 5)
            samples = []
            for column in range(12):
                # Include mouse traffic and type during the previous glide.
                start = time.perf_counter()
                os.write(master, b"\x1b[<35;1;1M" * 20 + b"m")
                pump_until(lambda: graphics.letter_visible(column), 1)
                samples.append((time.perf_counter() - start) * 1000)
            return dict(fps_cap=30, glyph_samples=len(samples), key_to_graphics_p50_ms=round(sorted(samples)[len(samples)//2], 2), key_to_graphics_max_ms=round(max(samples), 2))
        finally:
            process.terminate()
            process.wait(timeout=2)
            os.close(master)
            os.close(slave)


def measure(mouse_events):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 1200, 800))

    def attach():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    with tempfile.TemporaryDirectory() as directory:
        process = subprocess.Popen(
            [str(BINARY), "--custom", "hello world", "--data-dir", directory],
            stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach,
        )
        output = bytearray()

        def pump_until(predicate, timeout):
            deadline = time.monotonic() + timeout
            while not predicate() and time.monotonic() < deadline:
                if select.select([master], [], [], 0.001)[0]:
                    output.extend(os.read(master, 65536))
            return predicate()

        try:
            assert pump_until(lambda: b"\x1b[?2026l" in output, 5), "no initial frame"
            payload = b"\x1b[<35;1;1M" * mouse_events + b"hello world"
            start = time.perf_counter()
            os.write(master, payload)
            history = Path(directory) / "history.json"
            completed = pump_until(history.exists, 5)
            elapsed_ms = (time.perf_counter() - start) * 1000
            if completed:
                assert json.loads(history.read_text())[0]["accuracy"] == 100
            return dict(mouse_events=mouse_events, completed=completed, input_to_saved_result_ms=round(elapsed_ms, 2))
        finally:
            process.terminate()
            pump_until(lambda: process.poll() is not None, 1)
            if process.poll() is None:
                process.kill()
            process.wait()
            os.close(master)
            os.close(slave)


if __name__ == "__main__":
    results = [measure(count) for count in (0, 240)]
    print(json.dumps(results, indent=2))
    assert all(r["completed"] and r["input_to_saved_result_ms"] < 100 for r in results), "input processing exceeded 100 ms"
    rendered = render_latency()
    print(json.dumps(rendered, indent=2))
    assert rendered["key_to_graphics_max_ms"] < 25, "keystrokes waited for the 30 Hz animation deadline"
