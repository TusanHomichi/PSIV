"""Reading the oracle's two CSVs, at the widths `oracle/ram_map.json` gives.

`--trace` is the CSV `psiv_oracle --rng-trace` writes: one row per call of
`UpdateRNGSeed2` (`ps4.asm:86097`), with the HV word the read returned, the
frame counter and the seed longword around each call. `--log` is the RAM log
from the same run (`--groups core,battle,bhit,enemy,chars,rng,vehicle`). Both
open with `#` provenance lines, which are kept (they are part of the fixture's
provenance) and skipped when the rows are read.
"""
import csv
import hashlib
import json
import os
import re

from .errors import FixtureError


def load_rows(path):
    """A CSV from the oracle as row dicts, minus its provenance comments."""
    with open(path) as handle:
        return list(csv.DictReader(
            line for line in handle if not line.startswith("#")))


def header_lines(path):
    """The `#` provenance lines an oracle CSV opens with, verbatim."""
    with open(path) as handle:
        return [line.rstrip("\n") for line in handle
                if line.startswith("#")]


#: The header fields that name a file the capture was *given*, with the tail
#: the host writes after each value. A capture's own bytes are the same
#: wherever it was taken, so `provenance_lines` cuts each of these values to
#: the file's name: a fixture is then reproducible from another directory, and
#: the two captures' logs are byte-identical. The tail is matched by the exact
#: shape `oracle/host/psiv_oracle.c` writes - a value that does not match is
#: kept whole, because then the line is not one this extractor knows.
PATH_FIELDS = {
    "rom": re.compile(r"^(?P<path>.*?)(?P<tail>\s+size=\d+)?$"),
    "tape": re.compile(
        r"^(?P<path>.*?)(?P<tail>\s+steps=\d+ frames=\d+)?$"),
    "rng-trace": re.compile(r"^(?P<path>.*?)(?P<tail>)?$"),
}


def provenance_lines(path):
    """A log's `#` lines, with every input named by its file and not its path.

    `header_lines` reads the log as the host wrote it; this is what a fixture
    stores. `# rom=/somewhere/Phantasy Star IV (USA).md size=3145728` becomes
    `# rom=Phantasy Star IV (USA).md size=3145728`, and likewise for the tape
    and the roll trace, so a fixture does not depend on the directory the
    capture was swept into. A value that already carries no directory - every
    log the host writes today - reads back exactly as it stands.
    """
    lines = []
    for line in header_lines(path):
        field, separator, rest = line.lstrip("# ").partition("=")
        pattern = PATH_FIELDS.get(field)
        if not separator or pattern is None:
            lines.append(line)
            continue
        match = pattern.match(rest)
        named = os.path.basename(match.group("path"))
        lines.append(f"# {field}={named}{match.group('tail') or ''}")
    return lines


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def load_ram_map(path):
    """{name: field} from `oracle/ram_map.json`."""
    with open(path) as handle:
        fields = json.load(handle)["fields"]
    return {field["name"]: field for field in fields}


class Log:
    """The RAM log, with each column read at the width the map gives it."""

    def __init__(self, rows, ram_map):
        self.rows = rows
        self.map = ram_map
        self.by_frame = {}
        for row in rows:
            self.by_frame[int(row["frame"])] = row

    def has(self, name):
        """Whether the log carries the column at all (its group was on)."""
        if name not in self.map:
            return False
        return any(name in row for row in self.rows)

    def raw(self, frame, name):
        """The column's text, as the host wrote it."""
        row = self.by_frame.get(frame)
        if row is None:
            raise FixtureError(f"the log has no frame {frame}")
        if name not in row:
            raise FixtureError(f"the log has no column {name!r}; rerun the "
                               f"oracle with the group that carries it")
        return row[name]

    def num(self, frame, name):
        """The column as an integer, honouring its hex/dec rendering."""
        text = self.raw(frame, name)
        return int(text, 16) if self.map[name].get("hex") else int(text)

    def signed(self, frame, name):
        """A column the cartridge stores as a signed word (HP goes below 0)."""
        value = self.num(frame, name)
        return value - 0x10000 if value > 0x7FFF else value

    def changed(self, frame, name):
        """Whether the column differs from the *previous frame in the log*.

        A missing previous frame is not a change: the caller is looking for the
        frame something moved, and with a hole in the log there is nothing to
        compare against. `require_complete` is what keeps that from hiding a
        real move.
        """
        previous = self.by_frame.get(frame - 1)
        if previous is None:
            return False
        return previous.get(name) != self.by_frame[frame].get(name)

    def require_complete(self, first, last):
        """Insist every frame between `first` and `last` has a row."""
        missing = [frame for frame in range(first, last + 1)
                   if frame not in self.by_frame]
        if missing:
            raise FixtureError(
                f"the log is missing {len(missing)} frame(s) between {first} "
                f"and {last} (first: {missing[0]}); a fixture needs a row for "
                f"every frame, because a change is read against the frame "
                f"before it")
