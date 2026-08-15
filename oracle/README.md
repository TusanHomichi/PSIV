# PSIV emulator oracle

The retail cartridge as a scriptable behaviour oracle: deterministic input
tapes in, per-frame named-RAM logs out, for bit-comparison against `psiv-core`
replaying the same inputs.

Built 2026-08-15. Every number in the Results section is re-derived by
`./oracle/verify.sh`, which fails loudly rather than drifting.

## What this runs on, and why it is not BizHawk

**It runs the Genesis Plus GX libretro core inside a purpose-built headless
host** (`host/psiv_oracle.c`, ~700 lines). Same emulation core BizHawk's
Genesis support is built on, no GUI anywhere in the loop.

BizHawk was tried first and was made to work far enough to prove it was the
wrong tool here:

- BizHawk 2.11.1's Linux build is .NET Framework 4.8 targeted and runs under
  **Mono**, which Fedora 44 packages (`mono-core` 6.14.1) but which cannot be
  installed without root on this machine.
- It was nonetheless brought up: the Mono RPMs were unpacked into a user
  prefix and mounted over `/usr` and `/etc` with a `bubblewrap` overlay in a
  user namespace. `mono --version` ran, and EmuHawk got as far as loading its
  own assemblies and initialising WinForms.
- At that point the stack for a *determinism* tool was: extracted RPMs, a
  namespace overlay, Xvfb, Mono, WinForms, a GUI event loop, and Lua driving
  input injection — six layers of scaffolding between the tape and the CPU,
  every one of them a place for a frame to slip.

The libretro host replaces all of it. It has no window, no audio device, no
timer, and no frame pacing; it calls `retro_run()` in a loop and reads
`retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM)` directly. Determinism is
structural rather than configured, RAM access is a raw pointer instead of a
scripting-language bridge, and the whole thing builds with `gcc` and `-ldl`.
The Mono/BizHawk scaffolding was deleted after the experiment; nothing here
depends on it.

The tradeoff is that there is no Lua console for ad-hoc poking. The RAM map
and the tape format are data files instead, which is what the comparator
wants anyway.

## Layout

```
oracle/
├── host/psiv_oracle.c      the headless libretro host
├── host/libretro.h         minimal libretro ABI subset
├── build_core.sh           fetches + builds the pinned emulation core
├── route.py                plans a walking route over the pack's collision data
├── navigate.py             closed-loop tape authoring (route + observe + re-plan)
├── analyze_rng.py          per-frame RNG call census from a log
├── core/…_libretro.so      built core (not committed)
├── gpgx-src/               core source checkout (not committed)
├── ram_map.json            RAM map, source of truth, with disassembly cites
├── gen_ram_map.py          emits ram_map.tsv from the json
├── ram_map.tsv             generated flat map the C host reads
├── tapes/*.tape            input tapes
├── logs/*.csv              output logs (not committed)
└── verify.sh               the verification run
```

### `.gitignore`

Add to the repo's `.gitignore` — the core source and binary are third-party
and large, and the logs are regenerable Sega-derived output:

```
oracle/gpgx-src/
oracle/core/
oracle/logs/
oracle/bin/
```

`host/`, `tapes/`, `ram_map.json`, `ram_map.tsv`, `gen_ram_map.py`,
`build_core.sh` and `verify.sh` are harness code and **are** committed.

## Running it

```sh
./oracle/build_core.sh     # once: clone + build Genesis Plus GX (pinned commit)
./oracle/verify.sh         # build the host, prove determinism, re-derive findings
```

One tape by hand:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/02_walk_timing.tape \
    --groups core,pos,collision \
    --out  oracle/logs/walk.csv
