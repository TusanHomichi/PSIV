#!/usr/bin/env python3
"""tools/pack_parity.py - hold `psiv-pack` to the Python pack builder (#216).

Until the switch, the Python builder (`python3 -m psiv_tools pack`) is the
pack the game, the lanes and the gate use, and `rust/psiv-pack` ports it a
section at a time. This step, part of the full gate, builds `psiv-pack` in
release and checks it against the Python builder from the same ROM:

1. The wrong-ROM refusal: both builders run on a file that is not the US
   retail ROM. Both must exit non-zero, leave the output alone, and print the
   same message after their program prefix (`psiv-tools: error: ` and
   `psiv-pack: error: `), which the setup screen shows to the player.
2. Every path `psiv-pack --claims` names (its sections, each with the paths
   it writes): both builders build a full pack from
   the ROM, and each file under a claimed path must be in both packs, with
   byte-identical bytes, or for a PNG the same `image_sha256` (its decoded
   content: zlib implementations deflate alike rows differently,
   `psiv_tools/png.py`). A claim the Python pack lacks fails, and so does a
   file `psiv-pack` writes outside its claims. A claim names a file, a
   directory, or with a `*` the matching files directly in a directory. With no claims, the Python
   build is skipped, and the output is still checked: it must be an
   installed directory holding no link and nothing outside the claims.
3. The probes: ported extractors that no claimed section writes yet
   (`examples/probe.rs`, `PROBES` here) print their records as one JSON
   document, which must match the Python extractors' byte for byte.
4. The decoders: every Nemesis, Kosinski and Enigma call a full Python build
   attempts, refused ones included, is listed, by decoder, offset and arguments, in the census
   (`rust/psiv-pack/tests/fixtures/decode_census.json`, written by
   `--write-census`). Both decode every listed stream from the ROM; each
   output (by SHA-256) and consumed length must agree, and both must refuse
   the same calls. psiv-pack's decode time is printed beside Python's.
5. `psiv-pack`'s run: one `progress <done>/<total> <section>` line per
   section, then `installed <pack>`, exit 0. Its wall time is printed with
   the CPU clock and load, the number each port PR records.

`--write-fixtures` instead rewrites `rust/psiv-pack/tests/fixtures/harness.json`,
the cases Python's `json.dumps(..., indent=2, sort_keys=True)` and PNG encoder
produce that the crate's own tests hold its writers to;
`tests/test_pack_parity.py` fails when the committed file is stale.

The cargo build takes the heavy lock's `build` class and each pack build its
`pack` class; under the gate's exclusive hold both are no-ops. Exit 0 on
parity, 1 on any difference or missing input, 2 on bad usage.
"""

from __future__ import annotations

import argparse
import base64
import fnmatch
import functools
import hashlib
import json
import os
import random
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools"))

from heavy_lock import BUILD, PACK, hold_heavy_lock  # noqa: E402
from psiv_tools import decode_cache  # noqa: E402
from psiv_tools.png import encode_indexed, encode_rgb, image_sha256  # noqa: E402
from tree_rss import cpu_mhz, load_1m  # noqa: E402

DEFAULT_ROM = ROOT / "Phantasy Star IV (USA).md"
FIXTURE = ROOT / "rust" / "psiv-pack" / "tests" / "fixtures" / "harness.json"
BINARY = ROOT / "rust" / "target" / "release" / ("psiv-pack.exe" if os.name == "nt" else "psiv-pack")
PROBE_BINARY = BINARY.parent / "examples" / BINARY.name.replace("psiv-pack", "probe")
CENSUS = ROOT / "rust" / "psiv-pack" / "tests" / "fixtures" / "decode_census.json"
CENSUS_BINARY = BINARY.parent / "examples" / BINARY.name.replace("psiv-pack", "decode_census")
CARGO_BUILD = ["cargo", "build", "--release", "--manifest-path", "rust/Cargo.toml", "-p", "psiv-pack",
               "--bins", "--example", "probe", "--example", "decode_census"]
