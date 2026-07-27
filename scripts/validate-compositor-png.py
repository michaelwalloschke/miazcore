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
    if bits_per_pixel not in {24, 32}:
        fail("screenshot did not convert to an RGB bitmap")
    bitmap_width = int.from_bytes(data[18:22], "little", signed=True)
    bitmap_height = abs(int.from_bytes(data[22:26], "little", signed=True))
    if bitmap_width != width or bitmap_height != height:
        fail("screenshot bitmap dimensions changed during conversion")
    bytes_per_pixel = bits_per_pixel // 8
    row_bytes = ((width * bits_per_pixel + 31) // 32) * 4
    pixels = data[offset:]
    if len(pixels) < row_bytes * height:
        fail("screenshot bitmap pixel data is truncated")
    if not any(
        any(pixels[row * row_bytes + column * bytes_per_pixel + channel] > 8 for channel in range(3))
        for row in range(height)
        for column in range(width)
    ):
        fail("screenshot is all black")


if __name__ == "__main__":
    main()
