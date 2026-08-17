"""Compare a 3x Godot presentation capture with a 320x224 oracle frame.

Godot's debug screenshots are normally 1280x800.  The field camera renders
the retail 320x224 surface at 3x, centred in that viewport, so this tool crops
that surface and samples one pixel from each 3x3 block before calculating the
RGB root-mean-square error.  It also accepts already-normalized 320x224 PNGs.
"""

from __future__ import annotations

import argparse
import math
import struct
import zlib
from pathlib import Path


class PngError(ValueError):
    pass


def read_png(path: Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise PngError(f"{path} is not a PNG")
    offset = 8
    width = height = color_type = bit_depth = None
    palette: bytes | None = None
    alpha: bytes | None = None
    compressed = bytearray()
    while offset < len(data):
        size = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4 : offset + 8]
        payload = data[offset + 8 : offset + 8 + size]
        offset += size + 12
        if kind == b"IHDR":
            width, height, bit_depth, color_type, compression, filtering, interlace = (
                struct.unpack(">IIBBBBB", payload)
            )
            if (bit_depth, compression, filtering, interlace) != (8, 0, 0, 0):
                raise PngError("only non-interlaced 8-bit PNGs are supported")
        elif kind == b"PLTE":
            palette = payload
        elif kind == b"tRNS":
            alpha = payload
        elif kind == b"IDAT":
            compressed += payload
        elif kind == b"IEND":
            break
    if None in (width, height, color_type) or not compressed:
        raise PngError(f"{path} has no usable image data")
    channels = {2: 3, 3: 1, 6: 4}.get(color_type)
    if channels is None:
        raise PngError(f"unsupported PNG colour type {color_type}")
    raw = zlib.decompress(compressed)
    stride = width * channels
    if len(raw) != height * (stride + 1):
        raise PngError(f"{path} scanline size is inconsistent")
    rows: list[bytes] = []
    previous = bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_type = raw[cursor]
        cursor += 1
        row = bytearray(raw[cursor : cursor + stride])
        cursor += stride
        for index in range(stride):
            left = row[index - channels] if index >= channels else 0
            above = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 1:
                row[index] = (row[index] + left) & 0xFF
            elif filter_type == 2:
                row[index] = (row[index] + above) & 0xFF
            elif filter_type == 3:
                row[index] = (row[index] + ((left + above) // 2)) & 0xFF
            elif filter_type == 4:
                estimate = left + above - upper_left
                distances = (abs(estimate - left), abs(estimate - above), abs(estimate - upper_left))
                predictor = (left, above, upper_left)[distances.index(min(distances))]
                row[index] = (row[index] + predictor) & 0xFF
            elif filter_type != 0:
                raise PngError(f"unsupported PNG filter {filter_type}")
        rows.append(bytes(row))
        previous = row

    rgb = bytearray()
    for row in rows:
        if color_type == 2:
            rgb += row
        elif color_type == 6:
            for index in range(0, len(row), 4):
                red, green, blue, opacity = row[index : index + 4]
                if opacity == 0:
                    rgb += b"\x00\x00\x00"
                elif opacity == 255:
                    rgb += bytes((red, green, blue))
                else:
                    rgb += bytes(
                        channel * opacity // 255
                        for channel in (red, green, blue)
                    )
        else:
            if palette is None:
                raise PngError("indexed PNG has no palette")
            for value in row:
                start = value * 3
                opacity = 255 if alpha is None or value >= len(alpha) else alpha[value]
                colour = palette[start : start + 3]
                rgb += bytes(channel * opacity // 255 for channel in colour)
    return width, height, bytes(rgb)


def normalize_godot(width: int, height: int, rgb: bytes) -> bytes:
    if (width, height) == (320, 224):
        return rgb
    expected = (960, 672)
    if width < expected[0] or height < expected[1]:
        raise PngError(f"Godot capture is {width}x{height}, expected 320x224 or at least 960x672")
    left = (width - expected[0]) // 2
    top = (height - expected[1]) // 2
    normalized = bytearray()
    for y in range(224):
        for x in range(320):
            source = ((top + y * 3) * width + left + x * 3) * 3
            normalized += rgb[source : source + 3]
    return bytes(normalized)


def rmse(left: bytes, right: bytes) -> float:
    if len(left) != len(right):
        raise PngError("images do not have equal RGB sizes")
    squared = sum((a - b) ** 2 for a, b in zip(left, right))
    return math.sqrt(squared / len(left))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("godot", type=Path)
    parser.add_argument("oracle", type=Path)
    args = parser.parse_args()
    godot_width, godot_height, godot_rgb = read_png(args.godot)
    oracle_width, oracle_height, oracle_rgb = read_png(args.oracle)
    if (oracle_width, oracle_height) != (320, 224):
        raise SystemExit(f"oracle frame is {oracle_width}x{oracle_height}, expected 320x224")
    normalized = normalize_godot(godot_width, godot_height, godot_rgb)
    print(
        f"godot={godot_width}x{godot_height} normalized=320x224 "
        f"oracle={oracle_width}x{oracle_height} rmse={rmse(normalized, oracle_rgb):.6f}"
    )


if __name__ == "__main__":
    main()