#: The memoized decoders a census records, by `decode_cache` name. Any other
#: memoized function in a build (a section's own cache) is not a decompressor.
DECODERS = {"psiv_tools.nemesis.decompress": "nemesis",
            "psiv_tools.kosinski.decompress": "kosinski",
            "psiv_tools.enigma.decompress": "enigma"}


def _probe_names(rom: bytes):
    from psiv_tools.text import extract_names
    return extract_names(rom)


def _probe_cram(rom: bytes):
    from psiv_tools.gfx import decode_color
    return [decode_color(word) for word in [*range(0x1000), 0xF000]]


@functools.lru_cache(maxsize=1)
def _probe_maps(rom: bytes):
    from psiv_tools.maps import extract_maps
    return extract_maps(rom)


def _probe_map_pictures(rom, record, decoded, overworld, palette, image, overlay, effects, updates):
    """`build_pack`'s picture work for one map, as the JSON it hands the map
    file. The PNGs themselves are claimed; `resolve_palette_effects` needs the
    sprites, so it joins this with them."""
    from psiv_tools.map_patches import (atlas_json, index_overworld_patches, index_writes,
                                        patch_atlas, resolve_map_effects,
                                        resolve_overworld_patches, scene_patch_chunks)
    from psiv_tools.map_updates_render import bind_updates
    from psiv_tools.pack import layout_variants
    stem = f"{record['id']:03X}_{record['symbol']}"
    with tempfile.TemporaryDirectory(prefix="psiv-probe-") as scratch:
        directory = Path(scratch)
        (directory / "maps").mkdir()
        replacements = [r for r in effects["replacements"] if r["map"] == record["id"]]
        variants = layout_variants(rom, record, decoded, replacements, directory, stem)
        map_effects, written, counts = resolve_map_effects(
            decoded, effects["per_map"].get(record["id"], []))
        patches, pairs = resolve_overworld_patches(decoded, overworld)
        scene = scene_patch_chunks(rom, record)
        chunks = sorted(set(written) | set(scene))
        patch_tiles = None
        if chunks or pairs:
            base, over, entries = patch_atlas(decoded, chunks, palette, pairs)
            index_writes(map_effects, entries)
            index_overworld_patches(patches, entries)
            base_name, over_name = f"maps/{stem}_patch.png", f"maps/{stem}_patch_over.png"
            (directory / base_name).write_bytes(base)
            if over is not None:
                (directory / over_name).write_bytes(over)
            patch_tiles = atlas_json(entries, base_name, base,
                                     over_name if over is not None else None, over)
        payload = {"png": f"maps/{stem}.png", "png_over": f"maps/{stem}_over.png",
                   "layout_variants": variants, "patch_tiles": patch_tiles}
        bind_updates(rom, record, payload, directory, image, overlay, updates)
    return {"map_effects": map_effects, "patch_counts": counts, "overworld_patches": patches,
            "scene_chunks": scene, "variants": variants, "patch_tiles": patch_tiles,
            "update_images": payload.get("map_update_images")}


def _probe_layouts(rom: bytes):
    """Every real map, as the pack decodes and renders it; a render is held by
    its decoded image. For a world map, also its format facts, its patches, and
    for each cell a write touches the patches that open it."""
    from psiv_tools.layouts import chunk_palette, render_layout
    from psiv_tools.map_effects import extract_map_effects
    from psiv_tools.map_updates import extract_map_updates
    from psiv_tools.overworld import check_code_sites, opening_patches
    from psiv_tools.pack_layouts import decode_map_section, unloaded_patterns
    from psiv_tools.render import priority_overlay
    maps = _probe_maps(rom)["maps"]
    effects, updates = extract_map_effects(rom, maps), extract_map_updates(rom, maps)
    out = []
    for record in maps:
        if record.get("is_null"):
            continue
        decoded, anomalies, overworld = decode_map_section(rom, record)
        palette = chunk_palette(rom, decoded.spec.palette)
        image = render_layout(decoded.chunks, decoded.bg, decoded.patterns, palette,
                              overlay=decoded.fg)
        overlay, priority = priority_overlay(decoded, palette)
        entry = {"id": record["id"], "layout": decoded.to_json(), "anomalies": anomalies,
                 "unloaded_patterns": unloaded_patterns(decoded),
                 "png_sha256": image_sha256(image),
                 "png_over_sha256": image_sha256(overlay) if overlay is not None else None,
                 "priority": priority,
                 "pictures": _probe_map_pictures(rom, record, decoded, overworld, palette, image,
                                                 overlay, effects, updates)}
        if overworld is not None:
            entry["overworld"] = overworld.to_json()
            entry["patches"] = [patch.to_json() for patch in overworld.patches]
            entry["opening"] = [
                [f"0x{patch.routine:06X}" for patch in opening_patches(overworld, [
                    (write.chunk_x * 2 + (i & 1), write.chunk_y * 2 + (i >> 1))])]
                for patch in overworld.patches for write in patch.writes for i in range(4)]
            entry["code_sites"] = check_code_sites(rom)
        out.append(entry)
    return out


