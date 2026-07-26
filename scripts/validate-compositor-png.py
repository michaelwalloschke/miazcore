#!/usr/bin/env python3
"""Fail closed unless a macOS compositor capture contains plausible frame data."""

from __future__ import annotations

import pathlib
import struct
import subprocess
import sys
import tempfile


def fail(message: str) -> None:
    raise SystemExit(f"compositor capture failed: {message}")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} <capture.png>")
    image = pathlib.Path(sys.argv[1])
    if not image.is_file() or image.stat().st_size < 100_000:
        fail("screenshot is implausibly small")
    header = image.read_bytes()[:24]
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        fail("screenshot is not PNG")
    width, height = struct.unpack(">II", header[16:24])
    if width < 1024 or height < 720:
        fail(f"screenshot is only {width}x{height}")

    with tempfile.TemporaryDirectory() as directory:
        bitmap = pathlib.Path(directory) / "capture.bmp"
        subprocess.run(
            ["sips", "-s", "format", "bmp", str(image), "--out", str(bitmap)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        data = bitmap.read_bytes()
    offset = int.from_bytes(data[10:14], "little")
    bits_per_pixel = int.from_bytes(data[28:30], "little")
    if bits_per_pixel != 32:
        fail("screenshot did not convert to 32-bit bitmap")
    pixels = data[offset:]
    if not any(
        any(channel > 8 for channel in pixel[:3])
        for pixel in zip(*[iter(pixels)] * 4)
    ):
        fail("screenshot is all black")


if __name__ == "__main__":
    main()
