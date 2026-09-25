# PSIV oracle results: continued

This ledger continues [`RESULTS.md`](RESULTS.md) at the next section boundary.

### RETRACTED: the chest/temp flag-bank alias, and the un-looting bug

**Chest flags do not live at `$FFFFF140`. There is no alias, and the un-looting
bug reported here from tape 18 does not exist.** This section used to claim the
opposite; the claim is withdrawn, and the sections below are the corrected
account. Every *measurement* previously reported still stands — only the
inference that two addresses were the same bank was wrong.

The retraction rests on two chests rather than one. A single chest cannot
separate the bit rule from a coincidence, because byte 3 bit 7 is what a dozen
different arithmetics give for a single id. `AcademyBasement` has two chests
with adjacent ids, so the rule predicts adjacent bits in the same byte:

| write | id | predicted | measured |
|---|---|---|---|
| `TempEveFlag_Xanafalgue` | `$13` = 19 | `$F140` + 2, bit 4 | **`$FFFFF142` bit 4**, tape 17 |
| `ChestFlag_PiataMonomate` | `$18` = 24 | `$F120` + 3, bit 7 | **`$FFFFF123` bit 7**, tape 20 |
| `ChestFlag_PiataMeseta` | `$19` = 25 | `$F120` + 3, bit 6 | **`$FFFFF123` bit 6**, tape 21 |

Two chest ids, two neighbouring bits of one byte, both exactly where
`base $F120`, `byte id >> 3`, `mask 1 << (7 - (id & 7))` puts them, while
`$FFFFF143` — where those ids would land if `$F140` were the chest bank — stays
`00` through every one of these tapes. This agrees with core-lane's independent
ROM reading: the chest system's three call sites (`LoadTreasureChests` test at
`$537E0`, `ItemFound` entry test at `$66B2A`, `ItemFound` set at `$66DE0`) all
address the `$F120` door.

So `ps4.constants.asm` mislabels its banks: the door named `Chest_Flags`
(`$F140`) is where the "temp" event flags go, and the door named
`Extended_Event_Flags` (`$F120`) is the real chest bank. A "temp" flag and a
chest flag of the same raw id are in **different** banks and cannot collide.
The Garuberk Tower Moon Slasher chest is not touched by walking through the
Piata basement, at any point, in either direction.

A third measurement makes the identification independent of any single write.
`Event_GameStart` **preloads eleven ids into `$F120-$F13F` at f902**, and the
runtime pack's own `game_start` snapshot for `extended_event_flags` carries the
same thirty-two bytes:

```
0000000001a00880000080140010008000000000010000000000000000000000
```

Decoded with the same rule, those eleven bits are ids `$27 $28 $2A $34 $38 $50
$5B $5D $6B $78 $A7` — and the pack lists exactly those ids (as 295, 296, 298,
… 423, its numbering carrying a +256 bank offset). Eleven-for-eleven, they are
chest ids. A bank that ships pre-set with a list of chest ids, and that takes a
chest's bit when a chest is opened, is the chest bank.

**Integration note for core-lane and pack-lane, filed rather than assumed:** the
pack numbers this bank's ids with a **+256 offset** (`extended_event_flags_set`
holds 295 for `$27`) while the chest call sites use the raw 0-255 id. Whatever
the engine's `Flag::chest` uses must be one consistent space, or the preload and
the chest writes will land in different places. Separately, the manifest counts
**82 map-effect gates on a bank it calls `chest_flags`** — those gates are a
different code path from the three chest call sites, and which door *they* read
is not settled by these tapes. It is worth the same byte-level check core-lane
did for the chest sites.

### Where the "temp" event flags land, on hardware

`docs/source-notes/disassembly-discrepancies.md` proves from ROM bytes that retail has **four** flag banks, not
five: nothing addresses the clone's `Temp_Event_Flags` at `$FFFFF156`, and the
"TempEveFlags" calls dispatch through the door at `$FFFFF140`. That part holds —
it is only the name on that door that was wrong. Tape 17 measures the write.

The prediction comes from `ChestFlags_Set` (`ps4.asm:116683`), whose bit
numbering inside a byte is **reversed**: `bset (7 - (id & 7)), (bank + (id >> 3))`.
So `TempEveFlag_Xanafalgue = $13` = 19 must land at byte `19 >> 3` = 2 and bit
`7 - (19 & 7)` = 4 — that is, **`$10` at `$FFFFF142`**.

Measured, walking a short way into the Piata basement:

| | |
|---|---|
| `chestb2` (`$FFFFF142`) | **`00` → `$10` at f24618**, at cell (42,20) on map $15 |
| `tempb0..3` (`$FFFFF156-9`) | **`00` for all 24,880 frames** — never written |
| `extb3` (`$FFFFF123`) | `00` throughout |
| `FieldRoutine_ItemFound` (`$24`) | never runs |

The write lands exactly where the reversed-bit arithmetic predicts, and the
clone's fifth bank stays dead.

The innocent explanation is ruled out rather than assumed away: no chest was
opened (the ItemFound routine never runs), and the basement's own chests write
`$FFFFF123` — which stayed `00`. Nothing but the flag-$13 write can account for
byte 2 bit 4.

There is **no colliding chest**. An earlier revision of this section named
`ChestFlag_GrbrkTwMoonSlashr` ($13) as sharing this bit and drew a bug from it;
that followed from reading `$F140` as the chest bank, and tapes 20 and 21
disprove it. Chest id `$13` lives at `$FFFFF122` bit 4, in the other bank, and
nothing in the Piata basement writes it.

#### What the flag actually gates, and the flee itself

Map $15 carries exactly **one** object: the Xanafalgue (`object_id` 388),
spawned at cell (44,19). Logging the object columns alongside the flag catches
the whole scripted moment:

- From f24545 the Xanafalgue **runs west** as the party approaches — (43,19),
  (42,19), … (25,19) — at **4 frames per cell**. That is the *fast* step mode:
  the party walks at 8 frames/cell and wandering NPCs at ~32, so the flee is
  twice the party's speed and eight times an idle townsperson's.
- At **f24618 the object despawns** (`obj_id` `$8184` → `0000`) **and
  `$FFFFF142` takes `$10` in the same frame.**

So the flag gates the Xanafalgue's presence, and set-on-despawn is exact —
same frame, no lag.

#### Leaving clears it

Tape 18 routes out of the basement into `PiataAcademyNearBasement` ($12):

| frame | event |
|---|---|
| f24618 | flag sets (`$10`) as the Xanafalgue despawns |
| f25191 | party arrives on map $12, flag still `$10` |
| **f25230** | **`$FFFFF142` → `$00`** — cleared, 39 frames into the new map |

**Retail does clear it, and it clears it in the `$F140` bank.** The clone's
"cleared when you get out" prose is right about the behaviour and wrong only
about which bank the write names.

Set on entering the basement, cleared on leaving: an ordinary scene-scoped temp
flag doing an ordinary temp flag's job. No chest flag is involved in either
direction.

One precision worth keeping: the clear lands **39 frames after arriving on map
$12**, not on leaving $15, so it is tied to loading the destination map rather
than to the exit itself. Whether every map clears id `$13` or only some is
untested.

#### And re-entering respawns it, so the whole thing is repeatable

Continuing the same tape back down the stairs:

| frame | event |
|---|---|
| f25191 | arrive on map $12, flag still `$10` |
| f25230 | flag cleared to `$00` |
| f25823 | back on map $15, flag `$00` |
| **f25857** | **the Xanafalgue is back** — object slot 0 holds id 388 again at its spawn |

So the scene resets completely: **leave, and the flag clears; return, and the
Xanafalgue is there to flee again.** The set/clear cycle can be run as many
times as the player likes.

(The re-entry in the tape is hand-written rather than routed: coming up the
stairs leaves the party standing *on* the stairs cell, and the cartridge's
anti-ping-pong rule will not fire a warp from the cell you were placed on, so
the tape steps off and back on. `python3 -m oracle.scripts.navigate` cannot plan
from a warp cell it is already standing on — noted as a tooling limit, not a
cartridge one.)

For `psiv-core`: model **four** banks, not five and not three. The `$F156` bank
does not exist and must never be written. The `$F140` bank takes the "temp"
event flag API. Chests get their own 256-id bank at `$F120`, which is also the
bank `Event_GameStart` preloads. All of them use the same reversed in-byte
numbering, `bit = 7 - (id & 7)`.

### Camera, and what gates the wander timer

Added for core-lane's camera model: a `camera` group covering
`Camera_X/Y_Pos_FG` ($FFFFEF94/$FFFFEF90) and `_BG` ($FFFFEF9C/$FFFFEF98) as
16.16 longwords plus their high-word pixel views, the four
`Camera_*_Step_Counter_*` longwords ($FFFFEC50-$FFFFEC5C), and
`Map_Row/Column_Size_FG/BG` ($FFFFEC64-$FFFFEC67). Per object, the
`offscreen_flag` byte (+$12) joins the existing `render_flags`. The leader's
`x_step_constant` / `y_step_constant` (+$20/+$24) are in the `pos` group.

