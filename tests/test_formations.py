import unittest
from pathlib import Path

from psiv_tools.core import extract_all, read_rom
from psiv_tools.formations import (
    BOSS_FORMATION_BLOCK,
    FORMATION_BLOCKS,
    FORMATION_INDEX_BLOCK,
    FormationError,
    decompress_block,
    split_formation_records,
)
from psiv_tools.kosinski import KosinskiError, decompress

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
ORACLE_DIR = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm" / "battles"

# Uncompressed formation sources in the public disassembly, paired with the
# compressed retail blob they must reproduce.
ORACLE_FILES = {
    "block_1": "battle_formations_1.asm",
    "block_2": "battle_formations_2.asm",
    "block_3": "battle_formations_3.asm",
    "block_4": "battle_formations_4.asm",
    "boss": "boss_formations.asm",
}


def read_oracle_bytes(path: Path) -> bytes:
    """Read a `dc.b`-only data listing from the disassembly.

    These files are pure data, never assembled by this project. Anything other
    than a comment, blank line, or `dc.b` directive means the assumption no
    longer holds and the oracle can no longer be trusted, so fail loudly.
    """
    out = bytearray()
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        directive, _, operands = line.partition("\t")
        directive = directive.strip()
        if directive != "dc.b":
            raise AssertionError(
                f"{path.name}:{lineno}: expected a dc.b data listing, got {line!r}"
            )
        for token in operands.split(","):
            token = token.strip()
            if not token.startswith("$"):
                raise AssertionError(f"{path.name}:{lineno}: non-hex operand {token!r}")
            out.append(int(token[1:], 16))
    return bytes(out)


class KosinskiWriter:
    """Minimal Kosinski encoder used only to build test fixtures.

    It reproduces the format's interleaving quirk faithfully: a description
    field is reserved in the byte stream at the moment the previous one fills,
    which is *before* the operand bytes of the element whose flag bit filled
    it. KosDecomp reloads at that same point, so a decoder that gets this wrong
    will desynchronise here.
    """

    def __init__(self) -> None:
        self.out = bytearray()
        self._reserve()

    def _reserve(self) -> None:
        self.desc_pos = len(self.out)
        self.out += b"\x00\x00"
        self.bits = 0
        self.count = 0

    def _bit(self, value: int) -> None:
        if value:
            self.bits |= 1 << self.count
        self.count += 1
        if self.count == 16:
            self.out[self.desc_pos] = self.bits & 0xFF
            self.out[self.desc_pos + 1] = self.bits >> 8
            self._reserve()

    def literal(self, value: int) -> "KosinskiWriter":
        self._bit(1)
        self.out.append(value)
        return self

    def literals(self, data: bytes) -> "KosinskiWriter":
        for value in data:
            self.literal(value)
        return self

    def inline_match(self, displacement: int, length: int) -> "KosinskiWriter":
        assert -256 <= displacement <= -1 and 2 <= length <= 5
        encoded = length - 2
        self._bit(0)
        self._bit(0)
        self._bit((encoded >> 1) & 1)
        self._bit(encoded & 1)
        self.out.append(displacement & 0xFF)
        return self

    def _full(self, displacement: int, count: int) -> None:
        assert -8192 <= displacement <= -1 and 0 <= count <= 7
        word = displacement & 0xFFFF
        self._bit(0)
        self._bit(1)
        self.out.append(word & 0xFF)
        self.out.append(((word >> 5) & 0xF8) | count)

    def full_match(self, displacement: int, length: int) -> "KosinskiWriter":
        assert 3 <= length <= 9
        self._full(displacement, length - 2)
        return self

    def extended_match(self, displacement: int, length: int) -> "KosinskiWriter":
        assert 3 <= length <= 256
        self._full(displacement, 0)
        self.out.append(length - 1)
        return self

    def resume(self, displacement: int = -1) -> "KosinskiWriter":
        """A full match with count 0 and extra byte 1: decodes to nothing."""
        self._full(displacement, 0)
        self.out.append(1)
        return self

    def finish(self) -> bytes:
        self._full(-0x2000, 0)  # displacement is irrelevant to the terminator
        self.out.append(0)
        self.out[self.desc_pos] = self.bits & 0xFF
        self.out[self.desc_pos + 1] = self.bits >> 8
        return bytes(self.out)


