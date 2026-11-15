"""Ship caption decoding and the tape-35 field-control measurement boundary."""

from pathlib import Path
import unittest

from oracle.ship_flight import measure, measure_fades
from psiv_tools.ship_menu_pack import FLIGHT_CAPTION_PTRS, ShipMenuError, flight_captions

ROOT = Path(__file__).resolve().parent.parent


class CaptionCase(unittest.TestCase):
    def test_captions_use_pointer_order_dialogue_font_and_keep_spacing(self):
        rom = bytearray(FLIGHT_CAPTION_PTRS + 128)
        for world in range(6):
            entry = FLIGHT_CAPTION_PTRS + world * 4
            ptr = FLIGHT_CAPTION_PTRS + 24 + world * 4
            rom[entry:entry + 4] = ptr.to_bytes(4, "big")
            # Dialogue 'a' is $1B; the window charset would decode this as 0.
            rom[ptr:ptr + 4] = bytes([0, 27, world + 1, 255])
        captions = flight_captions(bytes(rom))
        self.assertEqual([row["world"] for row in captions], list(range(6)))
        self.assertEqual(captions[0]["text"], " aA")
        self.assertTrue(all(row["instant"] and row["glyph_count"] == 3 for row in captions))
        # Negative controls: dangling pointer and a line without its terminator.
        rom[FLIGHT_CAPTION_PTRS:FLIGHT_CAPTION_PTRS + 4] = (len(rom)).to_bytes(4, "big")
        with self.assertRaisesRegex(ShipMenuError, "outside"):
            flight_captions(bytes(rom))
        rom[FLIGHT_CAPTION_PTRS:FLIGHT_CAPTION_PTRS + 4] = (FLIGHT_CAPTION_PTRS + 64).to_bytes(4, "big")
        with self.assertRaisesRegex(ShipMenuError, "bounded"):
            flight_captions(bytes(rom))

    def test_local_rom_captions_are_six_complete_instant_lines(self):
        rom = ROOT / "Phantasy Star IV (USA).md"
        if not rom.is_file():
            self.skipTest("local US ROM absent; flight caption extraction skipped")
        captions = flight_captions(rom.read_bytes())
        self.assertEqual(len(captions), 6)
        self.assertTrue(all(0 < row["glyph_count"] <= 32 for row in captions))
        self.assertTrue(all(row["text"].startswith(" ") for row in captions))


class ControlBoundaryCase(unittest.TestCase):
    @staticmethod
    def row(frame, map_id, mode, routine):
        return dict(frame=str(frame), map_index=f"{map_id:04X}",
                    game_mode=f"{mode:04X}", game_mode_routine=f"{routine:04X}")

    def test_landing_write_reload_and_place_name_do_not_count_as_control(self):
        rows = [self.row(7401, 0xBF, 12, 12), self.row(9053, 0x18D, 12, 12),
                self.row(9086, 0x18D, 8, 0), self.row(9138, 0x18D, 12, 28),
                self.row(9262, 0x18D, 12, 0)]
        self.assertEqual(measure(rows, 0x18D)["frames_to_control"], 1861)
        # Negative control: the tape must actually reach field control.
        with self.assertRaisesRegex(ValueError, "without field control"):
            measure(rows[:-1], 0x18D)


class FadeBoundaryCase(unittest.TestCase):
    def test_fade_checker_rejects_wrong_words_display_timing_and_call_multiplicity(self):
        rows = []
        for frame in range(100, 116):
            elapsed = frame - 100
            row = dict(frame=str(frame), vdp_reg1="8124" if elapsed == 0 else "8164")
            for word in range(64):
                row[f"saved_{word}"] = "0EEE"
                row[f"live_{word}"] = f"{min(elapsed, 14) * 0x111:04X}"
            rows.append(row)
        calls = [dict(pc="0645D6", frame="100"), dict(pc="0645DC", frame="116")]
        self.assertEqual(measure_fades(rows, calls, [0x645D6])[0]["return"], 116)
        for changed in (calls[:1], calls + calls, list(reversed(calls))):
            with self.assertRaisesRegex(ValueError, "omitted, duplicated or reordered"):
                measure_fades(rows, changed, [0x645D6])
        for field, value in [("live_1", "0EEE"), ("vdp_reg1", "8124")]:
            altered = [dict(row) for row in rows]
            altered[1][field] = value
            with self.assertRaisesRegex(ValueError, "wrong"):
                measure_fades(altered, calls, [0x645D6])
        with self.assertRaisesRegex(ValueError, "16 frames"):
            measure_fades(rows, [calls[0], dict(pc="0645DC", frame="115")], [0x645D6])


if __name__ == "__main__":
    unittest.main()