Caveat on two of them: **`$FFFFEC24` and `$FFFFEC25` are not named in
`ps4.constants.asm`** — the block runs straight from `Game_Mode_Routine`
($EC20) to `Routine_Exit_Flags` ($EC27). They are logged positionally as
`gate_ec24` / `gate_ec25`; the "plane select" and "FG camera driver enable"
readings are core-lane's own and are not corroborated by the constants file.
On tape 02 both go `00 -> 01` at f1209 and never change again, so on this tape
they are not a dynamic gate.

From the tape-02 re-log, over the 1563 field-control frames:

- **The camera is rigidly locked to the leader.** `leader - camera_FG` is
  `(152, 88)` on *every* field-control frame — exactly one distinct offset, no
  lag, no easing, no deadzone.
- **FG and BG cameras are identical** on all 1563 of those frames. The 679
  frames where they differ are all in the intro cutscene, so there is no
  parallax to model for ordinary field play on this map.
- **Camera step counters take three values only:** 0, `$00020000` (+2.0) and
  `$FFFE0000` (-2.0) — the same 2 px/frame the characters move at.

#### What actually gates an NPC's wander timer

The question was whether slot 2's timer resuming at f7606, with the leader long
stopped, meant a BG-plane object on an independent BG camera, or a step
constant staying hot while walled. **Neither.**

**The wander timer only ticks while the object is at rest.** Slot 2 was
mid-step the whole time: its `x_step_duration` counts 3968 down to 0 across
f7575-7605, and the timer resumes on f7606, the first frame after the step
completes. Across all 32 slots for the whole tape: **1974 timer decrements,
zero of them while `x_step_duration` or `y_step_duration` is non-zero**, and
13101 mid-step frames with the timer frozen.

So the rule is `timer ticks iff xdur == 0 and ydur == 0`. `render_flags` bit 2
("animation finished") is *not* the gate — it sets at f7602, four frames early,
and 270 of the 1974 decrements happen with it clear.

Also from the same trace: **NPC steps are four times slower than the party's.**
The step duration decrements by 128 per frame for objects versus 512 for the
leader, so an NPC takes ~32 frames to cross a cell where the party takes 8.

#### Do the leader's step constants stay hot while blocked?

No. Holding Up into the wall at (50,15):

| frame | `y_step_duration` | `y_step_constant` | `coll_up` |
|---|---|---|---|
| f7578 | 0 (step just landed) | `$FFFE0000` | `00` |
| f7579 | 0 | **`$00000000`** | `08` (wall) |

The constant is stale for exactly **one frame** — the landing frame, before the
next step is evaluated — and is cleared the moment the blocked evaluation runs.
It does not stay hot.

### Collision grid indexing, independently confirmed

`GetChunkAndCollision` adds `$10` to Y before shifting down to a cell, so the
cell a character *occupies* is one row below `curr_y_pos / 16`. The packer
already applies that shift when it emits `y_cell`, and
`rust/psiv-data/src/map.rs` documents it.

This harness measured the same offset from the other direction, against the
game's own `Tile_Collision_Standing` / `_Up` / `_Down` / `_Left` / `_Right`
readouts over 6510 cell-aligned samples: **dx=0, dy=+1 matches 99.86%**, versus
93.2% for the next-best offset and 79.8% for the naive `y_px // 16`. The
convention in the pack is right, and it is now confirmed behaviourally rather
than only by reading the disassembly.

### Field objects block, and block taller than the grid

Two independent confirmations beyond the first report:

- An NPC standing at pixel row R blocks a character trying to enter pixel
  row R+1, so **objects occupy a two-row footprint**, not one cell.
  `python3 -m oracle.scripts.navigate` encodes this as `OBJ_FOOTPRINT`.
- Object blocking is invisible to the terrain grid. Walking west along the
  map-$13 corridor stops dead with `coll_left` reading `00`.

This matters more than it first looks: on map $13 a single NPC (Alys) closes
the only two-row corridor completely, so the terrain grid alone says the map is
traversable when the cartridge says it is not.

### Story gates found by walking into them

Two gates showed up while trying to reach an encounter map on foot, both of
which a route planner working from map data alone would not predict:

- **`NPCAlysPiata`** (map $13 object 7, dialogue 41) is parked in the only
  corridor to the doorway. Talking to her fires event `$03` and
  `Current_Party_Slots` becomes `0100FFFF` at f7478 — **Alys in slot 1, Chaz
  in slot 2**, confirming `Event_AlysFound` and that Alys leads. Tape
  `04_alys_joins.tape`.
- **The Piata town gate is closed.** The gate is the four-cell gap at cols
  30-33 in the wall spanning rows 46-47; guards were observed at cells (31,46)
  and (33,46). The navigator tried all four columns in turn and the cartridge
  refused every one, so this is a hard story gate
  (`Event_PiataGuardsReprimand`), not two NPCs that can be walked around.
  **The Motavia overworld is not reachable on foot at this point in the
  story**, which is why the game's first random encounters are in the Academy
  Basement (encounter group 14) rather than outside town.

### Battle RAM map

Added for the battle lane, all transcribed from the constants file with the
same citation discipline as the rest of the map:

| group | what it covers |
|---|---|
| `battle` | `Battle_Routine` ($FFFF4100), `Battle_Routine_2`, `Battle_Total_Comd_Input`, the cursor indices, `Battle_Priority` ($FFFFEE45: 0 normal / 1 surprise / $FF ambush), `Enemy_Count`, ambush and run chances, item drop rate, and `Battle_Turn_Order` ($FFFFEFB0) |
| `bhit` | `Fighters_Hit_Flags` ($FFFF4150; $00 normal, $01 critical, $FF miss or untargeted) and `Battle_Heal_Damage_List` ($FFFF415A), the per-fighter damage numbers |
| `enemy` | all four `Enemy_Stats` slots ($FFFF4200, $80 stride): id, level, HP/max HP, status, strength, agility and battle agility, dexterity, attack and defence |
| `chars` | Chaz and Alys from `Character_Stats` ($FFFFF500, $80 stride): level, EXP, HP/TP, status, the same stat block, plus `Current_Money` |

`Battle_Routine` values are decoded from `BattleRoutines` (`ps4.asm:7524`):
`$08` ProcessCOMD, `$0A` ProcessMACRO, `$0C` ProcessRUN, `$0E` **OrderTurns**,
`$16` **DoAttackEffect**. `Battle_OrderTurns` builds `Battle_Turn_Order` from
each living fighter's `agility_battle`, four bytes per entry (index word then
agility one byte in), sorted highest first — so turn order versus agility is
readable straight out of the log.

EXP is not accumulated in a battle-local total; `ps4.asm:4775` adds it directly
into each character's `exp` field, so the award is observed as a delta on
`chaz_exp` / `alys_exp`. Meseta accumulates in `$FFFF41D0` and is added to
`Current_Money` at `ps4.asm:4801`.

`python3 -m oracle.scripts.analyze_battle` reads a battle log and reports the
formation and stats, the routine timeline, the turn order, every HP change
with the hit flags, damage list and RNG seed at that frame, and the
EXP/meseta deltas.

### The encounter clock, and why long tapes need tuning

Any tape that walks far inside a dungeon is playing a dice game, and it is
worth stating the odds because they set what tape lengths are practical.
`RunRandomBattles` (`ps4.asm:116840`) counts `$FFFFECE4` down from 10 — reset
on every map entry and after every battle — on each *completed* step, then from
that point rolls on every step: `UpdateRNGSeed`, and a battle if
`RNG_Seed & $1F == 0` (`& $7F` in a vehicle). Flat 1/32 per step, no ramp.

So an N-step route survives with probability `(31/32)^(N-10)`: 25 steps is a
62% bet, 58 steps a 22% one. Tape 21's route is 58 steps, and sweeping its
pre-walk idle over 14 values produced exactly 3 clean runs — 21%, against 22%
predicted. That is a behavioural confirmation of the rule, and incidentally of
`psiv-runtime`'s `GRACE_STEPS = 10` and `FOOT_MASK = 0x1F`, which match.

The dodge is `python3 -m oracle.scripts.sweep_encounter`: idle frames advance
the seed without consuming a step, so varying one `N . <mark>` wait re-rolls
every check downstream while leaving the route byte-identical. It runs the
variants in parallel and reports which values walk clean. Tapes 20 and 21 both
carry a tuned wait, noted as such in their comments — a tape that walks far and
has no tuned wait in it is a tape that has not been re-run since it was written.

### Tape authoring tools