class TestKosinskiDecoder(unittest.TestCase):
    """Decoder unit tests. These need no ROM and no disassembly."""

    def test_literal_run(self):
        stream = KosinskiWriter().literals(b"PSIV!").finish()
        self.assertEqual(decompress(stream), (b"PSIV!", len(stream)))

    def test_inline_match_overlaps(self):
        stream = KosinskiWriter().literals(b"AB").inline_match(-2, 3).finish()
        # The copy is byte-by-byte, so it reads bytes it has just written.
        self.assertEqual(decompress(stream)[0], b"ABABA")

    def test_inline_match_length_range(self):
        for length in (2, 3, 4, 5):
            with self.subTest(length=length):
                stream = KosinskiWriter().literals(b"abcd").inline_match(-4, length).finish()
                # Length 5 outruns the 4-byte window and wraps into its own output.
                self.assertEqual(decompress(stream)[0], b"abcd" + (b"abcd" * 2)[:length])

    def test_full_match(self):
        stream = KosinskiWriter().literals(b"0123456789").full_match(-8, 5).finish()
        self.assertEqual(decompress(stream)[0], b"0123456789" + b"23456")

    def test_full_match_length_range(self):
        base = bytes(range(0x40, 0x40 + 16))
        for length in range(3, 10):
            stream = KosinskiWriter().literals(base).full_match(-16, length).finish()
            self.assertEqual(decompress(stream)[0], base + base[:length])

    def test_extended_count_match(self):
        base = b"0123456789abcdef"
        stream = KosinskiWriter().literals(base).extended_match(-16, 40).finish()
        expected = base + (base * 3)[:40]
        self.assertEqual(decompress(stream)[0], expected)

    def test_extended_count_maximum(self):
        base = bytes(range(0x80, 0x80 + 32))
        stream = KosinskiWriter().literals(base).extended_match(-32, 256).finish()
        self.assertEqual(decompress(stream)[0], base + (base * 8)[:256])

    def test_extra_byte_one_resumes_instead_of_terminating(self):
        """Extra byte 0 ends the stream; 1 falls back into the main loop."""
        stream = KosinskiWriter().literals(b"AB").resume().literals(b"CD").finish()
        decoded, consumed = decompress(stream)
        self.assertEqual(decoded, b"ABCD")
        self.assertEqual(consumed, len(stream))

    def test_end_marker_stops_and_reports_length(self):
        stream = KosinskiWriter().literals(b"done").finish()
        padded = b"\xAA\xBB" + stream + b"\xDE\xAD\xBE\xEF"
        self.assertEqual(decompress(padded, 2), (b"done", len(stream)))

    def test_description_field_is_refilled_before_the_sixteenth_operand(self):
        """The reload in KosDecomp happens after the 16th bit, before its byte."""
        stream = KosinskiWriter().literals(bytes(range(1, 21))).finish()
        decoded, _ = decompress(stream)
        self.assertEqual(decoded, bytes(range(1, 21)))
        # 20 literals need two description fields; the second is embedded
        # between literal 16 and its own operand byte.
        self.assertEqual(len(stream), 2 + 16 + 2 + 4 + 3)

    def test_truncated_stream_raises(self):
        stream = KosinskiWriter().literals(b"abc").finish()
        with self.assertRaises(KosinskiError):
            decompress(stream[:-1])

    def test_match_before_start_of_output_raises(self):
        stream = KosinskiWriter().literal(0x41).inline_match(-16, 3).finish()
        with self.assertRaises(KosinskiError):
            decompress(stream)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestFormationsFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_all(cls.data)
        cls.formations = cls.result["formations"]
        cls.indexes = cls.result["formation_indexes"]
        cls.by_id = {f["id"]: f for f in cls.formations["formations"]}

    def test_compressed_ranges_are_consumed_exactly(self):
        """Every documented blob must decode and end where the map says."""
        specs = [*FORMATION_BLOCKS, BOSS_FORMATION_BLOCK, FORMATION_INDEX_BLOCK]
        expected = {
            "Battle_FormationIndexes": (0x77B, 0x1100),
            "Battle_FormationData1": (0x442, 0x6AC),
            "Battle_FormationData2": (0x457, 0x6B4),
            "Battle_FormationData3": (0x462, 0x68E),
            "Battle_FormationData4": (0x3F0, 0x604),
            "Battle_BossFormationData": (0x96, 0x12C),
        }
        for spec in specs:
            with self.subTest(spec["label"]):
                decompressed, source = decompress_block(self.data, spec)
                compressed_size, decompressed_size = expected[spec["label"]]
                self.assertEqual(source["compressed_size"], compressed_size)
                self.assertEqual(len(decompressed), decompressed_size)

    def test_block_4_runs_to_the_boss_data(self):
        """The disassembly's first inline range annotation for block 4 stops at
        0x284C3D, but that is only the first annotated chunk. The stream ends
        where Battle_BossFormationData begins."""
        block_4 = next(b for b in FORMATION_BLOCKS if b["name"] == "block_4")
        self.assertEqual(block_4["end"], BOSS_FORMATION_BLOCK["start"])
        with self.assertRaises(FormationError):
            decompress_block(self.data, {**block_4, "end": 0x284C3D})

    @unittest.skipUnless(ORACLE_DIR.is_dir(), f"disassembly oracle not present at {ORACLE_DIR}")
    def test_oracle_round_trip(self):
        """Decompressed retail bytes must equal the disassembly's plain sources."""
        specs = {s["name"]: s for s in [*FORMATION_BLOCKS, BOSS_FORMATION_BLOCK]}
        for name, filename in ORACLE_FILES.items():
            with self.subTest(name):
                oracle = read_oracle_bytes(ORACLE_DIR / filename)
                decompressed, _ = decompress_block(self.data, specs[name])
                self.assertEqual(decompressed, oracle)

    def test_block_record_counts(self):
        counts = {b["name"]: b["record_count"] for b in self.formations["blocks"]}
        self.assertEqual(counts, {"block_1": 128, "block_2": 128, "block_3": 128, "block_4": 120})
        self.assertEqual(self.formations["total_formations"], 504)
        self.assertEqual(self.formations["boss_block"]["record_count"], 27)
        self.assertEqual(self.formations["total_boss_formations"], 27)

    def test_global_ids_are_contiguous_and_block_aligned(self):
        ids = [f["id"] for f in self.formations["formations"]]
        self.assertEqual(ids, list(range(0x1F8)))
        self.assertEqual(self.by_id[0x080]["block"], "block_2")
        self.assertEqual(self.by_id[0x100]["block"], "block_3")
        self.assertEqual(self.by_id[0x180]["block"], "block_4")
        self.assertEqual(self.by_id[0x1F7]["index_in_block"], 119)

    def test_pinned_formation_zero(self):
        first = self.by_id[0x000]
        self.assertEqual(first["block"], "block_1")
        self.assertEqual(first["index_in_block"], 0)
        self.assertEqual(first["raw_hex"], "10000880020300010e011aff")
        self.assertEqual(first["surprise_agility"], 0x10)
        self.assertEqual(first["run_agility"], 0x00)
        self.assertTrue(first["can_run"])
        self.assertEqual(first["item_drop_rate"], 0x08)
        self.assertEqual(first["dropped_item"], {"id": 0x80, "symbol": "Antidote"})
        self.assertEqual(first["enemy_count"], 2)
        self.assertTrue(first["enemy_count_matches_entries"])
        self.assertEqual(first["group_1_mask"], "0x03")
        self.assertEqual(first["group_2_mask"], "0x00")
        self.assertEqual(first["group_1_slots"], [1, 2])
        self.assertEqual(first["group_2_slots"], [])
        self.assertEqual(
            [(e["enemy"]["id"], e["enemy"]["symbol"], e["position"], e["groups"]) for e in first["enemies"]],
            [(0x01, "MonsterFly", 0x0E, [1]), (0x01, "MonsterFly", 0x1A, [1])],
        )

    @unittest.skipUnless(ORACLE_DIR.is_dir(), f"disassembly oracle not present at {ORACLE_DIR}")
    def test_enemy_ids_are_zero_based(self):
        """`EnemyID_Helex = 0`, `EnemyID_MonsterFly = 1` in ps4.constants.asm.

        The formation sources annotate each enemy byte with its symbol, so the
        oracle comments prove the mapping directly.
        """
        annotated = {}
        for filename in ORACLE_FILES.values():
            for raw in (ORACLE_DIR / filename).read_text(encoding="utf-8").splitlines():
                code, _, comment = raw.partition(";")
                comment = comment.strip()
                code = code.strip()
                if not comment.startswith("EnemyID_") or not code.startswith("dc.b"):
                    continue
                value = int(code.partition("\t")[2].strip().lstrip("$"), 16)
                # Some annotations carry a trailing comma from the disassembler.
                annotated.setdefault(value, comment[len("EnemyID_"):].rstrip(","))
        self.assertGreater(len(annotated), 100)
        self.assertEqual(annotated[0x00], "Helex")
        self.assertEqual(annotated[0x01], "MonsterFly")

        resolved = {}
        for formation in [*self.formations["formations"], *self.formations["boss_formations"]]:
            for enemy in formation["enemies"]:
                resolved.setdefault(enemy["enemy"]["id"], enemy["enemy"]["symbol"])
        for enemy_id, symbol in annotated.items():
            self.assertEqual(resolved[enemy_id], symbol, f"enemy id 0x{enemy_id:02X}")

    def test_dropped_item_ids_are_one_based(self):
        """`ItemID_Dagger = 1` and `ItemID_Antidote = $80`, matching items.json."""
        self.assertEqual(self.result["items"][0]["id"], 1)
        self.assertEqual(self.result["items"][0]["symbol"], "Dagger")
        self.assertEqual(self.result["items"][0x7F]["symbol"], "Antidote")
        for formation in self.formations["formations"]:
            item = formation["dropped_item"]
            if item is not None:
                self.assertEqual(item["symbol"], self.result["items"][item["id"] - 1]["symbol"])

    def test_every_record_is_structurally_sound(self):
        for formation in [*self.formations["formations"], *self.formations["boss_formations"]]:
            with self.subTest(formation.get("id", formation.get("event_battle_index"))):
                enemies = formation["enemies"]
                self.assertTrue(1 <= len(enemies) <= 4)
                self.assertTrue(all(e["enemy"]["symbol"] is not None for e in enemies))
                self.assertEqual(len(formation["raw_hex"]), 2 * (7 + 2 * len(enemies) + 1))

    def test_known_declared_count_discrepancy(self):
        """Block 3 record $77 declares four enemies but lists three.

        The disassembly's uncompressed source carries the same bytes, so this
        is the ROM's own inconsistency and must be reported, not smoothed over.
        """
        mismatches = [
            f["id"] for f in self.formations["formations"]
            if not f["enemy_count_matches_entries"]
        ]
        self.assertEqual(mismatches, [0x177])
        odd = self.by_id[0x177]
        self.assertEqual(odd["enemy_count"], 4)
        self.assertEqual(len(odd["enemies"]), 3)

    def test_terminator_scan_agrees_with_structural_split(self):
        for spec in [*FORMATION_BLOCKS, BOSS_FORMATION_BLOCK]:
            with self.subTest(spec["label"]):
                decompressed, _ = decompress_block(self.data, spec)
                self.assertEqual(
                    len(split_formation_records(decompressed)),
                    decompressed.count(0xFF),
                )

    def test_formation_indexes_structure(self):
        self.assertEqual(self.indexes["group_count"], 68)
        self.assertEqual(self.indexes["group_size_bytes"], 0x40)
        self.assertEqual(self.indexes["entries_per_group"], 32)
        self.assertEqual(self.indexes["source"]["rom_offset"], "0x2836EC")
        self.assertEqual(self.indexes["source"]["decompressed_size"], 68 * 0x40)
        for group in self.indexes["groups"]:
            self.assertEqual(len(group["formation_ids"]), 32)
            self.assertEqual(len(group["raw_hex"]), 2 * 0x40)
        first = self.indexes["groups"][0]
        self.assertEqual(first["group"], 0)
        self.assertEqual(first["formation_ids"][:6], [0, 0, 1, 1, 2, 3])

    def test_formation_indexes_referential_integrity(self):
        referenced = {
            formation_id
            for group in self.indexes["groups"]
            for formation_id in group["formation_ids"]
        }
        self.assertTrue(referenced.issubset(self.by_id.keys()))
        self.assertEqual(max(referenced), 0x1F7)

    def test_provenance_is_carried(self):
        first = self.by_id[0x000]
        self.assertEqual(first["source"]["rom_offset"], "0x283E6C")
        self.assertEqual(first["source"]["rom_end_exclusive"], "0x2842AE")
        self.assertEqual(first["source"]["compression"], "kosinski")
        self.assertEqual(len(first["source"]["compressed_sha256"]), 64)
        self.assertEqual(first["block_offset"], "0x0000")


if __name__ == "__main__":
    unittest.main()
