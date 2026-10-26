"""`png.image_sha256`: an image's identity is what it decodes to (#194).

Deflate bytes depend on the zlib build that wrote them, so the digest has to
see through the encoding and still see every pixel, colour and dimension.
"""

import re
import struct
import unittest
import zlib
from pathlib import Path

from psiv_tools import png

ROOT = Path(__file__).resolve().parent.parent
#: A digest of encoded PNG bytes, the way every one was written before #194:
#: a `png*_sha256` field filled from `hashlib`, or `hashlib` over an image.
FILE_BYTE_DIGEST = re.compile(
    r"png\w*_sha256.*hashlib\.sha256|hashlib\.sha256.*png\w*_sha256"
    r"|hashlib\.sha256\(\w*image\w*\)"
)

PALETTE = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255)]
PIXELS = bytes([0, 1, 2, 3, 3, 2, 1, 0, 1, 1, 2, 2, 3, 3, 0, 0])


def chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    out, pos = [], 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        out.append((data[pos + 4:pos + 8], data[pos + 8:pos + 8 + length]))
        pos += 12 + length
    return out


def assemble(parts: list[tuple[bytes, bytes]]) -> bytes:
    out = bytearray(png.PNG_SIGNATURE)
    for kind, payload in parts:
        out += struct.pack(">I", len(payload)) + kind + payload
        out += struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    return bytes(out)


def filtered(rows: list[bytes], kind: int, bpp: int) -> bytes:
    """Apply one PNG filter type to every row, the way another encoder might."""
    out, previous = bytearray(), bytes(len(rows[0]))
    for row in rows:
        line = bytearray()
        for x, value in enumerate(row):
            left = row[x - bpp] if x >= bpp else 0
            up = previous[x]
            corner = previous[x - bpp] if x >= bpp else 0
            predictor = (0, left, up, (left + up) >> 1, png._paeth(left, up, corner))[kind]
            line.append((value - predictor) & 0xFF)
        out += bytes([kind]) + line
        previous = row
    return bytes(out)


def reencode(data: bytes, level: int, filter_kind: int = 0, extra: bytes = b"") -> bytes:
    """The same image with different IDAT bytes: another level, filter or
    chunk split, plus an optional ancillary chunk before IDAT."""
    parts = chunks(data)
    header = dict(parts)[b"IHDR"]
    width, height = struct.unpack(">II", header[:8])
    bpp = {png.COLOR_TYPE_INDEXED: 1, png.COLOR_TYPE_RGB: 3}[header[9]]
    raw = zlib.decompress(b"".join(p for k, p in parts if k == b"IDAT"))
    stride = width * bpp
    rows = [raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)] for y in range(height)]
    stream = zlib.compress(filtered(rows, filter_kind, bpp), level)
    rebuilt = [(k, p) for k, p in parts if k not in (b"IDAT", b"IEND")]
    if extra:
        rebuilt.append((b"tEXt", extra))
    half = len(stream) // 2
    rebuilt += [(b"IDAT", stream[:half]), (b"IDAT", stream[half:]), (b"IEND", b"")]
    return assemble(rebuilt)


