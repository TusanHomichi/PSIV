#!/usr/bin/env python3
"""Fail a change that carries a copy of the cartridge's tables or tiles (issue #149).

    python3 tools/check_rom_tables.py [--pack DIR]

The ROM-derived tables live in the local runtime pack and never in the tree
(README, "License and third-party code"). Names and single numbers in documents
and tests are allowed; a table is not, and neither is art. Three checks read every text file of the
change (`tools/repo_files.py`), whatever its suffix:

- **Removed copies.** A file whose content is a version of a table copy this
  repository once tracked fails, by its Git blob id (`REMOVED_BLOBS`). This
  needs no local input, so a revert of #149's removal fails on any clone.
- **Enemy records.** With the runtime pack present (`runtime-pack/`, or
  `PSIV_RUNTIME_PACK`, or `--pack`), an enemy's name - its symbol or display
  name, case and separators ignored - followed within `RECORD_WINDOW`
  characters by its HP and at least `RECORD_STATS` of its seven battle stats is
  one copied record. A file holding `TABLE_RECORDS` or more distinct records is
  a table and fails. The hand-transcribed records some tests seat stay well
  below that (six in the busiest file when this was written); the removed
  replay pack held 112. Without a pack this check says it was skipped.
- **Pixel-string tiles.** Eight consecutive string literals of exactly eight
  characters each (no space, at most sixteen distinct characters), separated
  only by commas, brackets and whitespace, are an 8x8 tile written as pixels,
  which is how cartridge art was once transcribed into source. Art loads from
  the pack; a test that wants a picture of a decoded tile compares its digest.
  Every eight contiguous rows are one tile. A synthetic picture - an ASCII test
  map - is listed here by the digest of its rows (`SYNTHETIC_TILES`), so it is
  read in review once, and any other tile beside it still fails. No comment or
  other text in a file can exempt a tile.
  This needs no local input.

Only paths and counts are printed; the pack's names and values never leave the
process. `check_docs.py` runs this after its other checks, prints its summary
line and folds the problems into its exit code.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
import sys
from pathlib import Path

try:  # imported as `tools.check_rom_tables` by tests and check_docs
    from tools.repo_files import repo_files
except ModuleNotFoundError:  # direct `python3 tools/check_rom_tables.py`
    from repo_files import repo_files

PACK = Path("runtime-pack")
ENEMIES = Path("battle") / "enemies.json"

#: Every version of the copies #149 removed: the replay fixtures' pack (enemy,
#: skill, formation, equipment and technique records) and the Motavia sweep's
#: formation list (formation ids by encounter group).
REMOVED_BLOBS = frozenset({
    "0dd5e7f1cfab372a18225323524562dee0921c50",
    "1161d5f29c0ecccd6d1c7c77bef6300084b60819",
    "2496a733d877dae73f7f7b3478d0177120beedc4",
    "314cdeb569e728557f7c21c6e10d992574f89021",
    "6b4354ed843a169b97cdc4fc6aaa17cab9922fbf",
    "96d1410095cf4cf36d507cb3bd4c9547e475c6cd",
    "9e5ca5e5ce2c8f7986a9e066b25638208669e24b",
    "bb5ec5bb036377d19e81a28d31d2faca0b661ac7",
    "bd3f90a6437385b4432c2986f20b6aa8f06dfde4",
    "d51fc428f1119b535ff9a17dbb7b5df2dd8d95f3",
    "daa1f27a905e2137824dd32017cd1b0b455063ea",
})

#: How far after a name its values are looked for.
RECORD_WINDOW = 300
#: The battle stats an enemy record is matched on, besides its HP.
STATS = ("strength", "mental", "agility", "dexterity", "attack", "defense", "magic_defense")
#: How many of `STATS` must follow the name with the HP for one record.
RECORD_STATS = 4
#: Distinct records that make a file a table.
TABLE_RECORDS = 16
#: A name is matched across at most this many words (`DESRT LEACH`).
NAME_WORDS = 3

#: A tile's rows: string literals of exactly eight characters.
TILE_LITERAL = re.compile(r""""([^"\\\n]{8})"|'([^'\\\n]{8})'""")
#: A row that reads as a numbered name: letters, then its number.
NUMBERED_NAME = re.compile(r"([A-Za-z]+)\d+")
#: A row that reads as a hexadecimal number: an offset or an address.
HEX_NUMBER = re.compile(r"0x[0-9A-Fa-f]{6}")
#: What may sit between two rows of one tile.
TILE_SEPARATOR = re.compile(r"[\s,\[\](){}]*")
#: The synthetic pictures the tree may hold, by `tile_digest`: test maps of walls
#: and floor drawn by hand, with no cartridge content. Add one only after reading it.
SYNTHETIC_TILES = frozenset({
    "803c0b3ee2762eb85c668d8afcc88721340cb8b424bfa81f5854ca3ab47bedf0",  # rust/psiv-core/tests/determinism.rs
    "8f1b5495421b4c544ca84a45b0a41cf17e44c1a5aa9241c784bfe8478dbe79b5",  # rust/psiv-core/tests/party.rs
    "bd495137a8e5e6f5e6e59baa7f8f63c1e2dda165bd29ec4e6c3a197dbb1086f6",  # rust/psiv-core/tests/party.rs
    "48b4104d901183f4a154e9ffcd97b9f569c648315ccc090f5687d0e5d4e20fb5",  # rust/psiv-core/tests/wrapping.rs
})

WORD = re.compile(r"[A-Za-z][A-Za-z0-9]*")
NUMBER = re.compile(r"\d+")


def normal(name: str) -> str:
    """A name with case and separators dropped: `Desrt Leach` is `DESRTLEACH`."""
    return re.sub(r"[^A-Z0-9]", "", name.upper())


