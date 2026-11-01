#!/usr/bin/env python3
"""Check pixels read back from a real Kitty GPU window. Requires Pillow and Kitty.

Remote-control injection and screenshot overhead are INCLUDED, so these are
upper bounds, not hardware key-to-photon measurements. The window is minimized.
"""
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time

from PIL import Image


def measure(binary):
    with tempfile.TemporaryDirectory(prefix="monkeytype-kitty-latency-") as directory:
        socket = f"{directory}/kitty.sock"
        address = f"unix:{socket}"
        process = subprocess.Popen([
            "kitty", "--config", "NONE", "--start-as", "minimized",
            "--listen-on", address, "-o", "allow_remote_control=yes",
            str(Path(binary).resolve()), "--data-dir", f"{directory}/data",
            "--custom", "mmmmmmmmmmmmmmmm end",
        ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

        def remote(*args):
            result = subprocess.run(["kitty", "@", "--to", address, *args], capture_output=True, timeout=5)
            if result.returncode:
                raise RuntimeError(result.stderr.decode())

        def visible_pixels():
            screenshot = f"{directory}/screen.png"
            remote("screenshot", screenshot)
            with Image.open(screenshot) as image:
                w, h = image.size
                roi = image.convert("RGB").crop((20, int(h*.38), min(430,w-20), int(h*.72)))
                return sum(r > 170 and g > 170 and b > 150 for r,g,b in roi.get_flattened_data())

        try:
            deadline = time.monotonic()+8
            while not os.path.exists(socket) and process.poll() is None and time.monotonic()<deadline:
                time.sleep(.01)
            if not os.path.exists(socket):
                raise RuntimeError(process.stderr.read().decode())
            visible_pixels()  # Wait for and read back the initial GPU frame.
            previous = 0
            samples = []
            for _ in range(12):
                start = time.perf_counter()
                remote("send-key", "m")
                while True:
                    pixels = visible_pixels()
                    if pixels > previous+10:
                        previous = pixels
                        samples.append((time.perf_counter()-start)*1000)
                        break
                    if time.perf_counter()-start>2:
                        raise AssertionError("Kitty did not display the newly typed letter")
            return dict(binary=binary, samples=len(samples), key_to_kitty_pixels_median_ms=round(statistics.median(samples),2), key_to_kitty_pixels_max_ms=round(max(samples),2), includes="remote-control process startup, key injection, GPU readback, PNG encoding and decoding")
        finally:
            if process.poll() is None:
                try:
                    remote("send-key", "ctrl+c")
                    process.wait(timeout=3)
                except (RuntimeError, subprocess.TimeoutExpired):
                    process.terminate()
                    process.wait(timeout=3)


if __name__ == "__main__":
    binaries = sys.argv[1:] or ["target/release/monkeytype-tui"]
    print(json.dumps([measure(binary) for binary in binaries], indent=2))