class TestImageSha256(unittest.TestCase):
    def setUp(self):
        self.image = png.encode_indexed(4, 4, PIXELS, PALETTE, transparent=(0,))
        self.digest = png.image_sha256(self.image)

    def test_the_encoding_does_not_change_the_digest(self):
        for level in (0, 1, 6, 9):
            for kind in range(5):
                with self.subTest(level=level, filter=kind):
                    other = reencode(self.image, level, kind)
                    self.assertNotEqual(other, self.image)
                    self.assertEqual(png.image_sha256(other), self.digest)

    def test_an_annotation_chunk_is_not_the_image(self):
        other = reencode(self.image, 9, extra=b"Comment\x00another encoder")
        self.assertEqual(png.image_sha256(other), self.digest)

    def test_rgb_images_see_through_the_encoding_too(self):
        image = png.encode_rgb(2, 2, bytes(range(12)))
        for kind in range(5):
            with self.subTest(filter=kind):
                self.assertEqual(
                    png.image_sha256(reencode(image, 1, kind)), png.image_sha256(image)
                )

    def test_one_pixel_changes_the_digest(self):
        for index in range(len(PIXELS)):
            with self.subTest(pixel=index):
                pixels = bytearray(PIXELS)
                pixels[index] = (pixels[index] + 1) % len(PALETTE)
                other = png.encode_indexed(4, 4, bytes(pixels), PALETTE, transparent=(0,))
                self.assertNotEqual(png.image_sha256(other), self.digest)

    def test_a_palette_colour_changes_the_digest(self):
        palette = list(PALETTE)
        palette[3] = (0, 0, 254)
        other = png.encode_indexed(4, 4, PIXELS, palette, transparent=(0,))
        self.assertNotEqual(png.image_sha256(other), self.digest)

    def test_transparency_changes_the_digest(self):
        for transparent in ((), (1,), (0, 1)):
            with self.subTest(transparent=transparent):
                other = png.encode_indexed(4, 4, PIXELS, PALETTE, transparent=transparent)
                self.assertNotEqual(png.image_sha256(other), self.digest)

    def test_the_same_bytes_in_another_shape_change_the_digest(self):
        for width, height in ((2, 8), (8, 2), (16, 1)):
            with self.subTest(size=(width, height)):
                other = png.encode_indexed(width, height, PIXELS, PALETTE, transparent=(0,))
                self.assertNotEqual(png.image_sha256(other), self.digest)

    def test_a_damaged_file_is_refused_rather_than_named(self):
        damaged = bytearray(self.image)
        damaged[-20] ^= 0xFF  # inside the IDAT payload: its CRC no longer holds
        with self.assertRaises(png.PngError):
            png.image_sha256(bytes(damaged))
        with self.assertRaises(png.PngError):
            png.image_sha256(self.image[8:])
        with self.assertRaises(png.PngError):
            png.image_sha256(self.image[:-30])

    def test_a_format_it_does_not_decode_is_refused(self):
        parts = chunks(self.image)
        header = bytearray(parts[0][1])
        header[8] = 4  # bit depth 4
        with self.assertRaises(png.PngError):
            png.image_sha256(assemble([(b"IHDR", bytes(header))] + parts[1:]))


class NoImageIsNamedByItsFileBytes(unittest.TestCase):
    """The guard that keeps the class fixed: an image digest is `image_sha256`."""

    def test_the_pattern_catches_each_way_it_was_written(self):
        for line in (
            '"png_sha256": hashlib.sha256(image).hexdigest(),',
            'self.assertEqual(entry["png_over_sha256"], hashlib.sha256(blob).hexdigest())',
            '"sha256": hashlib.sha256(overlay_image).hexdigest(),',
            'self.assertEqual(hashlib.sha256(image).hexdigest(), PINNED)',
        ):
            with self.subTest(line=line):
                self.assertRegex(line, FILE_BYTE_DIGEST)
        for line in (
            '"png_sha256": png.image_sha256(image),',
            '"json_sha256": hashlib.sha256(data).hexdigest(),',
            '"decompressed_sha256": hashlib.sha256(raw).hexdigest(),',
        ):
            with self.subTest(line=line):
                self.assertNotRegex(line, FILE_BYTE_DIGEST)

    def test_no_python_file_hashes_png_bytes_to_name_an_image(self):
        from tools.repo_files import repo_files

        found = [
            f"{path}:{number}: {line.strip()}"
            for path in repo_files(ROOT, ("*.py",))
            if path != "tests/test_png.py"
            for number, line in enumerate(
                (ROOT / path).read_text(encoding="utf-8").splitlines(), 1
            )
            if FILE_BYTE_DIGEST.search(line)
        ]
        self.assertEqual(found, [], "use png.image_sha256 for a digest that names an image")


if __name__ == "__main__":
    unittest.main()
