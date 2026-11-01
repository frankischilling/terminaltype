#!/usr/bin/env python3
"""Exercise the release binary through a real Unix PTY. No third-party modules."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time


ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target/release/monkeytype-tui"


def exercise(terminate=False):
    master, slave = pty.openpty()
    process = None
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 1200, 800))
        original = termios.tcgetattr(slave)

        def attach_terminal():
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

        with tempfile.TemporaryDirectory(prefix="monkeytype-pty-") as directory:
            process = subprocess.Popen(
                [str(BINARY), "--data-dir", directory, "--custom", "hello world"],
                stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach_terminal,
            )
            output = bytearray()

            def drain_until(predicate, seconds=5):
                deadline = time.monotonic() + seconds
                while not predicate() and time.monotonic() < deadline:
                    if select.select([master], [], [], 0.02)[0]:
                        output.extend(os.read(master, 65536))
                    if process.poll() is not None:
                        break
                if not predicate():
                    Path("/tmp/monkeytype-pty-failure.bin").write_bytes(output)
                    raise AssertionError(f"Timed out; process={process.poll()}, captured /tmp/monkeytype-pty-failure.bin")

            drain_until(lambda: b"\x1b[?2026l" in output)
            if terminate:
                process.send_signal(signal.SIGTERM)
            else:
                # Bracketed paste must not count as a typed test.
                first_updates = output.count(b"a=T,")
                os.write(master, b"\x1b[200~hello world\x1b[201~")
                os.write(master, b"h")
                drain_until(lambda: output.count(b"a=T,") > first_updates)
                # Resize while active, then finish using ordinary keystrokes.
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 1000, 640))
                process.send_signal(signal.SIGWINCH)
                os.write(master, b"ello world")
                history_file = Path(directory) / "history.json"
                drain_until(history_file.exists)
                history = json.loads(history_file.read_text())
                assert len(history) == 1
                assert history[0]["accuracy"] == 100
                assert history[0]["correct"] == 11
                assert b"Pasting" not in output  # graphics, rather than text-mode UI
                os.write(master, b"\x03")
            drain_until(lambda: process.poll() is not None)
            # Capture any final restoration sequences buffered in the PTY.
            while select.select([master], [], [], 0)[0]:
                output.extend(os.read(master, 65536))
            assert process.returncode == 0
            assert termios.tcgetattr(slave) == original, "terminal flags were not restored"
            assert b"\x1b[?1049l" in output, "alternate screen was not restored"
            print("PASS:", "SIGTERM cleanup" if terminate else "typing, paste rejection, resize, graphics patches, save, Ctrl+C cleanup")
    finally:
        if process and process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    if not BINARY.exists():
        raise SystemExit("Build first: cargo build --release --locked")
    exercise()
    exercise(terminate=True)
