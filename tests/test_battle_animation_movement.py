"""#410: an attack object's `addq`/`subq.w #n,d16(a4)` coordinate writes.

The 68000 encodes them `0101 qqq s 01 101 100`: the quick value in bits 11-9
(0 means 8) and subtract in bit 8. The decoder read bits 3-1 and bit 0, which
for `d16(a4)` are always `$6C`'s, so every such write came out `addq.w #6`.
The Rust port (`movement.rs`) holds the same cases. No ROM needed.
"""

from __future__ import annotations

import unittest

from psiv_tools.battle_animations import FIGHTER_X_FIELD, FIGHTER_Y_FIELD, _coordinate_sites


def code(*words: int) -> bytes:
    return b"".join(word.to_bytes(2, "big") for word in words) + bytes(16)


class QuickWrites(unittest.TestCase):
    def test_each_write_keeps_its_own_quick_value_and_direction(self):
        rom = code(
            0x5F6C, FIGHTER_X_FIELD,  # subq.w #7
            0x536C, FIGHTER_Y_FIELD,  # subq.w #1
            0x506C, FIGHTER_X_FIELD,  # addq.w #8 (quick 0)
            0x5A6C, FIGHTER_Y_FIELD,  # addq.w #5
        )
        sites = _coordinate_sites(rom, 0, 32, 0x40)
        self.assertEqual(
            [(site["delta"], site["operation"]) for site in sites],
            [(-7, "subq.w"), (-1, "subq.w"), (8, "addq.w"), (5, "addq.w")],
        )


if __name__ == "__main__":
    unittest.main()