`route.py` plans a walk over the runtime pack's own collision grids and warp
graph, so a generated route that walks correctly on the cartridge is also a
check that the extraction agrees with the cartridge. It models the anti-ping-
pong rule (`GameMode_LoadFieldMap` initialising `Tile_Collision_Standing` to 1,
so a doorway never fires on the frame you are placed on its destination) — the
planner walks straight back through the door it came from without it.

`python3 -m oracle.scripts.navigate` closes the loop, because terrain is not
the whole story: it plans a short leg, runs it, reads where the character
actually ended up, marks observed object footprints and cells the cartridge
refused to enter, and re-plans. **The feedback is at authoring time only** — the
artifact it writes is an ordinary static tape with no runtime feedback in it, so
it still replays byte-identically like every other tape here.

## Still open

Closed since the first draft, and listed here only so the history is legible:
beside-talk (tape 11, hardware evidence), the accept press during the open
animation (tapes 13/15/16 — it turned out to be hold-to-accelerate, not
buffering), and battle ground truth (tapes 07/09/10/12/14).

Genuinely outstanding:

- **No escape-failure sample.** Nine press timings all succeeded, matching the
  ~83% the formula predicts. Team lead's call is not to grind for one: the
  failure branch gets pinned by forced rolls in the Tier 1 engine tests, and a
  hardware sample is a nice-to-have if a future tape happens to catch one.
- **No page-index RAM field.** Tape 16 pins the page *boundary* behaviour
  precisely, but only by watching `Text_Buffer` writes. Nothing in the map
  distinguishes page N from page N+1, so a tape cannot assert "we are on page
  2" directly. Finding that variable would make per-page assertions cheap.
- **`$F9` pause length is uncharacterised.** The 3-frame draw cadence has one
  6-frame gap in it, consistent with an explicit pause control code, but the
  pause's own duration has not been measured against the dialogue data.
- **Scroll-arrow timing.** `Text_Scroll_Arrow` ($FFFFC2C0) is mapped and its
  `offscreen_flag` logged, but that byte read 0 for the whole dialogue, so as
  mapped it does not indicate the arrow's visibility. The right field has not
  been found, and the arrow's art is still unextracted.
- **~~The other five aliased id pairs~~ — closed, no experiment needed.** This
  entry used to queue a Vahal Fort platform tape to demonstrate the chest/temp
  alias bidirectionally. Tapes 20 and 21 showed there is no alias to
  demonstrate: the six "pairs" were an artifact of reading `$F140` as the chest
  bank. `VahFortMovingPltfrm1` and `ChestFlag_PsycoWand` share an id but sit in
  different banks (`$F141` bit 6 and `$F121` bit 6), so riding the platform
  cannot touch the chest. Nothing to route to, nothing to test.

- **Which door the 82 `chest_flags` map-effect gates read.** The chest system's
  three call sites are settled at `$F120`. The manifest counts 82 map-effect
  gates the extractor labels `chest_flags`, and those come from a different code
  path; if that path reads `$F140` while chests write `$F120`, gates that are
  supposed to react to a looted chest never fire. Cheapest resolution is a
  byte-level read of the gate call sites, the way core-lane did for the chest
  ones, not a tape.

- **Camera beyond map $13.** The `(152, 88)` leader lock and the identical
  FG/BG cameras are measured on one interior map. Overworld maps are tori with
  paged scrolling and may well behave differently; the columns are in place, a
  tape is not.

## Feeding a comparator

The intended shape for comparing against `psiv-core`:

1. `psiv-core` gains a replay entry point taking the same tape file and
   emitting the same CSV columns — per frame, its own values for the mapped
   fields.
2. Comparison is a column-wise diff keyed on `frame`. Because the oracle log
   is byte-reproducible, a golden log can be committed as a fixture and the
   core checked against it without an emulator in CI.
3. Not every column is a fair target. `rng_seed` and the raw `game_mode_*`
   dispatch values are cartridge implementation detail; the meaningful
   comparison set is position, facing, step durations, collision, party slots,
   event flags, and window open/close frames. Suggest declaring a per-tape
   column subset in the fixture rather than diffing everything.
4. Frame alignment is exact — both sides count frames from power-on and the
   tape is the same file — so a mismatch is a real behavioural divergence, not
   drift.

The RAM map is deliberately readable from Rust: `ram_map.json` carries the
addresses, sizes and groups, so `psiv-data` can load the same file the oracle
uses instead of re-declaring the field list.
