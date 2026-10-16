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

## Measured on the cartridge

Measured on the cartridge with the [oracle](../../oracle/README.md); frame numbers cite marks in
the named tape's log, which `oracle/verify.sh` regenerates.

### Level up, and the equipment stat-lag bug

Tape 10 fights three encounters back to back (2x ZoranBult, then two
3x Xanafalgue groups) so Chaz crosses the 21 EXP that `progression.json` gives
for level 2. EXP runs 0 -> 8 -> 17 -> 26.

The level applies over four frames, and the staggering is the level-up window
revealing one line at a time:

| frame | change |
|---|---|
| f51785 | `exp` 17 -> 26 |
| f51786 | `level` 1 -> 2 |
| f51817 | `strength` 8 -> 9 |
| f51833 | `agility` 7 -> 8, `dexterity` 5 -> 6 |
| f51849 | `max_hp` 25 -> 31, `max_tp` 10 -> 13 |

All of it matches the `progression.json` level-2 record exactly (hp 31, tp 13,
str 9, agi 8, dex 6).

**The stat-lag bug is confirmed as measured ground truth.** Diffing every Chaz
column before the level against 600 frames after, the *only* fields that moved
are `level`, `max_hp`, `max_tp`, `strength`, `agility`, `dexterity`. The
equipment-derived and modified stats did **not** refresh:

- `atk_pow` stayed **18** despite strength going 8 -> 9
- `dfs_pow` stayed 10
- `strength_mod` stayed 8, `agility_mod` 7, `dexterity_mod` 5
- `magic_dfs` stayed 6

So retail really does leave the derived stats stale at level-up until something
else recalculates them. Anything that "fixes" this changes observable numbers
and belongs in the bugfix-policy ledger as a deliberate deviation, not a
silent correction.

### Pinned against the level-up tape

`the_level_up_tape_round_trips_through_the_roster` uses tape 10's measured
numbers (`oracle/logs/verify_levelup.csv`), Chaz's second level-up:

| frame | change |
| --- | --- |
| f51600 | level 1, exp 17, hp 20/25, tp 10/10, str 8 men 6 agi 7 dex 5 |
| f51785 | exp 17 -> 26 (pool 27, three recipients, 9 each) |
| f51786 | level 1 -> 2 |
| f51817 | str 8 -> 9, men 6 -> 7 |
| f51833 | agi 7 -> 8, dex 5 -> 6 |
| f51849 | maxhp 25 -> 31, maxtp 10 -> 13 |
| f51899 | level 2, exp 26, hp **20**/31, tp **10**/13 |

The stat rises are staged over ~64 frames for presentation; only the end state
persists. The load-bearing line is the last one: **a level-up raises the maxima
and does not refill.** Chaz walks out on 20 of 31.

Computing the rise is `battle::apply_level_ups` and is tested there. What this
pins is the half the roster owns — the award arithmetic and what persists.
