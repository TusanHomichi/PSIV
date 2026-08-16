# Battle UI oracle slice

The decoded files under `oracle/layouts/` are the authority for the battle
screen's plane-A geometry. `docs/BATTLE_GEOMETRY.md` remains useful context,
but a disagreement is resolved in favor of the decoded layout.

## Implemented surface

`rust/psiv-godot/src/battle/chrome.rs` and `layout.rs` render the retail
window atlas, menu font, exact command cursor, one 36x6 status window with
decoded separator holes, HP/TP labels, and the decoded number glyph run.
`ui.rs` owns the exact enemy-name, command, victory, status, message, and
enemy-anchor constants. Its oracle-gated tests read the JSON artifacts when
present and assert the decoded rectangles, text origins, tile-run body spans,
and visible SAT coordinates.

`PSIV_DEBUG_BATTLE=0x88` is the command-idle oracle fixture: Chaz/Alys/Hahn
with 25/10, 53/40, and 21/25 HP/TP against two Zoran Bults at the decoded
body anchors `(88,72)` and `(184,72)`. It is a capture selector, not a raw
formation id.

## Deliberate boundary

The final Xvfb capture structurally matches the retail frame: windows,
messages, text origins, cursor, status panes, live numbers, party placement,
and enemy anchors agree. The remaining visual difference is enemy artwork.
The current `psiv_tools.battle_art` pack emits the static body layer; the
retail oracle frame also contains animated/overlay plane composition. A global
art-bank permutation is not a valid fix because the existing extractor's
ordering is covered by the broader pack tests and changing it breaks unrelated
enemy assets. Do not crop or copy pixels from `oracle/frames/`; the next art
slice must decode the missing overlay pipeline generically.

The headless container cannot run the bare `godot/run.sh` loop because no X11
display exists. The same built binary was exercised under Xvfb and saved a
fresh 320x224 crop for visual inspection.
