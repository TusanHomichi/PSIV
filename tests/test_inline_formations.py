"""The formations battle objects keep inline (Fusion's and COMBINE's).

`psiv_tools.formations.extract_inline_formations` reads each record from its
offset and accepts it only when exactly one `lea (<offset>).l, a0` loads it.
The constructed controls never copy cartridge bytes; the ROM-gated test reads
the retail records.
"""
import unittest
from pathlib import Path

from psiv_tools.battle_pack import build_formations
from psiv_tools.core import read_rom
from psiv_tools.formations import (INLINE_FORMATIONS, LEA_ABS_A0, FormationError,
                                   extract_inline_formations)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


def image(records: dict[int, bytes], loads: dict[int, int]) -> bytes:
    """A blank image holding `records` at their offsets and `loads[offset]`
    copies of the `lea` that names each."""
    data = bytearray(0x30000)
    cursor = 0x100
    for offset, record in records.items():
        data[offset:offset + len(record)] = record
        for _ in range(loads.get(offset, 1)):
            reference = LEA_ABS_A0 + offset.to_bytes(4, "big")
            data[cursor:cursor + len(reference)] = reference
            cursor += 16
    return bytes(data)


#: A constructed record: header (ambush 1, run 2, drop 3, item 4, one enemy,
#: group masks), enemy 9 at position 5, terminator.
RECORD = bytes([1, 2, 3, 4, 1, 1, 0, 9, 5, 0xFF])


class Constructed(unittest.TestCase):
    def records(self, loads=None, record=RECORD):
        offsets = [spec["offset"] for spec in INLINE_FORMATIONS]
        return image({offset: record for offset in offsets},
                     loads or {offset: 1 for offset in offsets})

    def test_each_record_is_read_to_its_terminator(self):
        parsed = extract_inline_formations(self.records())
        self.assertEqual([p["label"] for p in parsed],
                         [spec["label"] for spec in INLINE_FORMATIONS])
        for record in parsed:
            self.assertEqual(record["run_agility"], 2)
            self.assertEqual([e["enemy"]["id"] for e in record["enemies"]], [9])
            self.assertEqual(record["enemies"][0]["position"], 5)
            self.assertEqual(record["raw_hex"], RECORD.hex())

    def test_a_record_no_lea_loads_is_refused(self):
        first = INLINE_FORMATIONS[0]["offset"]
        loads = {spec["offset"]: 1 for spec in INLINE_FORMATIONS}
        loads[first] = 0
        with self.assertRaisesRegex(FormationError, "expected one `lea`"):
            extract_inline_formations(self.records(loads))

    def test_a_record_two_leas_load_is_refused(self):
        first = INLINE_FORMATIONS[0]["offset"]
        loads = {spec["offset"]: 1 for spec in INLINE_FORMATIONS}
        loads[first] = 2
        with self.assertRaisesRegex(FormationError, "found 2"):
            extract_inline_formations(self.records(loads))

    def test_a_record_without_a_terminator_is_refused(self):
        with self.assertRaisesRegex(FormationError, "no terminator"):
            extract_inline_formations(self.records(record=bytes(range(1, 17))))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class Retail(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_fusion_and_combine_each_seat_one_enemy(self):
        parsed = {p["label"]: p for p in extract_inline_formations(self.data)}
        # BattleObj_Fusion's MetaSlug and loc_23C84's TwinArms, both in slot 1
        # at position $14 (ps4.asm:35846-35847, 47505).
        self.assertEqual([(e["slot"], e["enemy"]["id"], e["position"])
                          for e in parsed["loc_1A2F4"]["enemies"]], [(1, 36, 0x14)])
        self.assertEqual([(e["slot"], e["enemy"]["id"], e["position"])
                          for e in parsed["loc_23D00"]["enemies"]], [(1, 87, 0x14)])
        # Lane A6: the WorkerPods' COMBINE seats one LifeDeletr (27571-27572),
        # FractOoze's FISSION four JR.OOZE at 8, $10, $18, $20 (35075-35076),
        # InfantWorm's NOTHING one SandWorm (45818).
        self.assertEqual([(e["slot"], e["enemy"]["id"], e["position"])
                          for e in parsed["loc_1308C"]["enemies"]], [(1, 26, 0x14)])
        self.assertEqual([(e["slot"], e["enemy"]["id"], e["position"])
                          for e in parsed["loc_1987E"]["enemies"]],
                         [(1, 35, 0x08), (2, 35, 0x10), (3, 35, 0x18), (4, 35, 0x20)])
        self.assertEqual([(e["slot"], e["enemy"]["id"], e["position"])
                          for e in parsed["loc_224E8"]["enemies"]], [(1, 80, 0x14)])
        for record in parsed.values():
            self.assertEqual(record["run_agility"], 0)
            self.assertTrue(record["enemy_count_matches_entries"])

    def test_the_pack_carries_them_under_its_own_field_names(self):
        inline = build_formations(self.data)["inline_formations"]
        self.assertEqual([r["label"] for r in inline],
                         ["loc_1A2F4", "loc_23D00", "loc_1308C", "loc_1987E", "loc_224E8"])
        for record in inline:
            self.assertEqual(record["run_chance"], 0)
            self.assertNotIn("run_agility", record)
            self.assertEqual(set(record["enemies"][0]), {"slot", "enemy_id", "position",
                                                        "groups"})


if __name__ == "__main__":
    unittest.main()
