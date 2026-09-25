"""Map effects: the cartridge's flag-gated patches to a loaded map.

When a map loads, the last section of its record is a `$FFFF`-terminated list
of indexes into `MapDataManagerJmpTbl`. `MapDataManager` (`0x051B38`) walks it
and calls each routine, and those routines are what make a map react to the
story: Igglanova is gone after you beat it, Zema's doors open, Alys is not
standing in the academy once you have found her.

Three facts about the mechanism decide the shape of everything below, and all
three are read from the cartridge rather than assumed.

**They run at map load and only at map load.** `MapDataManager` is reached from
exactly two `jsr` sites in the image, both inside a map loader, and
`Map_Data_Manager_Addr` is written at both and read nowhere. A flag that
changes while the player is standing on a map changes nothing until the map is
loaded again. A runtime that re-applies these on a flag change is not
reproducing the cartridge.

**A routine can abort the rest of the list.** The dispatcher is
`jsr (a1,d0.w)` followed by `bne.s` to `GoPast_FFFF_Terminator`, so a routine
that returns non-zero skips every remaining entry for that map. Every retail
routine ends `moveq #0,d7 / rts`, so it never fires -- but the emitted data
carries the bit per path, because a consumer that ignores it would apply
patches the cartridge would have skipped.

**Object patches address record objects, not RAM.** `LoadMapObjects` fills
slots through `Field_LoadObject`, a linear first-free scan from
`Field_Obj_Secondary` (`$FFFFC300`) in `$40` steps, so on a fresh load the
record's object N is at `$C300 + N * $40`. Every object write in the game --
106 of them -- lands on an aligned slot inside its own map's object count, so
this module emits the record index and never a RAM address.

Decoding
--------

The routines are small, straight-line-with-forward-branches 68000. This module
walks them with a reader for exactly the instruction forms they use, forking at
each conditional branch so that a write is emitted together with the flag
conditions that reach it. That is why the output is a list of *paths*: a
routine like `MapDataMan_VahFortMovPlatforms` writes the same chunk ids at one
of two rows depending on a temporary flag, and a model that flattened it would
have to pick one.

Anything outside the reader's vocabulary aborts that entry, which is then
reported with its offset and bytes rather than half-decoded. Slice 1 covers the
kinds that change what the player can see, walk through and talk to:

    object_despawn    clear an object slot -- the object never spawns
    object_rewrite    write a different object id into a slot
    object_dialogue   change an object's dialogue id
    layout_write      write chunk ids into `Map_Layout` through
                      `GetMapLayoutOffset` (doors, bridges, blocked paths)
    layout_replace    decompress an entirely different layout over a plane

Palette, scroll, chunk-table and temporary-flag-only routines are later slices
and are reported as `deferred` rather than silently dropped.

Modules
-------

Five files, one concern each, and this one is the package's public face: the
names the rest of the repository imports come from here.

    anchors.py    where the routines, the flag banks and the layout buffers are
                  in the image, derived from it and refusing a disagreement
    model.py      the decoded vocabulary: the error, a `Gate`, a `Write`, a
                  `Path`, and the kinds Slice 1 emits
    decoder.py    the instruction reader, which walks one routine into paths
    geometry.py   the two layout buffers, and a write's displacement resolved
                  to the cell it lands on
    extract.py    one jump-table entry decoded, every map's list, and the
                  census the manifest carries
"""

from __future__ import annotations

from .anchors import (
    FIELD_OBJ_LAST,
    FIELD_OBJ_SECONDARY,
    FIELD_OBJ_STRIDE,
    FLAG_BANKS,
    FLAG_BANK_ADDRESSES,
    FLAG_CLEAR_BLOCK,
    FLAG_CLEAR_DOORS,
    FLAG_SET_BLOCK,
    FLAG_TEST_BLOCK,
    FLAG_TEST_STRIDE,
    GET_MAP_LAYOUT_OFFSET,
    KOS_DECOMP,
    MAP_DATA_MANAGER_ROUTINES,
    OBJECT_DIALOGUE_OFFSET,
    dispatch_table,
    flag_clears,
    flag_tests,
    map_layout_bases,
    routine_address,
)
from .decoder import COMPARE_BRANCHES
from .extract import Entry, decode_entry, extract_map_effects
from .geometry import (
    MAP_LAYOUT,
    MAP_LAYOUT_BYTES,
    MapGeometry,
    resolve_layout_write,
)
from .model import SLICE_ONE_KINDS, Gate, MapEffectsError, Path, Write

__all__ = [
    "COMPARE_BRANCHES",
    "Entry",
    "FIELD_OBJ_LAST",
    "FIELD_OBJ_SECONDARY",
    "FIELD_OBJ_STRIDE",
    "FLAG_BANKS",
    "FLAG_BANK_ADDRESSES",
    "FLAG_CLEAR_BLOCK",
    "FLAG_CLEAR_DOORS",
    "FLAG_SET_BLOCK",
    "FLAG_TEST_BLOCK",
    "FLAG_TEST_STRIDE",
    "GET_MAP_LAYOUT_OFFSET",
    "Gate",
    "KOS_DECOMP",
    "MAP_DATA_MANAGER_ROUTINES",
    "MAP_LAYOUT",
    "MAP_LAYOUT_BYTES",
    "MapEffectsError",
    "MapGeometry",
    "OBJECT_DIALOGUE_OFFSET",
    "Path",
    "SLICE_ONE_KINDS",
    "Write",
    "decode_entry",
    "dispatch_table",
    "extract_map_effects",
    "flag_clears",
    "flag_tests",
    "map_layout_bases",
    "resolve_layout_write",
    "routine_address",
]
