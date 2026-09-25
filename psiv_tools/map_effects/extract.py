"""One jump-table entry decoded, every map's list, and the census.

`decode_entry` is the unit a consumer re-reaches by index: an entry that the
reader refuses comes back with `undecoded` set to why and its own bytes for
provenance, never dropped. `extract_map_effects` walks every real map's
`MapDataManager` list, resolves each `layout_write` against that map's own
geometry, and counts what it found -- kinds, gate banks, flag clears, the
object writes that land past a map's object count (a no-op on a fresh load, not
a defect) and the entries nothing could decode.

The per-map lists are keyed by map id so `psiv_tools.pack` can drop each one
into that map's own record: the data arrives with the map it patches, and a
kind a consumer does not know fails that map's build rather than the whole pack.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .anchors import (
    FLAG_BANK_ADDRESSES,
    MAP_DATA_MANAGER_ROUTINES,
    _kos_decomp,
    dispatch_table,
    flag_clears,
    flag_tests,
    map_layout_bases,
    routine_address,
)
from .decoder import _Decoder
from .geometry import MapGeometry, _geometry, resolve_layout_write
from .model import MapEffectsError, Path


@dataclass
class Entry:
    index: int
    rom_offset: int
    paths: list[Path]
    undecoded: str | None
    #: The routine's bytes, for provenance. Bounded by the furthest byte any
    #: path read, so it is the routine as decoded rather than a fixed window.
    raw_hex: str = ""

    @property
    def kinds(self) -> tuple[str, ...]:
        return tuple(sorted({w.kind for p in self.paths for w in p.writes}))

    def to_json(self, geometry: "MapGeometry | None" = None) -> dict[str, Any]:
        """This entry as the pack emits it.

        `geometry` is the map's layout shape and the two buffer addresses. A
        `layout_write`'s displacement is applied to an address, so it only
        becomes a coordinate once both the row sizes and the buffer bases are
        known -- which is why the pack always passes them.
        """
        out: dict[str, Any] = {
            "entry": self.index,
            "entry_hex": f"0x{self.index:02X}",
            "routine": f"0x{self.rom_offset:06X}",
            "raw_hex": self.raw_hex,
        }
        if self.undecoded is not None:
            out["decoded"] = False
            out["reason"] = self.undecoded
            return out
        out["decoded"] = True
        out["kinds"] = list(self.kinds)
        out["paths"] = [
            {
                "gates": [g.to_json() for g in path.gates],
                # A path with an unmodelled condition is not unconditional; a
                # consumer applying it on every load would be wrong.
                "unconditional": not path.gates and not path.conditions,
                "unmodelled_conditions": path.conditions,
                "aborts_remaining_entries": path.aborts,
                "deferred": path.deferred,
                "writes": [
                    resolve_layout_write(w, geometry.widths, geometry.bases,
                                         geometry.heights)
                    if w.kind == "layout_write" and geometry else w.to_json()
                    for w in path.writes
                ],
            }
            for path in self.paths
            # A path that does nothing and gates nothing is the routine's own
            # "flag not set, return" arm; it carries no information.
            if path.writes or path.aborts
        ]
        return out


def decode_entry(rom: bytes, index: int, *, table: int | None = None,
                 tests: dict[int, str] | None = None, kos: int | None = None,
                 clears: dict[int, str] | None = None) -> Entry:
    table = dispatch_table(rom) if table is None else table
    tests = flag_tests(rom) if tests is None else tests
    clears = flag_clears(rom) if clears is None else clears
    kos = _kos_decomp(rom) if kos is None else kos
    address = routine_address(rom, table, index)
    decoder = _Decoder(rom, address, tests, kos, clears)
    try:
        paths = decoder.run()
    except MapEffectsError as exc:
        return Entry(index, address, [], str(exc),
                     rom[address:address + 0x40].hex())
    end = max((w.at for p in paths for w in p.writes), default=address) + 8
    return Entry(index, address, paths, None, rom[address:end].hex())


def extract_map_effects(rom: bytes, maps: list[dict[str, Any]]) -> dict[str, Any]:
    """Every map's `MapDataManager` list, decoded, with the census.

    `maps` is `psiv_tools.maps.extract_maps(rom)["maps"]`. The per-map lists are
    keyed by map id so `psiv_tools.pack` can drop each one into that map's own
    record: the data arrives with the map it patches, and a kind a consumer does
    not know fails that map's build rather than the whole pack.
    """
    ctx = dict(table=dispatch_table(rom), tests=flag_tests(rom),
               kos=_kos_decomp(rom), clears=flag_clears(rom))
    bases = map_layout_bases(rom)
    real = [record for record in maps if not record["is_null"]]
    referenced = sorted({i for r in real for i in r["map_data_manager"]["ids"]})
    entries = {index: decode_entry(rom, index, **ctx) for index in referenced}

    per_map: dict[int, list[dict[str, Any]]] = {}
    past_end: list[dict[str, Any]] = []
    kinds: dict[str, int] = {}
    banks: dict[str, int] = {}
    flags: dict[str, set[int]] = {}
    maps_per_kind: dict[str, set[int]] = {}
    undecoded: list[dict[str, Any]] = []
    unconditional = gated = position_gated = 0
    clears: dict[str, dict[int, set[int]]] = {}
    replacements: list[dict[str, Any]] = []

    for record in real:
        ids = record["map_data_manager"]["ids"]
        if not ids:
            continue
        geometry = _geometry(record, bases)
        emitted = [entries[index].to_json(geometry) for index in ids]
        per_map[record["id"]] = emitted
        for entry in emitted:
            if not entry["decoded"]:
                continue
            for path in entry["paths"]:
                if path["gates"]:
                    gated += 1
                elif path["unconditional"]:
                    unconditional += 1
                else:
                    position_gated += 1
                for gate in path["gates"]:
                    banks[gate["bank"]] = banks.get(gate["bank"], 0) + 1
                    flags.setdefault(gate["bank"], set()).add(gate["flag"])
                for write in path["writes"]:
                    kinds[write["kind"]] = kinds.get(write["kind"], 0) + 1
                    maps_per_kind.setdefault(write["kind"], set()).add(record["id"])
                    index = write.get("object_index")
                    if index is not None and index >= record["objects"]["count"]:
                        past_end.append({
                            "map": record["id"], "entry": entry["entry_hex"],
                            "at": write["at"], "object_index": index,
                            "map_objects": record["objects"]["count"],
                        })
                    if write["kind"] == "flag_clear":
                        (clears.setdefault(write["bank"], {})
                              .setdefault(entry["entry"], set())
                              .add(write["flag"]))
                    if write["kind"] == "layout_replace":
                        replacements.append({
                            "map": record["id"], "plane": write["plane"],
                            "source": write["source"],
                        })

    for index in referenced:
        entry = entries[index]
        if entry.undecoded is not None:
            undecoded.append({
                "entry": index,
                "entry_hex": f"0x{index:02X}",
                "routine": f"0x{entry.rom_offset:06X}",
                "reason": entry.undecoded,
                "maps": sorted(r["id"] for r in real
                               if index in r["map_data_manager"]["ids"]),
            })

    return {
        "per_map": per_map,
        "replacements": replacements,
        # The jump table's address, for a caller that needs to re-reach a
        # routine by entry index -- `routine_address(rom, dispatch_table, entry)`.
        # The census carries the same value formatted for a reader.
        "dispatch_table": ctx["table"],
        "census": {
            "jump_table": f"0x{ctx['table']:06X}",
            "table_entries": MAP_DATA_MANAGER_ROUTINES,
            "referenced_entries": len(referenced),
            "unreferenced_entries": MAP_DATA_MANAGER_ROUTINES - len(referenced),
            "maps_with_entries": len(per_map),
            "map_entry_pairs": sum(len(v) for v in per_map.values()),
            "decoded_entries": len(referenced) - len(undecoded),
            "undecoded_entries": undecoded,
            "kinds": {k: kinds[k] for k in sorted(kinds)},
            "maps_per_kind": {k: len(maps_per_kind[k]) for k in sorted(maps_per_kind)},
            "gate_banks": {k: banks[k] for k in sorted(banks)},
            # Map-load flag clears, by the bank whose door was called. Emitted
            # per entry because that is how a runtime consumes them: it has the
            # map's entry list already. `bank_address` on each write is the
            # stable key -- the `$F140` bank's *name* is under review.
            "flag_clears": {
                bank: {
                    "bank_address": f"0xFFFF{FLAG_BANK_ADDRESSES[bank]:04X}",
                    "entries": len(by_entry),
                    "flag_ids": sorted({f for ids in by_entry.values() for f in ids}),
                    "by_entry": {
                        f"0x{entry:02X}": sorted(ids)
                        for entry, ids in sorted(by_entry.items())
                    },
                }
                for bank, by_entry in sorted(clears.items())
            },
            "flags_per_bank": {k: sorted(flags[k]) for k in sorted(flags)},
            # Some routines clear more slots than the map has objects. On a
            # fresh load those slots are empty, so the write is a no-op -- the
            # routine is written for the largest map that uses it. A consumer
            # should ignore an index past the map's object count rather than
            # treat it as a defect.
            "object_writes_past_map_object_count": past_end,
            "paths_gated": gated,
            # Reached only under a condition this slice recognises but does
            # not model -- currently the two player-position compares.
            "paths_position_gated": position_gated,
            "paths_unconditional": unconditional,
            "evaluated": "map load only",
            "note": (
                "MapDataManager is reached from two jsr sites, both in a map "
                "loader, and Map_Data_Manager_Addr is written at both and read "
                "nowhere: these apply when a map is built and are never "
                "re-evaluated while it is loaded. A routine returning non-zero "
                "aborts the rest of that map's list, which is what "
                "aborts_remaining_entries carries; no retail routine does."
            ),
        },
    }