```

Other flags: `--dump-options` lists every option the core declares with its
default; `--probe-endian` prints work-RAM diagnostics.

`--groups` selects which RAM-map groups are logged. Available groups: `core`,
`input`, `pos`, `pos2`, `collision`, `window`, `party`, `flags`, `rng`.
Omitting `--groups` logs all 66 fields.

## Tape format

Line-oriented, one step per line:

```
<frames> <buttons> [mark]
```

- `frames` — how many consecutive frames to hold this state, at least 1.
- `buttons` — `.` for nothing, otherwise letters from `UDLRABCS`
  (up/down/left/right, Genesis A, B, C, Start). `A`, `B` and `C` are the
  physical Genesis buttons; see the button mapping note below.
- `mark` — optional label, recorded in the log on the step's first frame.
  Marks are how findings get cited: every frame number in this document is a
  mark or an offset from one.

A run of steps repeats with a block (blocks do not nest):

```
repeat 336
4 C
12 .
end
```

Blank lines and `#` comments are ignored. **Playback always starts from
power-on**, so a tape plus a pinned core build fully determines the run; there
are no savestates in the loop.

### Button mapping (this bit is a trap)

The disassembly names buttons by *function*, and the mapping to physical
Genesis buttons is not the obvious one:

| Genesis button | joypad bit | disassembly name | field effect |
|---|---|---|---|
| B | 4 (`$10`) | `ButtonCancel` | nothing in field control |
| C | 5 (`$20`) | `ButtonSpeak`  | Talk — `FieldRoutine_Interaction` |
| A | 6 (`$40`) | `ButtonCamp`   | opens the camp menu |
| Start | 7 (`$80`) | `ButtonStart` | settings window |

(`ps4.constants.asm:1877-1892`; confirmed by pressing each in turn and
watching `Game_Mode_Routine` go to 8, 4, and $10 respectively.)

Genesis Plus GX maps libretro `Y`/`B`/`A` onto Mega Drive A/B/C, which the
host handles; tapes are written in Mega Drive letters.

## Log format

CSV with three provenance comment lines, then `frame,mark,buttons` and one
column per enabled RAM field. `frame` is 1-based and counts `retro_run()`
calls from power-on. Values are decimal, or fixed-width hex for fields flagged
`hex` in the map.

## RAM map

`ram_map.json` is the source of truth; **every address is transcribed from
`reference/ps4disasm/ps4.constants.asm` and carries the line it came from**.
Nothing was inferred by watching memory. Struct fields are recorded as base +
offset with both citations so the arithmetic is auditable — for example
`c1_facing` is `Character_1` (`constants:2086`, `$FFFFC000`) plus `facing_dir`
(`constants:107`, `+6`).

`gen_ram_map.py` emits the flat `ram_map.tsv` the C host parses, validating
sizes, alignment and address range on the way. `verify.sh` regenerates and
diffs it, so the two cannot drift apart silently.

### Work-RAM byte order

Genesis Plus GX builds with `-DLSB_FIRST`, which stores `work_ram` in
**host-native 16-bit word order**. The 68000 core reads bytes as
`READ_BYTE(base, addr) == base[addr^1]` and words as
`*(uint16 *)(base + addr)` (`core/macros.h`, `core/m68k/m68kcpu.h:854-882`).
So:

- byte at 68000 address `A` → `ram[A ^ 1]`
- word at even `A` → little-endian `uint16` load at `ram + A`
- long at even `A` → `word(A) << 16 | word(A + 2)`

Get this backwards and every log line is quietly wrong, so `verify.sh` proves
both paths against values the cartridge itself chose (see Results).

## Tapes

| tape | what it proves |
|---|---|
| `01_newgame_to_first_control.tape` | the boot path is reachable deterministically from power-on; new-game starting state |
| `02_walk_timing.tape` | frames per cell, the four facing values, blocked-press behaviour |
| `03_npc_talk.tape` | talk range, dialogue open/close timing, text draw rate |
| `04_alys_joins.tape` | the first story gate: Alys joins and takes slot 1 |
| `05_principal_assignment.tape` | the principal's assignment scene (event `$8001`) |
| `06_basement_quest.tape` | the basement-stair NPCs, event `$0004`, Hahn joins |
| `07_first_battle.tape` | the first random encounter and the fight through to victory |
| `08_rng_characterization.tape` | RNG call counts per frame across title, field, walking and menu |

Tape A's presses are placed from the disassembly, not by trial: `GameMode_Title`
begins at f226; `TitleRoutine_FadingText` takes a START to skip; then
`TitleRoutine_PressStartButton` polls `Joypad_Pressed` for START over a
`$233` = 563 frame window; then, with empty SRAM, `CheckPS4String` fails and
`Title_NoSavedData` opens window 4 and waits for Speak/Camp/Start.