def _probe_map_effects(rom: bytes):
    """`extract_map_effects`'s document, its per-map table as `[id, entries]`
    rows: JSON object keys would sort as text on the Rust side."""
    from psiv_tools.map_effects import extract_map_effects
    effects = dict(extract_map_effects(rom, _probe_maps(rom)["maps"]))
    effects["per_map"] = [[map_id, effects["per_map"][map_id]] for map_id in sorted(effects["per_map"])]
    return effects


def _probe_map_updates(rom: bytes):
    """`extract_map_updates`'s document, its per-map table as `[id, entries]`
    rows."""
    from psiv_tools.map_updates import extract_map_updates
    updates = dict(extract_map_updates(rom, _probe_maps(rom)["maps"]))
    updates["per_map"] = [[map_id, updates["per_map"][map_id]] for map_id in sorted(updates["per_map"])]
    return updates


def _probe_map_sprites(rom: bytes):
    """`build_pack`'s sprite work per map, as the map file and manifest will
    carry it. The sheets themselves are claimed."""
    import copy
    from psiv_tools.chest_sprites import bind_chest_sprites
    from psiv_tools.gfx import palette_rgb
    from psiv_tools.layouts import decode_map_palette
    from psiv_tools.map_effects import extract_map_effects
    from psiv_tools.map_patches import resolve_palette_effects
    from psiv_tools.pack_layouts import decode_map_section
    from psiv_tools.pack_map_json import _treasure_chests
    from psiv_tools.sprites import SpriteCensus, facing_table_extents, scan_field_objects
    from psiv_tools.sprites.emit import NPC_SPRITES_DIRECTORY, SheetRegistry, resolve_map_sprites
    maps = _probe_maps(rom)["maps"]
    effects = extract_map_effects(rom, maps)
    routines = scan_field_objects(rom)
    extents = facing_table_extents(routines)
    registry, census = SheetRegistry(NPC_SPRITES_DIRECTORY), SpriteCensus()
    per_map = []
    for record in maps:
        if record.get("is_null"):
            continue
        decoded, _, _ = decode_map_section(rom, record)
        sprites, artless = resolve_map_sprites(rom, record, decoded, routines, extents, registry, census)
        palette = palette_rgb(decode_map_palette(rom, decoded.spec.palette))
        map_effects = copy.deepcopy(effects["per_map"].get(record["id"], []))
        totals = resolve_palette_effects(rom, record, decoded, map_effects, sprites, routines, extents,
                                         registry, census, palette,
                                         int(effects["census"]["jump_table"], 16))
        chests = _treasure_chests(record)
        bind_chest_sprites(rom, chests, palette, record["general_var"], registry)
        per_map.append({"id": record["id"], "npcs": [meta.to_json() for meta in sprites],
                        "artless": artless, "chests": [chest["sprite"] for chest in chests],
                        "deferred_effects": [path["deferred_effects"] for effect in map_effects
                                             for path in effect.get("paths", ())
                                             if "deferred_effects" in path],
                        "palette_totals": totals})
    return {"per_map": per_map, "census": census.to_json()}


def _probe_warp_rects(rom: bytes):
    from psiv_tools.warps import PackError, warp_rect
    rects = []
    for range_id in range(16):
        for x, y in ((0, 0), (3, 2), (40, 60), (200, 200)):
            try:
                rect = warp_rect(range_id, x, y, 64, 72)
                rects.append(rect.to_json() if rect is not None else None)
            except PackError as error:
                rects.append(str(error))
    return rects


