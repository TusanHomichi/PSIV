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

The static-only enemy-art boundary is closed. The extractor now decodes
`EnemySpriteMappingsOffs`, the two retail sequence encodings, Art #2 source
patterns, Art #3 destination tiles, and body-relative placements. The additive
pack emits deterministic full-body overlay frames, and the Godot renderer
rebuilds the body with those replacement tiles at the retail `duration + 1`
cadence. A global art-bank permutation remains invalid; it would break the
existing body assets.

The old Wayland capture of `--psiv-debug-battle=0x88` is historical evidence
only. The state-matched fixture now uses an empty presentation timeline, so
the settled shot contains the two red Zoran bodies and no debug lightning.
`oracle/layouts/battle_command_idle.json` has no visible
SAT entries for this idle fixture, so its position proof is the decoded body
anchor plus the overlay placement metadata; the SAT artifact is not being
claimed as evidence where it contains no entries. The runtime frame still
requires a fresh compliant capture before an RMSE can be claimed.

The managed shell's Xvfb path is currently blocked before the game starts:
`/tmp/.X11-unix` is owned by `nobody`, and Xvfb refuses that socket directory.
Changing global socket ownership or deleting stale sockets would be an
unapproved host mutation, so the same built binary was verified through the
Wayland capture path instead. The debug environment hook remains intact; the
CLI selector is a deterministic fallback for this host-specific display-env
failure.