Tape A advances the opening scene with **Cancel (B)**, deliberately. Cancel
advances dialogue pages but does nothing once field control returns, so
presses that overrun the end of the scene cannot perturb the state being
measured. Using Speak here would fire a Talk on the first frame of control.

## Results

Frame numbers below cite marks in the named log; regenerate with `verify.sh`.

### Determinism

Tape A run twice from power-on produces **byte-identical** logs. `verify.sh`
prints the hash on every run and fails if the two differ; the hash itself is
deliberately not pinned in this document, because it changes whenever the RAM
map gains a column and a stale value here would look like a regression.

Core: **Genesis Plus GX v1.7.4**, libretro build, commit
`2d7131c5efa606f649d36e1685a8ca47c24f31b3`, region NTSC.

Core options are pinned explicitly in `host/psiv_oracle.c` and **validated
against the core's declared value list at startup, aborting on a mismatch**.
That guard exists because a wrong value is otherwise silent: Genesis Plus GX
parses its overclock option with `atoi()`, so an invalid `"1x"` became a 1%
clock divisor and produced a log full of plausible-looking zeroes rather than
an error. Anything not pinned falls through to the core's compiled-in default,
which is fixed for a fixed core build; `--dump-options` prints the full set.

### New-game starting state (answers the overworld lane)

**First sustained `FieldRoutine_Controls` frame: 6456** (`logs/01_newgame.csv`,
CSV line 6461).

| | |
|---|---|
| map | `$13` = `MapID_PiataAcademy_F1` (`constants:1086`) |
| position | (768, 288) px = **cell (48, 18)** |
| facing | 0 = DOWN |
| party | `Current_Party_Slots` = `00FFFFFF` — **Chaz alone**, slots 2-5 empty (`$FF`) |
| world | 0 (Motavia) |

The opening runs `Event_GameStart` (`$9F`) across maps `$11` → `$5E` → `$54`
→ `$00` → `$13`, then `Event_PiataChazAlone` (`$A0`) plays unattended for 226
frames (f6230-6455) before handing over.

Character IDs observed during the intro: **Chaz = 0, Alys = 1**. Party slots
pass through `0001FFFF` (Chaz, Alys) and then `0100FFFF` — Alys in slot 1 —
which corroborates the `Event_AlysFound` note in `EVENT_ENGINE_SCOUT.md` that
**Alys leads the opening party**. By first control she has left and Chaz is
alone.

The intro genuinely requires input: with no presses at all it never leaves map
`$11`.

### Walk timing

**Exactly 8.00 frames per 16px cell, all four directions** (`logs/02_walk_timing.csv`).
Position advances 2 px/frame; `x_step_duration` / `y_step_duration` count down
from `$1000` to 0 in steps of `$0200`, which is 8 states. This confirms the
8-frames-per-cell figure `RUNTIME_DESIGN.md` expected.

Facing values confirmed as `constants:107` states: 0 DOWN, 4 UP, 8 RIGHT,
`$C` LEFT.

**Any direction press commits a full 16px step — there is no turn-in-place.**
A 1-frame tap moves a whole cell (`walk_up` leg; a 1-frame LEFT tap at f7185
had already moved 2px on the press frame and completed the cell 8 frames
later). A character only turns without moving when the target cell is blocked.
This is a real constraint on scene authoring and on any "face this way" op.

### Collision

Blocked presses turn the character to face the obstacle and hold position.
Terrain blocking shows up as collision type `08` in the direction field
(`Tile_Collision_*`), matching the solid type in `RUNTIME_DESIGN.md`.

**Field objects block movement independently of the terrain collision grid.**
Walking UP from the start cell stops at (752, 160) with `coll_up` reading
`00` — walkable terrain — because the NPC at (752, 144) is in the way. The
collision array describes terrain only, so `psiv-core` needs a separate
object-occupancy test; a pure grid lookup will let the player walk through
NPCs.

### Talk range: facing only, not beside