#: The ported extractors no claimed section writes yet, by the key
#: `examples/probe.rs` prints each under: the Python side of the probe.
PROBES = {"cram": _probe_cram, "layouts": _probe_layouts, "map_effects": _probe_map_effects,
          "map_sprites": _probe_map_sprites, "map_updates": _probe_map_updates, "maps": _probe_maps,
          "names": _probe_names, "warp_rects": _probe_warp_rects}
PYTHON_PREFIX = "psiv-tools: error: "
RUST_PREFIX = "psiv-pack: error: "
PROGRESS = re.compile(r"\Aprogress (\d+)/(\d+) (\S+)\Z")


# ---------------------------------------------------------------------------
# Comparison
# ---------------------------------------------------------------------------
def _files(root: Path) -> dict[str, Path]:
    """Every file under `root`, by its root-relative POSIX path."""
    found = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or path.is_file():
            found[path.relative_to(root).as_posix()] = path
    return found


def _under(relative: str, claim: str) -> bool:
    """A claim names a file, a directory and everything in it, or, with a `*`
    in its last part (`maps/*.png`), the matching files directly in a directory."""
    if "*" in claim:
        return fnmatch.fnmatchcase(relative, claim) and relative.count("/") == claim.count("/")
    return relative == claim or relative.startswith(claim.rstrip("/") + "/")


def _claim_exists(root: Path, claim: str) -> bool:
    return any(root.glob(claim)) if "*" in claim else (root / claim).exists()


def check_output(rust_root: Path, claims: list[str]) -> list[str]:
    """`psiv-pack`'s installed output on its own: a directory, holding no
    link and no file outside its claims. Checked on every run, claims or
    none."""
    if not rust_root.is_dir():
        return [f"psiv-pack left no pack directory at {rust_root.name}"]
    problems = []
    for relative, path in _files(rust_root).items():
        if path.is_symlink():
            problems.append(f"{relative}: psiv-pack wrote a symbolic link")
        elif not any(_under(relative, claim) for claim in claims):
            problems.append(f"{relative}: psiv-pack wrote it outside its claims")
    return problems


def compare(rust_root: Path, python_root: Path, claims: list[str]) -> tuple[list[str], int]:
    """The differences between the two packs under `claims`, and how many
    claimed files matched. Empty problems is parity."""
    problems = check_output(rust_root, claims)
    rust = _files(rust_root)
    python = _files(python_root)
    for claim in claims:
        if not _claim_exists(python_root, claim):
            problems.append(f"claim {claim}: the Python pack has no such path")
    claimed = sorted({relative for relative in [*rust, *python]
                      if any(_under(relative, claim) for claim in claims)})
    matched = 0
    for relative in claimed:
        if relative not in python:
            if relative in rust and not rust[relative].is_symlink():
                problems.append(f"{relative}: psiv-pack wrote it, the Python pack has none")
            continue
        if relative not in rust:
            problems.append(f"{relative}: missing from psiv-pack's pack")
            continue
        if rust[relative].is_symlink():
            continue
        ours, theirs = rust[relative].read_bytes(), python[relative].read_bytes()
        if relative.endswith(".png"):
            try:
                same = image_sha256(ours) == image_sha256(theirs)
            except ValueError as error:
                problems.append(f"{relative}: not a readable PNG ({error})")
                continue
            if not same:
                problems.append(f"{relative}: the decoded image differs")
                continue
        elif ours != theirs:
            offset = next((i for i, (a, b) in enumerate(zip(ours, theirs)) if a != b),
                          min(len(ours), len(theirs)))
            problems.append(f"{relative}: bytes differ from offset {offset} "
                            f"({len(ours)} bytes against {len(theirs)})")
            continue
        matched += 1
    return problems, matched