class Records:
    """The pack's enemies, keyed by normalized name."""

    def __init__(self, enemies: list[dict]):
        self.by_name: dict[str, set[int]] = collections.defaultdict(set)
        self.values: dict[int, tuple[int, list[int]]] = {}
        for enemy in enemies:
            for name in (enemy.get("symbol"), enemy.get("display_name")):
                key = normal(name or "")
                # A short name ("ZIO") is a word of ordinary prose too often.
                if len(key) >= 4:
                    self.by_name[key].add(enemy["id"])
            self.values[enemy["id"]] = (enemy["hp"], [enemy["stats"][stat] for stat in STATS])
        self.prefixes = {key[:end] for key in self.by_name for end in range(1, len(key))}

    @classmethod
    def load(cls, pack: Path) -> Records:
        return cls(json.loads((pack / ENEMIES).read_text())["enemies"])

    def matches(self, enemy: int, after: str) -> bool:
        """Whether `after`, the text following the name, holds the record."""
        hp, stats = self.values[enemy]
        numbers = collections.Counter(int(n) for n in NUMBER.findall(after))
        if not numbers[hp]:
            return False
        numbers[hp] -= 1
        found = 0
        for value in stats:
            if numbers[value]:
                numbers[value] -= 1
                found += 1
        return found >= RECORD_STATS

    def in_text(self, text: str) -> set[int]:
        """The distinct enemies whose record `text` carries."""
        words = [(match.group().upper(), match.end()) for match in WORD.finditer(text)]
        found: set[int] = set()
        for first in range(len(words)):
            key, last = words[first][0], first
            while True:
                for enemy in self.by_name.get(key, ()):
                    end = words[last][1]
                    if enemy not in found and self.matches(enemy, text[end:end + RECORD_WINDOW]):
                        found.add(enemy)
                if (key not in self.prefixes or last - first + 1 >= NAME_WORDS
                        or last + 1 >= len(words)):
                    break
                last += 1
                key += words[last][0]
        return found


def numbered_names(rows: list[str]) -> bool:
    """Whether `rows` count through one name (`Name1`, `Name2`, ...) or are all
    hexadecimal numbers: a list the width of a tile's rows, not a picture."""
    if all(HEX_NUMBER.fullmatch(row) for row in rows):
        return True
    names = [NUMBERED_NAME.fullmatch(row) for row in rows]
    return (
        all(names)
        and len({name.group(1) for name in names}) == 1
        and len(set(rows)) == len(rows)
    )


def tile_digest(rows: list[str]) -> str:
    """What `SYNTHETIC_TILES` pins: exactly these eight rows, in order."""
    return hashlib.sha256("\n".join(rows).encode()).hexdigest()


def tiles_in(text: str, allowed: frozenset[str] = SYNTHETIC_TILES) -> list[int]:
    """The first line of every pixel-string tile `text` holds, those `allowed`
    excluded. Every eight contiguous rows are a tile of their own: an exempt
    one says nothing about the next."""
    found = []
    start, previous = 0, None
    rows: list[str] = []
    for match in TILE_LITERAL.finditer(text):
        row = match.group(1) or match.group(2)
        if " " in row or len(set(row)) > 16:
            rows, previous = [], None
            continue
        if previous is None or not TILE_SEPARATOR.fullmatch(text, previous, match.start()):
            rows = []
        if not rows:
            start = match.start()
        rows.append(row)
        previous = match.end()
        if len(rows) < 8:
            continue
        if not numbered_names(rows) and tile_digest(rows) not in allowed:
            found.append(text.count("\n", 0, start) + 1)
        rows = []
    return found


def blob_id(data: bytes) -> str:
    """Git's id for a file with this content."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def pack_dir(root: Path) -> Path:
    override = os.environ.get("PSIV_RUNTIME_PACK")
    return Path(override) if override else root / PACK


def run(root: Path = Path("."), pack: Path | None = None,
        files: list[str] | None = None) -> tuple[list[str], str]:
    """`(problem lines, summary)` for the change at `root`."""
    pack = pack or pack_dir(root)
    records = Records.load(pack) if (pack / ENEMIES).is_file() else None
    problems = []
    scanned = 0
    for name in (files if files is not None else repo_files(root)):
        path = root / name
        if not path.is_file():
            continue
        data = path.read_bytes()
        if blob_id(data) in REMOVED_BLOBS:
            problems.append(f"{name}:1: a copy of the battle tables #149 removed from the tree")
            continue
        if b"\0" in data:
            continue
        text = data.decode("utf-8", errors="replace")
        for line in tiles_in(text):
            problems.append(f"{name}:{line}: an 8x8 tile written as pixel strings (load art "
                            f"from the pack, or compare a digest; a synthetic picture goes in "
                            f"SYNTHETIC_TILES; #149)")
        if records is None:
            continue
        scanned += 1
        count = len(records.in_text(text))
        if count >= TABLE_RECORDS:
            problems.append(f"{name}:1: {count} enemy records of the runtime pack's battle "
                            f"tables (a table copied from the cartridge; keep it in the "
                            f"local pack, #149)")
    if records is None:
        summary = (f"rom tables: removed copies and tiles checked; records skipped "
                   f"(no {pack / ENEMIES})")
    else:
        summary = f"rom tables: {scanned} file(s) scanned, {len(problems)} problem(s)"
    return problems, summary


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--pack", type=Path, help="the runtime pack (default: runtime-pack/)")
    arguments = parsed.parse_args(argv)
    problems, summary = run(Path("."), arguments.pack)
    for problem in problems:
        print(problem)
    print(summary)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
