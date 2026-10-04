"""The runtime pack's format version: one owner, re-exported by `psiv_tools.pack`."""

from __future__ import annotations

#: Bumped whenever a field in the emitted JSON changes meaning or disappears.
#: `psiv-data` refuses a pack whose version it does not know.
#:
#: 1 -- field sprites: `sprites/party.json`, `sprites/npcs.json`, their PNG
#: sheets, and a `sprite` reference on every map record's NPC entries. The two
#: overworlds joined at the same version: their records use every field the
#: others do, and the one thing they add -- `layout_patches`, the event-gated
#: chunk writes their page loader performs -- is a key no other map carries and
#: no existing key changed meaning for. `interactable` on every NPC entry and
#: the `field_objects` table in `sprites/npcs.json` join on the same reasoning:
#: both are new keys, and nothing that was already emitted reads differently.
PACK_FORMAT_VERSION = 1