# ---------------------------------------------------------------------------
# Writer fixtures
# ---------------------------------------------------------------------------
def _tag(value):
    """`value` in a form that keeps what JSON text loses: int against float,
    a float's exact bits, an object's key order."""
    if value is None:
        return {"null": None}
    if isinstance(value, bool):
        return {"bool": value}
    if isinstance(value, int):
        return {"int": str(value)}
    if isinstance(value, float):
        return {"float": str(struct.unpack("<Q", struct.pack("<d", value))[0])}
    if isinstance(value, str):
        return {"str": value}
    if isinstance(value, list):
        return {"array": [_tag(item) for item in value]}
    return {"object": [[key, _tag(item)] for key, item in value.items()]}


def _json_cases():
    rng = random.Random(216)
    floats = [0.0, -0.0, 1.0, 0.5, 2.0, 0.1, 0.1 + 0.2, 1e-4, 1e-5, 9.999e-5, 1e15, 1e16,
              9.999999999999998e15, 1e22, 1e100, 5e-324, 1.7976931348623157e308,
              float("inf"), float("-inf"), 123456789.125, -2.5e-7]
    floats += [rng.uniform(-1000, 1000) for _ in range(60)]
    floats += [rng.random() * 10 ** rng.randint(-30, 30) for _ in range(120)]
    while len(floats) < 400:
        (bits,) = struct.unpack("<d", rng.getrandbits(64).to_bytes(8, "little"))
        if bits == bits and abs(bits) != float("inf"):
            floats.append(bits)
    strings = ["", "plain", "quote \" backslash \\ slash /", "\n\r\t\b\f", "\x00\x01\x1f\x7f",
               "café ÿ Ā € ￿", "\U0001F600 \U00010000 \U0010FFFF", "~ !"]
    unsorted = {"b": 1, "a": [], "C": {}, "é": None, "aa": True, "_": False,
                "10": 1, "9": 2, "": "empty"}
    nested = {"z": [1, [2, [3, {}]], {"y": {"x": [None]}}], "a": {"k": -12345678901234567890}}
    values = [None, True, False, 0, -1, 2 ** 63, -(2 ** 63), [], {}, [[]], [{}], {"a": []},
              strings, floats, unsorted, nested, {"pairs": [unsorted, nested]}]
    # Both orders only where there is an object to order.
    return [{"sort_keys": sort_keys, "value": _tag(value),
             "text": json.dumps(value, indent=2, sort_keys=sort_keys)}
            for value in values
            for sort_keys in ((True, False) if "{" in json.dumps(value) else (True,))]


def _png_cases():
    rng = random.Random(2160)
    cases = []
    shapes = [(1, 1, 1, []), (8, 8, 16, [0]), (37, 5, 3, [1, 2]), (300, 2, 256, [255]),
              (16, 3, 2, []), (5, 7, 64, [3, 60])]
    for width, height, colours, transparent in shapes:
        palette = [tuple(rng.randrange(256) for _ in range(3)) for _ in range(colours)]
        pixels = bytes(rng.randrange(colours) for _ in range(width * height))
        data = encode_indexed(width, height, pixels, palette, transparent)
        cases.append({"kind": "indexed", "width": width, "height": height,
                      "pixels": base64.b64encode(pixels).decode(),
                      "palette": [list(c) for c in palette], "transparent": transparent,
                      "png": base64.b64encode(data).decode(), "image_sha256": image_sha256(data)})
    for width, height in [(1, 1), (13, 4), (2, 90)]:
        pixels = bytes(rng.randrange(256) for _ in range(width * height * 3))
        data = encode_rgb(width, height, pixels)
        cases.append({"kind": "rgb", "width": width, "height": height,
                      "pixels": base64.b64encode(pixels).decode(),
                      "png": base64.b64encode(data).decode(), "image_sha256": image_sha256(data)})
    return cases


def harness_fixture():
    """What `--write-fixtures` writes: synthetic cases, no ROM content."""
    return {"json": _json_cases(), "png": _png_cases(),
            "source": "tools/pack_parity.py --write-fixtures"}


def fixture_text():
    return json.dumps(harness_fixture(), indent=2, sort_keys=True) + "\n"


