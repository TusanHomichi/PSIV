"""The per-build decode table (#130): one decode per blob, keyed by content."""

import unittest
from pathlib import Path
from unittest import mock

from psiv_tools import decode_cache, nemesis
from psiv_tools.battle_art import build_tile_bank, enemy_art_bounds, enemy_records
from psiv_tools.core import read_rom
from psiv_tools.decode_cache import memoized, scope
from psiv_tools.enigma import decompress as enigma_decompress

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

calls: list[tuple[bytes, int, int]] = []


@memoized()
def _echo(data: bytes, offset: int = 0, length: int = 2) -> tuple[bytes, int]:
    calls.append((data, offset, length))
    return data[offset:offset + length], length


@memoized(copy=list)
def _words(data: bytes) -> list[int]:
    calls.append((data, 0, 0))
    return list(data)


class TestTheTable(unittest.TestCase):
    def setUp(self):
        calls.clear()

    def test_outside_a_scope_every_call_decodes(self):
        self.assertIsNone(decode_cache.active())
        _echo(b"abcd", 1)
        _echo(b"abcd", 1)
        self.assertEqual(len(calls), 2)

    def test_inside_a_scope_a_blob_decodes_once(self):
        with scope() as table:
            first = _echo(b"abcd", 1)
            # Positional, keyword and defaulted spellings are one key.
            self.assertEqual(_echo(b"abcd", offset=1), first)
            self.assertEqual(_echo(b"abcd", 1, 2), first)
        self.assertEqual(len(calls), 1)
        self.assertEqual(table.hits, 2)
        self.assertEqual(list(table.decodes.values()), [1])

    def test_the_key_is_content_not_identity(self):
        rom = bytes(range(16))
        same = bytes(bytearray(rom))
        changed = bytearray(rom)
        changed[3] ^= 0xFF
        changed = bytes(changed)
        self.assertIsNot(rom, same)
        with scope():
            self.assertEqual(_echo(rom, 2)[0], b"\x02\x03")
            self.assertEqual(_echo(same, 2)[0], b"\x02\x03")
            # Same offset, same length, different bytes: never shared.
            self.assertEqual(_echo(changed, 2)[0], bytes([2, 3 ^ 0xFF]))
        self.assertEqual(len(calls), 2)

    def test_mutable_input_is_never_a_key(self):
        data = bytearray(b"abcd")
        with scope():
            self.assertEqual(_echo(data, 0)[0], b"ab")
            data[0] = ord("z")
            self.assertEqual(_echo(data, 0)[0], b"zb")
        self.assertEqual(len(calls), 2)

    def test_a_mutable_result_is_handed_out_as_a_copy(self):
        with scope():
            first = _words(b"\x01\x02")
            first.append(99)
            self.assertEqual(_words(b"\x01\x02"), [1, 2])
        self.assertEqual(len(calls), 1)

    def test_a_nested_scope_joins_and_the_table_ends_with_the_outer_one(self):
        with scope() as outer:
            _echo(b"abcd")
            with scope() as inner:
                self.assertIs(inner, outer)
                _echo(b"abcd")
        self.assertEqual(len(calls), 1)
        self.assertIsNone(decode_cache.active())
        with scope():
            _echo(b"abcd")
        self.assertEqual(len(calls), 2, "a new scope starts empty")

    def test_a_failing_decode_is_not_kept(self):
        @memoized()
        def boom(data: bytes) -> bytes:
            calls.append((data, 0, 0))
            raise ValueError("bad stream")

        with scope() as table:
            for _ in range(2):
                with self.assertRaises(ValueError):
                    boom(b"x")
        self.assertEqual(len(calls), 2)
        self.assertEqual(table.results, {})

    def test_every_call_is_an_attempt_refused_or_not(self):
        @memoized()
        def picky(data: bytes, offset: int = 0) -> bytes:
            if offset:
                raise ValueError("bad stream")
            return data

        with scope() as table:
            picky(b"x")
            picky(b"x", offset=0)
            for _ in range(2):
                try:
                    picky(b"x", 1)
                except ValueError:
                    pass  # a caller that probes, as battle_art._is_mapping does
        name = f"{picky.__module__}.{picky.__qualname__}"
        self.assertEqual(list(table.attempts), [(name, b"x", 0), (name, b"x", 1)])
        self.assertEqual(list(table.decodes), [(name, b"x", 0)])

    def test_mutable_input_is_counted_unkeyed(self):
        with scope() as table:
            _echo(bytearray(b"abcd"))
            _echo(memoryview(b"abcd"))
        self.assertEqual(table.unkeyed, 2)
        self.assertEqual(table.attempts, {})


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheRealDecoders(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = read_rom(ROM)
        cls.records = enemy_records(cls.rom)
        cls.bounds = enemy_art_bounds(cls.records, len(cls.rom))

    def _banks(self):
        return [build_tile_bank(self.rom, record, self.bounds) for record in self.records]

    def test_every_enemy_bank_decodes_each_nemesis_blob_once(self):
        # Counted where the decoder really runs: every Nemesis decode builds
        # its code table exactly once, at offset + 2.
        real = nemesis.build_code_table
        tables: dict[int, int] = {}

        def counting(data, offset):
            tables[offset - 2] = tables.get(offset - 2, 0) + 1
            return real(data, offset)

        with mock.patch.object(nemesis, "build_code_table", counting):
            uncached = self._banks()
            requests = sum(tables.values())
            tables.clear()
            with scope() as table:
                cached = self._banks()
        self.assertEqual(cached, uncached)
        distinct = {o for r in self.records for o in r["art_offsets"]}
        self.assertEqual(set(tables), distinct)
        self.assertEqual(set(tables.values()), {1})
        # Negative control: without the scope the same banks decode shared
        # blobs again, so the count the scope removed is real.
        self.assertGreater(requests, len(distinct))
        self.assertEqual(table.hits, requests - len(distinct))

    def test_two_roms_never_share_an_entry(self):
        offset = self.records[0]["art_offsets"][0]
        original, _ = nemesis.decompress(self.rom, offset)
        mutated = bytearray(self.rom)
        # A byte inside the stream's compressed data, past the code table.
        _, data_start = nemesis.build_code_table(self.rom, offset + 2)
        mutated[data_start + 4] ^= 0xFF
        mutated = bytes(mutated)
        expected = nemesis.decompress(mutated, offset)
        self.assertNotEqual(expected[0], original, "the mutation must change the decode")
        with scope():
            self.assertEqual(nemesis.decompress(self.rom, offset)[0], original)
            self.assertEqual(nemesis.decompress(mutated, offset), expected)
            self.assertEqual(nemesis.decompress(self.rom, offset)[0], original)
        # And a later scope on the mutated ROM sees only the mutated ROM.
        with scope() as table:
            self.assertEqual(nemesis.decompress(mutated, offset), expected)
            self.assertEqual(table.hits, 0)

    def test_enigma_words_are_copied_per_caller(self):
        record = self.records[0]
        with scope():
            try:
                words, _ = enigma_decompress(self.rom, record["mapping_offset"], max_words=4096)
            except Exception as exc:  # a raw mapping enemy; pick another
                self.skipTest(f"enemy 0 mapping is not Enigma: {exc}")
            words.append(-1)
            again, _ = enigma_decompress(self.rom, record["mapping_offset"], max_words=4096)
        self.assertNotIn(-1, again)


if __name__ == "__main__":
    unittest.main()
