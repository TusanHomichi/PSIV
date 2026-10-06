"""Measure the cartridge's cutscene-return field reload, one map at a time.

    PYTHONPATH=. python3 -m tests.reload_frames --out build/s8-reload/oracle
    PYTHONPATH=. python3 -m tests.reload_frames --maps 0x14 0x18d --out ...
    PYTHONPATH=. python3 -m tests.reload_frames --emit-rust build/.../table.json

`FieldRoutine_Cutscene` (`ps4.asm:120739-120758`) answers a zero return with
`bset #2, Map_Load_Flags` and `GameMode_LoadFieldMap` (mode 8). The frames that
mode spends - object-less map load, `Pal_FadeIn` - are CPU/decompression work,
so they differ per map. This runs the oracle host over tape 35's field prefix
(the same fixture `tools/certify.py` TAPE_35 pins): frame 7000 loads the map
(mode 8), frame 7200 puts the field in Events with `Event_Index = $8000`
(`Event_NoEvent`, an immediate zero return), and the rows the log spends in
mode 8 are the reload. Two states are measured per map:

* **A** `Saved_Sound_Index` equals the map's raw music id: the load's music
  branch (`ps4.asm:107539-107547`) is not taken. This is the table value.
* **B** `Saved_Sound_Index` differs: the branch writes the word and sets
  `$FFFFECED`, and the tail spends one extra `VInt_Prepare` on it
  (`ps4.asm:107619-107626`). `B - A` is 1 on most maps and 0 on a few, where
  the frame is absorbed by the load's own alignment, so both are tabled. The
  three organic captures pin their maps' B (`ORGANIC_MUSIC_TAKEN`).

Anchors the method reproduces: the principal's own organic end (tape 07, map
$14, 55 rows, music word cleared by the scene so it takes branch B) and tape
35's landings (map $18D 52 rows, $BF 42 rows).
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from oracle import host_binary
from tools.certify import ROM, TAPE_35

ROOT = Path(__file__).resolve().parent.parent
PACK = ROOT / "runtime-pack"
CORE = "oracle/core/genesis_plus_gx_libretro.so"
#: Frame the fixture reloads the map, and the frame it fires the cutscene.
LOAD_FRAME = 7000
CUTSCENE_FRAME = 7200
#: Tape 35's prefix ends at 7400; the closing speak and flight are not used.
TAPE_TAIL_LINES = 2
EXTRA_FRAMES = 300
SAVED_SOUND = "FFFFECEC"
#: Rows of the three organic captures that took the load's music branch (the
#: saved word differed going in), which the table pins over the fixture's own
#: B column: the principal's end on `$14` (tape 07, 55) and the two flight
#: landings (tape 35, `$18D` 52 and `$BF` 42, whose saved word goes `$9B` to
#: `$89` with `$FFFFECED` raised). The fixture's B is one frame longer on the
#: landings, the music frame being absorbed by the load's alignment there.
ORGANIC_MUSIC_TAKEN = {0x14: 55, 0x18D: 52, 0xBF: 42}
MODE_LOAD_FIELD_MAP = 8


def reload_tape(directory: Path) -> Path:
    """Tape 35 without its closing Speak and flight, plus idle frames."""
    lines = (ROOT / TAPE_35["tape"]).read_text().splitlines()
    kept = [line for line in lines if line.strip() and not line.startswith("#")]
    kept = kept[:-TAPE_TAIL_LINES]
    kept.append(f"{EXTRA_FRAMES} . after")
    path = directory / "reload.tape"
    path.write_text("\n".join(kept) + "\n")
    return path


def raw_music(map_id: int) -> int:
    manifest = json.loads((PACK / "manifest.json").read_text())
    for entry in manifest["maps"]:
        if entry["id"] == map_id:
            return int(json.loads((PACK / entry["json"]).read_text())["music"]["id"])
    raise KeyError(f"map {map_id:#x} is not in the pack")


def reload_rows(log_rows: list[dict[str, str]], since: int = CUTSCENE_FRAME) -> int:
    """Rows spent in mode 8 from frame `since` on (the reload)."""
    rows = 0
    for row in log_rows:
        if int(row["frame"]) < since:
            continue
        if int(row["game_mode"], 16) == MODE_LOAD_FIELD_MAP:
            rows += 1
        elif rows:
            return rows
    raise ValueError("the fixture never reached the reload, or never left it")


def run_one(map_id: int, saved: int, tape: Path, out: Path, bits: int = 0,
            at: tuple[int, int] | None = None, first_load: bool = False) -> int:
    """Rows of one reload, with `Saved_Sound_Index = saved` going in.

    `bits` is written to `Map_Load_Flags` just before the cutscene, to measure
    a flag's effect (bit 7 skips `Pal_FadeIn`). `at` is a cell to load the map
    at (`Saved_Char_X/Y_Pos`, read by the fixture's bit-1 load): the default
    is the tape's own position, which on some maps lands on a trigger that
    starts an event before the cutscene can fire. `first_load` measures the
    fixture's own frame-7000 load instead: the same `GameMode_LoadFieldMap`,
    for the maps whose entry event owns the field before frame 7200 (it equals
    the cutscene reload on every map both could measure, see `FIELD_RELOAD.md`).
    """
    log = out / f"m{map_id:03X}_s{saved:02X}_b{bits:02X}.csv"
    patches = [
        f"{LOAD_FRAME}:FFFFEC28:{map_id:04X}",
        f"{LOAD_FRAME}:FFFFEC2A:0000",
        f"{LOAD_FRAME}:FFFFEC4E:02",
        f"{LOAD_FRAME}:FFFFEF00:0008",
        f"{LOAD_FRAME}:FFFFF101:80",
        f"{CUTSCENE_FRAME - 1}:{SAVED_SOUND}:{saved:02X}",
        f"{CUTSCENE_FRAME}:FFFFECA8:8000",
        f"{CUTSCENE_FRAME}:FFFFEC20:000C",
    ]
    if first_load:
        patches.append(f"{LOAD_FRAME - 1}:{SAVED_SOUND}:{saved:02X}")
    if at is not None:
        patches += [f"{LOAD_FRAME}:FFFFF406:{at[0] * 16:04X}", f"{LOAD_FRAME}:FFFFF408:{at[1] * 16:04X}"]
    if bits:
        patches.append(f"{CUTSCENE_FRAME - 1}:FFFFEC4E:{bits:02X}")
    command = [
        str(host_binary.ensure()),
        "--core", CORE, "--rom", ROM, "--map", "oracle/ram_map.tsv",
        "--tape", str(tape), "--groups", "core", "--out", str(log),
    ]
    for patch in patches:
        command += ["--ram-patch", patch]
    done = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if done.returncode:
        raise RuntimeError(f"oracle exited {done.returncode}: {done.stdout.decode()[-400:]}")
    with log.open() as handle:
        rows = list(csv.DictReader(line for line in handle if not line.startswith("#")))
    log.unlink()
    return reload_rows(rows, LOAD_FRAME if first_load else CUTSCENE_FRAME)


def candidate_cells(map_id: int, count: int = 12) -> list[tuple[int, int]]:
    """Walkable cells spread over the map, to retry a load that fired an event."""
    manifest = json.loads((PACK / "manifest.json").read_text())
    entry = next(e for e in manifest["maps"] if e["id"] == map_id)
    record = json.loads((PACK / entry["json"]).read_text())
    rows = record["collision"]["grid"] if "grid" in record["collision"] else record["collision"]["rows"]
    cells = [(x, y) for y, row in enumerate(rows) for x, code in enumerate(row) if code == 0]
    step = max(1, len(cells) // count)
    return cells[::step][:count]


def measure_map(map_id: int, tape: Path, out: Path) -> dict:
    music = raw_music(map_id)
    tried = [None, *candidate_cells(map_id)]
    for at in tried:
        try:
            a = run_one(map_id, music, tape, out, at=at)
            b = run_one(map_id, (music + 1) & 0xFF, tape, out, at=at)
        except ValueError:
            continue
        row = {"map": map_id, "music": music, "rows": a, "rows_music_written": b}
        if at is not None:
            row["cell"] = list(at)
        return row
    # An entry event owns the field before the cutscene can fire, at every cell:
    # measure the fixture's own first load, the same routine.
    try:
        a = run_one(map_id, music, tape, out, first_load=True)
        b = run_one(map_id, (music + 1) & 0xFF, tape, out, first_load=True)
    except ValueError:
        raise ValueError(f"map {map_id:#x}: no load reached the reload") from None
    return {"map": map_id, "music": music, "rows": a, "rows_music_written": b,
            "via": "first_load"}


def pack_maps() -> list[int]:
    manifest = json.loads((PACK / "manifest.json").read_text())
    return sorted(entry["id"] for entry in manifest["maps"])


def emit_rust(table: list[dict]) -> str:
    """The rows of `field_reload.rs`'s `ROWS`: `(map, rows, rows_music_written)`."""
    lines = [
        f"    ({row['map']:#05x}, {row['rows']}, {row['rows_music_written']}),"
        for row in sorted(table, key=lambda r: r["map"])
    ]
    return "\n".join(lines)


def write_table(receipt: dict, path: Path) -> None:
    """Fill the two generated regions of `field_reload.rs` from a receipt."""
    text = path.read_text()
    proxies = sorted(row["map"] for row in receipt["maps"] if row.get("via"))
    receipt = {**receipt, "maps": [
        {**row, "rows_music_written": ORGANIC_MUSIC_TAKEN.get(row["map"], row["rows_music_written"])}
        for row in receipt["maps"]
    ]}
    for begin, end, body in (
        ("// BEGIN GENERATED: python3 -m tests.reload_frames --write-table <this file>",
         "// END GENERATED\n", emit_rust(receipt["maps"])),
        ("// BEGIN GENERATED PROXY\n", "// END GENERATED PROXY",
         "\n".join(f"    {m:#05x}," for m in proxies)),
    ):
        head, rest = text.split(begin, 1)
        first_newline = rest.index("\n") + 1 if not begin.endswith("\n") else 0
        marker = rest[:first_newline]
        tail = rest[rest.index(end):]
        text = f"{head}{begin}{marker}{body}\n    {tail}"
    path.write_text(text)


def merge(previous: dict, results: list[dict], failures: list[dict]) -> dict:
    """A rerun of some maps replaces their rows and failures in a receipt."""
    ran = {row["map"] for row in results} | {row["map"] for row in failures}
    return {
        "maps": [r for r in previous.get("maps", []) if r["map"] not in ran] + results,
        "failures": [r for r in previous.get("failures", []) if r["map"] not in ran] + failures,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--maps", nargs="*", help="map ids (default: every pack map)")
    parser.add_argument("--out", type=Path, default=ROOT / "build/s8-reload/oracle")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--retry-failures", action="store_true",
                        help="rerun the maps `--out`'s table.json recorded as failures and merge")
    parser.add_argument("--emit-rust", type=Path, help="print the table rows of a result json")
    parser.add_argument("--write-table", type=Path,
                        help="rewrite the generated regions of field_reload.rs from --out's table.json")
    args = parser.parse_args(argv)
    if args.write_table:
        write_table(json.loads((args.out / "table.json").read_text()), args.write_table)
        return 0
    if args.emit_rust:
        print(emit_rust(json.loads(args.emit_rust.read_text())["maps"]))
        return 0
    table_path = args.out / "table.json"
    previous = json.loads(table_path.read_text()) if table_path.is_file() else {}
    if args.retry_failures:
        maps = [row["map"] for row in previous.get("failures", [])]
    elif args.maps:
        maps = [int(m, 0) for m in args.maps]
    else:
        maps = pack_maps()
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as scratch:
        tape = reload_tape(Path(scratch))
        results, failures = [], []

        def job(map_id):
            try:
                return measure_map(map_id, tape, args.out)
            except (RuntimeError, ValueError, KeyError) as error:
                return {"map": map_id, "error": str(error)}

        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for row in pool.map(job, maps):
                (failures if "error" in row else results).append(row)
                print(row, flush=True)
    partial = args.retry_failures or bool(args.maps)
    merged = merge(previous, results, failures) if partial else {
        "maps": results,
        "failures": failures,
    }
    receipt = {
        "host_build_id": host_binary.reported_build_id(host_binary.ensure()),
        "tape": TAPE_35["tape"],
        **merged,
    }
    table_path.write_text(json.dumps(receipt, indent=2) + "\n")
    return 1 if merged["failures"] else 0


if __name__ == "__main__":
    sys.exit(main())