# ---------------------------------------------------------------------------
# Probes
# ---------------------------------------------------------------------------
def compare_probe(python: str, rust: str) -> list[str]:
    """The probe documents differ: name the first differing line."""
    if python == rust:
        return []
    ours, theirs = rust.splitlines(), python.splitlines()
    line = next((i for i, (a, b) in enumerate(zip(ours, theirs)) if a != b),
                min(len(ours), len(theirs)))
    shown = lambda lines: lines[line] if line < len(lines) else "<end>"  # noqa: E731
    return [f"probe: line {line + 1} differs: psiv-pack {shown(ours)!r}, "
            f"Python {shown(theirs)!r} ({len(ours)} lines against {len(theirs)})"]


def check_probes(rom_path: Path) -> list[str]:
    """Run every probe in both builders and compare the documents."""
    rom = rom_path.read_bytes()
    try:
        with decode_cache.scope():
            python = json.dumps({name: probe(rom) for name, probe in PROBES.items()},
                                indent=2, sort_keys=True)
    except ValueError as error:
        return [f"probe: the Python extractors refused the ROM: {error}"]
    run = subprocess.run([str(PROBE_BINARY), str(rom_path)], cwd=ROOT, capture_output=True,
                         text=True, encoding="utf-8")
    if run.returncode != 0:
        return [f"probe: psiv-pack exited {run.returncode}: {run.stderr.strip()}"]
    print(f"pack parity: probed {', '.join(PROBES)}", flush=True)
    return compare_probe(python, run.stdout.rstrip("\n"))


# ---------------------------------------------------------------------------
# Decoders
# ---------------------------------------------------------------------------
def census_calls(rom: bytes) -> list[dict]:
    """Every decompressor call a full Python build attempts, as census
    entries (`census_entries`)."""
    from psiv_tools import decode_cache, pack_sections
    from psiv_tools.pack import build_pack
    with pack_sections.fresh(), decode_cache.scope() as table, \
            tempfile.TemporaryDirectory(prefix="psiv-census-") as scratch:
        build_pack(rom, Path(scratch))
    return census_entries(table, rom)


def census_entries(table, rom: bytes) -> list[dict]:
    """A decode table's decompressor calls as census entries: the decoder and
    its arguments, never the bytes, one per distinct call. Every attempt is
    listed, including one the decoder refused and its caller caught
    (`battle_art._is_mapping` tells art from mappings that way), so both
    builders must refuse it alike. A call that does not read the ROM itself is
    an error, because the census could not name its input."""
    if table.unkeyed:
        raise ValueError(f"{table.unkeyed} decoder calls read a mutable buffer; "
                         "the census can only name ROM offsets")
    calls = []
    for name, data, *args in table.attempts:
        if name not in DECODERS:
            continue
        if data is not rom and data != rom:
            raise ValueError(f"{name} decoded something other than the ROM; "
                             "the census can only name ROM offsets")
        entry = {"decoder": DECODERS[name], "offset": args[0]}
        if DECODERS[name] == "enigma":
            entry.update(base_tile=args[1], max_words=args[2])
        calls.append(entry)
    return sorted(calls, key=lambda c: (c["decoder"], c["offset"], c.get("base_tile", 0),
                                        c.get("max_words") is not None, c.get("max_words") or 0))


def python_decode(rom: bytes, call: dict) -> dict:
    """One census call through the Python decoder: `{consumed, sha256}` of
    the bytes it leaves in memory (Enigma's words big-endian), or `{error}`."""
    from psiv_tools import enigma, kosinski, nemesis
    try:
        if call["decoder"] == "enigma":
            words, consumed = enigma.decompress(rom, call["offset"], call["base_tile"],
                                                call["max_words"])
            out = enigma.words_to_bytes(words)
        else:
            module = nemesis if call["decoder"] == "nemesis" else kosinski
            out, consumed = module.decompress(rom, call["offset"])
    except ValueError as error:
        return {"error": str(error)}
    return {"consumed": consumed, "sha256": hashlib.sha256(out).hexdigest()}


