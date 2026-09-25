"""The two layout buffers, and a write's displacement resolved to a cell.

`GetMapLayoutOffset` returns a pointer *into a buffer*, computed with the row
size of the plane it was asked for, and a routine then applies a displacement
to that address rather than to a coordinate -- so a displacement can leave the
plane it asked for. That is only resolvable with the buffers' own addresses and
the map's own row sizes, which is what this module owns: a `MapGeometry` is
those two shapes, and `resolve_layout_write` turns one write into the cell the
cartridge actually stamps, naming the plane it lands in when that is not the
plane the routine asked for.

The two buffer addresses and the distance between them are the reason a
`layout_write` can be resolved at all: the FG buffer sits exactly
`MAP_LAYOUT_BYTES` below the BG one, so `-$1000(a1)` off a BG pointer is the
same byte offset in the FG buffer.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .model import Write

#: The two layout buffers, from the `lea (Map_Layout_*).w, a1` pair inside
#: `GetMapLayoutOffset`. They are adjacent and exactly `MAP_LAYOUT_BYTES`
#: apart, which is the whole reason a routine can reach the *other* plane with
#: a negative displacement instead of asking for it: `-$1000(a1)` off a BG
#: pointer is the same byte offset in the FG buffer. `map_layout_bases`
#: re-derives both from the cartridge and refuses a disagreement.
MAP_LAYOUT = {0xA000: "fg", 0xB000: "bg"}
MAP_LAYOUT_BYTES = 0x1000


@dataclass(frozen=True)
class MapGeometry:
    """What a `layout_write` needs to become a cell on a particular map."""

    widths: dict[str, int]
    heights: dict[str, int]
    bases: dict[str, int]


def _geometry(record: dict[str, Any], bases: dict[str, int]) -> MapGeometry:
    """The map's layout shape per plane, in chunks. The record stores size-1."""
    dimensions = record["dimensions"]
    return MapGeometry(
        widths={
            "fg": dimensions["fg_row_size"] + 1,
            "bg": dimensions["bg_row_size"] + 1,
        },
        heights={
            "fg": dimensions["fg_column_size"] + 1,
            "bg": dimensions["bg_column_size"] + 1,
        },
        bases=bases,
    )


def resolve_layout_write(write: Write, widths: dict[str, int],
                         bases: dict[str, int],
                         heights: dict[str, int] | None = None) -> dict[str, Any]:
    """A layout write with its displacement resolved to a real cell.

    `GetMapLayoutOffset` returns a pointer *into a buffer*, computed with the
    row size of the plane it was asked for. The displacement is then applied to
    that address, not to a coordinate -- so it can leave the plane entirely.
    Three retail routines rely on that: they ask for BG, write the BG cell, and
    then reach `-$1000(a1)` and `-$1020(a1)` to stamp the matching FG cells,
    because the FG buffer sits exactly one buffer below the BG one.

    Folding the displacement into the asked-for plane's row arithmetic, which
    is what this used to do, turned those three into coordinates hundreds of
    rows negative. The address is resolved instead: which buffer it lands in
    names the plane actually written, and the offset within that buffer is read
    with *that* plane's row size.
    """
    detail = dict(write.detail)
    asked = detail["plane"]
    address = (
        bases[asked]
        + detail["chunk_y"] * widths[asked] + detail["chunk_x"]
        + detail["displacement"]
    )
    out: dict[str, Any] = {
        "kind": write.kind,
        "at": f"0x{write.at:06X}",
        "chunk_id": detail["chunk_id"],
        "requested_plane": asked,
        "displacement": detail["displacement"],
    }
    target = next(
        (name for name, base in bases.items()
         if base <= address < base + MAP_LAYOUT_BYTES),
        None,
    )
    if target is None:
        # No retail write takes this branch. It exists so that one ever
        # appearing is emitted as a refusal a consumer can skip, rather than as
        # a coordinate that looks addressable and is not.
        return {
            **out,
            "plane": asked,
            "out_of_bounds": True,
            "address": f"0xFFFF{address & 0xFFFF:04X}",
        }
    offset = address - bases[target]
    chunk_y, chunk_x = divmod(offset, widths[target])
    out.update({
        "plane": target,
        "crosses_plane": target != asked,
        "chunk_x": chunk_x,
        "chunk_y": chunk_y,
        "cell_x": chunk_x * 2,
        "cell_y": chunk_y * 2,
        "out_of_bounds": False,
    })
    # Inside the buffer but past the map's own layout: addressable, but not a
    # cell this map draws. Marked rather than dropped.
    if heights is not None and chunk_y >= heights[target]:
        out["past_map_layout"] = True
    return out
