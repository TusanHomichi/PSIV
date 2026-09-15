# Level-up learning and early native save repair

The extracted level JSON already contained each earned technique, skill and
eight skill-use maxima. The data/runtime bridge dropped those fields, and
the core level-up routine only changed level and stats. A native character
could therefore reach level four without receiving TSU. This affected the
connected Academy-to-Tonoe save; it was not a difficulty or player-choice
issue.

## Original rules now applied

The US source's `BattleResults_PartyExp` copies the eight maximum skill uses.
`loc_42BA` appends the earned technique to the first empty slot. `loc_4358`
appends the earned skill and initializes that slot's current uses from its
new maximum. Existing party uses remain spent. HP and TP do not refill.
Godot now displays an acknowledged learned-ability message after the
character's level-up message.

The separate benched-character pass (`loc_3F4C` through `loc_3FC0`) learns
silently and copies both current and maximum skill uses. Each character
still gains at most one level per battle, matching US `bugfixes=0`.
The existing native fixes for level-99 table-pointer desynchronization and
refreshing derived equipment stats remain in place.

The original Seth level-99 row has an extra byte with `bugfixes=0`. Its
misaligned technique byte is 99. The bridge preserves that original byte
instead of rejecting the entire battle pack; menus filter unknown ability
IDs. This change does not claim to repair that original malformed row.

## Existing saves

Ordinary load does not infer missing abilities or rewrite a modded roster.
`Runtime::repair_legacy_progression` is an explicit, atomic, idempotent
operation. It restores catalog-known abilities earned by attained levels
and the attained-level skill maxima. Existing spent uses, HP, TP, EXP,
levels, equipment, money, inventory and world state stay unchanged. A newly
restored skill receives its original learning-level uses.

The `repair_progression` runtime example loads source slot 1, writes a new
output slot 1, rejects an existing output, and verifies the source file is
unchanged. The connected repair is recorded in
`build/native-progression/repaired-campaign/repair-receipt.json`:

| Logical payload byte | Change |
| --- | --- |
| `$453` | Chaz's second technique: empty to TSU (7) |
| `$46B` | Chaz's EARTH maximum: 3 to 5 |
| `$554` | Hahn's third technique: empty to WAT (4) |

These are the only changed payload bytes. The original native Tonoe file
remains at `build/native-tonoe/saves/slot_1.sram`; the repaired copy is under
`build/native-progression/repaired-campaign/`. The next ordinary paid inn
can refill the repaired skill maximum under the usual rest rules.

## Verification

Core regressions cover first-empty-slot learning, new-skill initialization,
existing spent uses, absent-character refill and the one-level limit.
The real-record runtime test wins an original two-Zoran-Bult formation with
three characters, earns eight EXP each, teaches Chaz TSU and Hahn WAT, and
retains both through save/load. Separate tests bound the explicit repair
and verify that a full learned-slot array leaves the entire roster intact
on failure. The full workspace passes 868 tests and strict Clippy.

`build/native-progression/route/receipt.json` completes the corresponding
isolated Godot battle with normal commands, both learned-ability messages,
the correct levels, unchanged spent skill uses and SAVE 2. A fresh process
uses actual title CONTINUE 2, verifies the learned slots, moves one cell
down and writes SAVE 3. Its `continued/validation.json` verifies both source
files remain byte-identical and only standing Y changes in the new payload.
This fixture intentionally starts just below the level thresholds. It is
not connected campaign progress. Exact original results-window wording,
layout and sound timing remain presentation work. A separate held-message
capture run under `held-captures/` confirms both complete messages render;
the shared seven-letter word occupies identical glyph pixels in both images.