def compare_decodes(calls: list[dict], python: list[dict], rust: list[dict]) -> list[str]:
    """The census calls whose Rust result differs from Python's. An error is
    a result: both must refuse the same calls, though not in the same words."""
    if len(rust) != len(calls) or len(python) != len(calls):
        return [f"decode census: {len(calls)} calls, {len(python)} Python results, "
                f"{len(rust)} psiv-pack results"]
    problems = []
    for call, ours, theirs in zip(calls, rust, python):
        where = f"{call['decoder']} at 0x{call['offset']:X}"
        if call["decoder"] == "enigma":
            where += f" (base 0x{call['base_tile']:X}, max {call['max_words']})"
        if ("error" in ours) != ("error" in theirs):
            problems.append(f"{where}: psiv-pack {ours.get('error', 'decoded it')}, "
                            f"Python {theirs.get('error', 'decoded it')}")
        elif "error" not in ours and ours != theirs:
            what = "consumed length" if ours["sha256"] == theirs["sha256"] else "output"
            problems.append(f"{where}: the {what} differs")
    return problems


def check_decodes(rom_path: Path) -> list[str]:
    """Decode the census with both builders and compare."""
    census = json.loads(CENSUS.read_text(encoding="utf-8"))
    calls = census["calls"]
    rom = rom_path.read_bytes()
    start = time.monotonic()
    python = [python_decode(rom, call) for call in calls]
    python_s = time.monotonic() - start
    run = subprocess.run([str(CENSUS_BINARY), str(rom_path), str(CENSUS)], cwd=ROOT,
                         capture_output=True, text=True)
    if run.returncode != 0:
        return [f"decode census: psiv-pack exited {run.returncode}: {run.stderr.strip()}"]
    lines = [json.loads(line) for line in run.stdout.splitlines()]
    seconds = lines.pop()["seconds"]
    print(f"pack parity: decoded {len(calls)} census streams; psiv-pack {seconds:.3f} s, "
          f"Python {python_s:.1f} s", flush=True)
    return compare_decodes(calls, python, lines)


# ---------------------------------------------------------------------------
# The parity run
# ---------------------------------------------------------------------------
def _error_line(stderr: str, prefix: str) -> str | None:
    lines = [line for line in stderr.splitlines() if line.startswith(prefix)]
    return lines[-1][len(prefix):] if lines else None


def check_wrong_rom(binary: Path, scratch: Path) -> list[str]:
    """Both builders refuse a wrong ROM with the same message."""
    wrong = scratch / "wrong-rom"
    wrong.write_bytes(bytes(0x400))
    relative = os.path.relpath(wrong, ROOT)
    python = subprocess.run([sys.executable, "-m", "psiv_tools", "pack", relative,
                             os.path.relpath(scratch / "wrong-python", ROOT)],
                            cwd=ROOT, capture_output=True, text=True)
    rust = subprocess.run([str(binary), relative, os.path.relpath(scratch / "wrong-rust", ROOT)],
                          cwd=ROOT, capture_output=True, text=True)
    problems = []
    for name, run in (("python", python), ("psiv-pack", rust)):
        if run.returncode == 0:
            problems.append(f"wrong ROM: {name} exited 0")
    for name in ("wrong-python", "wrong-rust"):
        if os.path.lexists(scratch / name):
            problems.append(f"wrong ROM: {name} was created")
    ours, theirs = _error_line(rust.stderr, RUST_PREFIX), _error_line(python.stderr, PYTHON_PREFIX)
    if ours is None or theirs is None or ours != theirs:
        problems.append(f"wrong ROM: psiv-pack says {ours!r}, the Python builder {theirs!r}")
    return problems


def check_progress(stdout: str, sections: int) -> list[str]:
    """`psiv-pack`'s stdout: a progress line per section, then `installed`."""
    lines = stdout.splitlines()
    if not lines or not lines[-1].startswith("installed "):
        return [f"psiv-pack's last line is not `installed <pack>`: {lines[-1:]!r}"]
    problems, seen = [], []
    for line in lines[:-1]:
        match = PROGRESS.match(line)
        if not match:
            problems.append(f"not a progress line: {line!r}")
            continue
        seen.append(int(match.group(1)))
        if int(match.group(2)) != sections:
            problems.append(f"progress total {match.group(2)}, not {sections}: {line!r}")
    if seen != list(range(1, sections + 1)):
        problems.append(f"progress counted {seen}, not 1..{sections}")
    return problems