**Settled, and it contradicts the remembered behaviour.**
`Interaction_ChkObjects` (`ps4.asm:118729`) builds a probe point from the
player's position projected to its step destination, plus a one-cell offset in
the facing direction from `FieldObj_MovementsTbl`, then accepts an object only
if both `|dx|` and `|dy|` are `<= $80000`. In the 16.16 fixed point these
positions use, `$80000` is **8.0 pixels**. The accept box is therefore 16×16
px — exactly one cell — centred one cell ahead of facing.

An NPC directly beside the player is 16 px off-axis, which fails the test.
**Standing beside an NPC and pressing Speak cannot talk to it.** The
transcription already in `RUNTIME_DESIGN.md` ("projects one cell ahead of the
party's facing (±8px)") is correct as written; the recollection of beside-talk
is not supported by the cartridge.

Empirically confirmed on the facing side: Speak while facing an adjacent NPC
opens the dialogue (`logs/03_npc_talk.csv`, mark `speak`). A fully clean
beside-adjacent-but-not-facing press could not be staged on this map, because
committing a full step on every direction press makes it hard to turn while
staying adjacent — see Still open.

### Dialogue timing

From `logs/03_npc_talk.csv`:

- Speak pressed f7185 → `Game_Mode_Routine` = 8 (`FieldRoutine_Interaction`).
- f7187 `Window_Render_Mode` = `$0600` — the open animation starts.
- f7196 `Windows_Opened_Num` = 1, render mode back to 0. **Window open takes
  9 frames; 11 frames from press to open.**
- The window then stays open with no input for the full 300-frame observation
  window. **The final page does not auto-close; it requires a press.**
- A press closes it over 9 frames (f7793 → f7802) and control returns at
  f7803.

**Text draws progressively at one character every 3 frames.** Sampling
`Win_Tile_Buffer` (`$FFFFE220`, `constants:2106`) every frame during the
dialogue shows writes at f7197, 7200, 7203, 7206, 7209, … in a strict 3-frame
cadence, with occasional 6-frame gaps (consistent with `$F9` pauses or line
breaks). At 60 Hz that is **20 characters per second**.

This contradicts the current implementation: `RUNTIME_DESIGN.md` records
dialogue text as instant, with `$F9` treated as the only pause. Retail is a
3-frame-per-character typewriter. The cartridge outranks the doc.

### RNG

**There is one seed, not two.** `RNG_Seed` ($FFFFEF0C, `constants:2328`) is the
only random state in the game: the whole disassembly contains exactly two
writes to it, one in each of the two update routines. What look like two
generators are two *algorithms* over the same 32-bit word.

- **`UpdateRNGSeed`** (`ps4.asm:86066-86092`) rewrites the whole longword.
  Substitute `$2A6D365B` when the low word is zero, multiply the longword by
  41, then store `(low16 + high16)` of the product in the high word and the
  product's low word in the low word. Transcribed in `analyze_rng.py`.
- **`UpdateRNGSeed2`** (`ps4.asm:86098-86102`) is not a generator at all in the
  usual sense: it `ror`s the *high word* by one bit and returns a value mixed
  from `Main_Frame_Count` and the pre-rotation seed.

Because the first algorithm is exact, a log can be turned into a census: predict
`UpdateRNGSeed` forward and count how many applications reach the next frame's
value. That count is how many times the game called the generator that frame.
Over `logs/06_rng.csv`, **every one of the 8037 frame transitions is explained
exactly** — which is itself the proof that the transcription above is what the
cartridge runs.

| phase | RNG calls per frame |
|---|---|
| title / attract | 1 (0 during fades, when the vblank handler does not run) |
| intro cutscene | 1 |
| **field control, idle** | **2** (291/300 frames), 3 on 9 frames |
| **field control, walking** | **2** (92/96 frames), 3 on 4 frames |
| **camp menu idle** | **1** on all 300 frames, no exceptions |

Reading the table:

- The baseline is **one call per frame from the vblank handler**, everywhere.
- **Field mode adds exactly one more call per frame**, with an occasional third.
- **Walking does not advance the seed beyond that baseline.** Idle and walking
  are statistically identical, so the answer to "does walking advance it" is:
  only in the sense that being in field mode at all does. Steps are not the
  clock; frames are.
- **Opening the camp menu drops back to one call per frame**, which says the
  second field-mode call belongs to per-frame field-object updating (NPC
  wander), suspended while a menu is up.

The practical consequence for `psiv-core`: the seed is a **free-running
per-frame counter**, not a stream advanced by consumers. Anything that wants
bit-exact RNG parity has to advance it on the same frame schedule, not on
gameplay events. A core that only ticks the RNG when it rolls something will
desync immediately.

### Battle ground truth

Tape 07 walks the whole opening act from power-on and fights the first random
encounter in the Academy Basement. Battle at **f24794-30428**, `Battle_Priority`
= 0 (normal, no ambush or preemptive strike).

**Formation: two ZoranBult (enemy id 10).** Every stat in RAM matches
`generated/enemies.json` exactly:

| stat | RAM | enemies.json |
|---|---|---|
| HP | 25 | 25 |
| attack | 16 | 16 |
| defence | 2 | 2 |
| agility | 6 | 6 |
| strength | 18 | 18 |
| mental | 4 | 4 |
| dexterity | 8 | 8 |

One wrinkle worth knowing: **enemies leave the base stat bytes at zero and
carry their live values in the `_battle` variants** (`strength_battle` `$1A`,
`mental_battle` `$1D`, `dexterity_battle` `$23`, `agility_battle` `$20`).
Reading `strength` `$18` off an enemy gets 0, and `level` `$08` is not a level
at all. Party members populate both.

Party: Alys lvl 7 (HP 53, str 12, dex 13, atk 13, agi 15), Chaz lvl 1 (HP 25,
str 8, dex 5, atk 18, agi 7), Hahn lvl 1 (HP 21, str 6, dex 5, atk 8, agi 4).

#### Turn order versus agility

`Battle_Turn_Order` ($FFFFEFB0) is rebuilt each round, four bytes per entry —
a fighter index word then the ordering agility in the next word's low byte,
sorted highest first. Fighter index is slot + 1, so 1-3 are the party and 6-7
the enemies.

| round | frame | ordering values | stored `agility_battle` |
|---|---|---|---|
| 1 | f29483 | Alys 20, Chaz 11, Enemy2 11, Enemy1 9, Hahn 6 | 15, 7, 6, 6, 4 |
| 2 | f30091 | Alys 19, Chaz 12, Enemy2 8, Hahn 7 | 15, 7, 6, 4 |

**The ordering value is `agility_battle` plus a per-round random addend**, not
the raw stat: the addends were +5, +4, +5, +3, +2 in round one and +4, +5, +2,
+3 in round two, all in the range +2..+5 across nine samples. The stored
`agility_battle` never changed. Fitting the exact distribution is the design
session's job; the numbers above are the observations.

#### Damage events

Every HP change, with the attacker taken from the acting-fighter index
($FFFF4142, read by `Battle_DoAttackEffect`) and the RNG seed at that frame:

| frame | attacker | defender | damage | RNG seed |
|---|---|---|---|---|
| 29640 | Alys | Enemy1 | 12 | `037A83E3` |
| 29640 | Alys | Enemy2 | 10 | `037A83E3` |
| 29752 | Chaz | Enemy1 | 15 (kill) | `392D5BAB` |
| 29872 | Enemy2 | Hahn | 6 | `30B40CEB` |
| 29967 | Hahn | Enemy2 | 5 | `E4D042BB` |
| 30248 | Alys | Enemy2 | 11 (kill) | `F373A2CB` |

Notes for formula fitting: **Alys's normal attack hit both enemies in the same
frame** (f29640, `Fighters_Hit_Flags` slots 5 and 6 both `$00`), so her weapon
is multi-target and a single-target damage model will not fit her rows.
`Fighters_Hit_Flags` is nine bytes from `$FFFF4150`, indexed by fighter slot
(0-4 party, 5-8 enemies): `$00` normal hit, `$01` critical, `$FF` not targeted
or missed. `Battle_Heal_Damage_List` ($FFFF415A) is one word per fighter slot
with the same indexing. Enemy HP goes **signed negative** on death (-2 and -1
above), so a comparator reading it unsigned sees 65534/65535.

#### Rewards, and the EXP divisor

`ps4.asm:4772-4779` reads the battle EXP accumulator, halves it if the party is
in a vehicle, then does `divu.w d2, d1` where `d2` is the count of **living**
party members, and adds the quotient to each living member.

Observed exactly: `battle_exp_total` ($FFFF41CE) steps `0 -> 12` when the first
ZoranBult dies and `12 -> 24` when the second does, matching `experience_reward`
= 12 per enemy. The party is three by this point (Hahn joins in the basement
quest), so each of Chaz, Alys and Hahn gained **+8 = 24 / 3**. Meseta went
600 -> 606, **+6 = 2 x `meseta_reward` 3**.

So both reward fields in `generated/enemies.json` are confirmed against the
cartridge, and the EXP split is per living member, not per party slot.

#### What the battle menu required

`RunBattleRoutines2` dispatches on `Battle_Routine_2` and reads
`Joypad_Pressed`. `Battle_MainOptions` accepts `ButtonSpeak|ButtonCamp` — **C
or A** — and `Battle_Main_Option_Index` defaults to 0 (COMD). The per-character
command and target cursors default to attack and the first living enemy, so
**mashing C is enough to drive a whole fight**: tape 07 does nothing but pulse
C every 16 frames. `Battle_Total_Comd_Input` counts commands entered and
`Battle_ProcessCOMD` moves to `OrderTurns` once it reaches 5, skipping slots
whose character is dead, paralysed or asleep.

`Battle_Routine` values decoded from `BattleRoutines` (`ps4.asm:7524`) are in
`analyze_battle.py`; the observed fight ran init -> ProcessCOMD (three times,
once per living member) -> OrderTurns -> the action routines -> victory.

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
  `navigate.py` encodes this as `OBJ_FOOTPRINT`.
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

`analyze_battle.py` reads a battle log and reports the formation and stats, the
routine timeline, the turn order, every HP change with the hit flags, damage
list and RNG seed at that frame, and the EXP/meseta deltas.

### Tape authoring tools

`route.py` plans a walk over the runtime pack's own collision grids and warp
graph, so a generated route that walks correctly on the cartridge is also a
check that the extraction agrees with the cartridge. It models the anti-ping-
pong rule (`GameMode_LoadFieldMap` initialising `Tile_Collision_Standing` to 1,
so a doorway never fires on the frame you are placed on its destination) — the
planner walks straight back through the door it came from without it.

`navigate.py` closes the loop, because terrain is not the whole story: it plans
a short leg, runs it, reads where the character actually ended up, marks
observed object footprints and cells the cartridge refused to enter, and
re-plans. **The feedback is at authoring time only** — the artifact it writes is
an ordinary static tape with no runtime feedback in it, so it still replays
byte-identically like every other tape here.

## Still open

- **Beside-talk empirical confirmation.** Answered from code above with a
  citation, and the code is unambiguous, but no tape yet stages the exact
  adjacent-not-facing press. It needs a map spot where the cell the player
  would turn toward is blocked while an NPC sits perpendicular. Worth one
  targeted tape to close the loop.
- **Accept press during the window-open animation.** Untested. The 9-frame
  open animation (f7187-7196) is now a known window, so the tape is easy: press
  Speak inside it and see whether the page advances immediately on completion
  (buffered) or not (dropped).
- **Multi-page dialogue boundaries.** The talk in tape C spans two pages, but
  no logged field distinguishes page N from page N+1 — the first advance press
  changed nothing in the RAM map. Finding the page-index variable would let
  tapes assert per-page behaviour.
- **Scroll-arrow art.** `Text_Scroll_Arrow` (`$FFFFC2C0`) is in the map and its
  `offscreen_flag` is logged, but the arrow's on/off timing was not
  characterised.
- **Battle ground truth.** The RAM map, the log analyser (`analyze_battle.py`)
  and the encounter-hunting tape generator (`find_battle.py`) are in place, but
  no battle log has been captured yet. Random encounters need an encounter map,
  and the route to one runs through the opening act: the Piata town gate is
  shut (see Story gates), so the nearest encounter map is the Academy Basement,
  reached via Alys joining, the principal's assignment, and the NPCs currently
  blocking the basement stairs. Tapes 04 and 05 cover the first two.

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
