import unittest

from oracle.scroll_state import decode_scroll_state


def _region(raw: bytes, address: str = "0x0000") -> dict:
    return {
        "address": address,
        "size_bytes": len(raw),
        "bytes_hex": raw.hex(),
    }


def _word_region(words: list[int], size: int, address: str = "0x0000") -> dict:
    raw = b"".join(word.to_bytes(2, "big") for word in words)
    return _region(raw.ljust(size, b"\0"), address)


class TestScrollState(unittest.TestCase):
    def test_missing_receipt_is_reported_without_fabricating_zeroes(self):
        decoded = decode_scroll_state({"regions": {}}, grand_cross=0)
        self.assertFalse(decoded["present"])
        self.assertEqual(
            decoded["missing_regions"],
            ["camera", "camera_step_counters", "h_int_state"],
        )

    def test_camera_and_split_receipt_decode_as_retail_numbers(self):
        camera = b"".join((value << 16).to_bytes(4, "big", signed=True) for value in (0, 3, 0, 3))
        h_int = (
            b"\x4e\xf9\x00\x00\x07\x58\x00\x00\x00\x00\x00\x00"
            b"\x00\x00\x00\x00\x00\x00"
        )
        registers = bytearray(0x20)
        registers[11] = 0
        registers[13] = 0x37
        regions = {
            "camera": _region(camera, "0xFFFFEF90"),
            "camera_step_counters": _region(bytes(0x10), "0xFFFFEC50"),
            "h_int_state": _region(h_int, "0xFFFFECB0"),
            "hscroll_work_buffer": _region(bytes(0x1C0), "0xFFFF60E0"),
            "vsram_shadow": _region(bytes(0x1C0), "0xFFFF6000"),
            "vdp_registers": _region(bytes(registers)),
            "vdp_vsram": _word_region([0x0012, 0x0034], 0x80),
            "vdp_hscroll_table": _word_region([0x0056, 0x0078], 0x400, "0xDC00"),
        }
        decoded = decode_scroll_state({"regions": regions}, grand_cross=0)
        self.assertTrue(decoded["present"])
        self.assertEqual(decoded["camera"]["x_fg"]["whole_pixels"], 3)
        self.assertEqual(decoded["camera"]["x_bg"]["raw"], "0x00030000")
        self.assertFalse(decoded["h_int"]["enabled"])
        self.assertEqual(decoded["hardware"]["registers"]["hscroll_mode"], "full_screen")
        self.assertEqual(decoded["hardware"]["hscroll_table"]["plane_a"], "0x0056")
        self.assertEqual(decoded["hardware"]["vsram"]["plane_b"], "0x0034")
        self.assertEqual(decoded["window_rows"]["plane_a_source"], "VDP VSRAM[0]")
        self.assertEqual(decoded["placement_provenance"]["grand_cross"], 0)
        self.assertEqual(
            decoded["placement_provenance"]["plane_window_residue_pixels"],
            {"x": 1, "y": 1},
        )


if __name__ == "__main__":
    unittest.main()