def _clock():
    mhz = cpu_mhz()
    return f"load {load_1m()}, {mhz:.0f} MHz" if mhz else f"load {load_1m()}, clock unknown"


def run_parity(rom: Path, keep: Path | None) -> int:
    if not rom.is_file():
        print(f"pack parity: missing input, the ROM at {rom}", flush=True)
        return 1
    with hold_heavy_lock(lock_class=BUILD):
        built = subprocess.run(CARGO_BUILD, cwd=ROOT)
    if built.returncode != 0:
        print("pack parity: psiv-pack did not build", flush=True)
        return 1
    sections = json.loads(subprocess.run([str(BINARY), "--claims"], cwd=ROOT, check=True,
                                         capture_output=True, text=True).stdout)["sections"]
    claims = [claim for section in sections for claim in section["claims"]]
    build_dir = ROOT / "build" / "pack-parity"
    build_dir.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="run-", dir=build_dir)) if keep is None else keep
    scratch.mkdir(parents=True, exist_ok=True)
    try:
        problems = check_wrong_rom(BINARY, scratch)
        problems += check_probes(rom)
        with hold_heavy_lock(lock_class=PACK):
            problems += check_decodes(rom)
        with hold_heavy_lock(lock_class=PACK):
            print(f"pack parity: psiv-pack, {_clock()}", flush=True)
            start = time.monotonic()
            rust = subprocess.run([str(BINARY), str(rom), str(scratch / "rust")], cwd=ROOT,
                                  capture_output=True, text=True)
            seconds = time.monotonic() - start
            print(f"pack parity: psiv-pack took {seconds:.2f} s (release, {os.cpu_count()} cores, "
                  f"{_clock()})", flush=True)
            if rust.returncode != 0:
                problems.append(f"psiv-pack exited {rust.returncode}: {rust.stderr.strip()}")
            elif not claims:
                problems += check_output(scratch / "rust", claims)
            problems += check_progress(rust.stdout, len(sections))
            matched = 0
            if claims:
                start = time.monotonic()
                python = subprocess.run([sys.executable, "-m", "psiv_tools", "pack", str(rom),
                                         str(scratch / "python")], cwd=ROOT)
                print(f"pack parity: the Python builder took {time.monotonic() - start:.1f} s",
                      flush=True)
                if python.returncode != 0:
                    problems.append(f"the Python builder exited {python.returncode}")
                else:
                    found, matched = compare(scratch / "rust", scratch / "python", claims)
                    problems += found
            else:
                print("pack parity: psiv-pack claims no sections yet; Python build skipped",
                      flush=True)
    finally:
        if keep is None:
            shutil.rmtree(scratch, ignore_errors=True)
    for problem in problems:
        print(f"pack parity: {problem}", flush=True)
    print(f"pack parity: {len(claims)} claimed paths, {matched} files matched, "
          f"{len(problems)} problems", flush=True)
    return 1 if problems else 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--keep", type=Path, help="build into this directory and keep it")
    parser.add_argument("--write-census", action="store_true",
                        help=f"rewrite {CENSUS.relative_to(ROOT)} from a full Python build "
                             "(needs the ROM) and exit")
    parser.add_argument("--write-fixtures", action="store_true",
                        help=f"rewrite {FIXTURE.relative_to(ROOT)} and exit")
    args = parser.parse_args(argv)
    if args.write_census:
        from psiv_tools.core import read_rom
        with hold_heavy_lock(lock_class=PACK):
            calls = census_calls(read_rom(args.rom))
        CENSUS.write_text(json.dumps({"calls": calls, "source": "tools/pack_parity.py "
                                      "--write-census"}, indent=2, sort_keys=True) + "\n",
                          encoding="utf-8", newline="\n")
        print(f"wrote {CENSUS.relative_to(ROOT)}: {len(calls)} calls")
        return 0
    if args.write_fixtures:
        FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        FIXTURE.write_text(fixture_text(), encoding="utf-8", newline="\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    return run_parity(args.rom, args.keep)


if __name__ == "__main__":
    raise SystemExit(main())
