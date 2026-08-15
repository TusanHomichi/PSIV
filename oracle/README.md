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
| `09_second_battle.tape` | a second encounter on a different seed path, including a critical hit |
| `10_levelup.tape` | three encounters back to back; Chaz reaches level 2 |
| `11_beside_press.tape` | Speak at an NPC that is adjacent but not in front |
| `12_escape.tape` | a successful escape attempt |
| `13_open_anim_press.tape` | Speak pressed inside the window-open animation |
| `14_defend.tape` | the DEFEND command and its `physical_prop` clobber |

`prelude_basement.tape` is a generated intermediate (`navigate.py` output) that
both battle tapes are built from; `find_battle.py` consumes it.

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

#### Second battle (tape 09)

A different patrol produces a different seed path and a different formation:
**Xanafalgue (id 9) + ZoranBult (id 10)**, battle f25002-31908. Both records
check out against `generated/enemies.json` again — Xanafalgue hp 16, attack 13,
defence 0, agility 5.

| frame | attacker | defender | damage | note | RNG seed |
|---|---|---|---|---|---|
| 31040 | Alys | Xanafalgue | 13 | multi-target | `F97FD423` |
| 31040 | Alys | ZoranBult | 10 | same frame | `F97FD423` |
| 31135 | Xanafalgue | Alys | 1 | atk 13 vs dfs 18 | `F3FAAC6B` |
| 31257 | Chaz | Xanafalgue | 18 (kill) | | `5AE02DAB` |
| 31377 | Hahn | ZoranBult | 7 | **critical** (`hit_06 = $01`) | `3B14E3BB` |
| 31728 | Alys | ZoranBult | 10 (kill) | | `6CBAB75B` |

Rewards: `battle_exp_total` steps `0 -> 9 -> 21` (9 + 12, matching both
records), **21 / 3 = 7 each**, meseta **+5 = 2 + 3**. The divisor rule from
tape 07 reproduces on a different formation, and the run supplies the first
**critical hit** sample for the damage formula.

#### Level up, and the equipment stat-lag bug

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

#### Miss and critical samples

The action-level view (`analyze_battle.py <log> --actions`) enumerates each
acting fighter and then checks whether any HP moved, which is what makes a miss
visible at all — a miss produces no HP change and is invisible to HP-change
scanning.

- **Miss, party side:** tape 10, f51294. Hahn (dexterity 5) attacks and
  `Fighters_Hit_Flags` reads `FF` across the board with no HP change.
- **Miss, enemy side:** tape 10, f51340. Enemy3 attacks, same signature.
- **Critical, party side:** tape 10, f38908. Chaz hits Enemy1 for 22 with
  `hit_05 = $01`. Tape 09 has a second at f31377 (Hahn, 7 damage).

`Fighters_Hit_Flags` is only meaningful **at the frame an action resolves** —
it persists between actions, so scanning it over a whole battle reports
hundreds of spurious criticals.

#### Escape

Tape 12. `Battle_ProcessRUN` calls `Battle_CalculateChances` with the party's
highest agility against `Enemy_Run_Chance`, `d3 = 2`, lower bound `$28`:

```
roll  = UpdateRNGSeed2() & $3F          ; 0..63
score = (roll + highest_agility - run_chance) * 2
score <= $28  ->  escape fails
```

The basement formations carry `run_chance = 5` (well under the `$F0` that makes
escape impossible), and Alys's agility is 15, so escape needs `roll > 10` —
about 53 in 64, and every one of nine timing variations tried succeeded.

Observed on the successful run: cursor 0 -> 1 -> 2, Speak at f30809 sets
`Battle_Routine_2 = 3`, `Battle_Routine` becomes `$0C` (`Battle_ProcessRUN`) at
f30813, the message runs `$23` -> `$24` -> `$2F`, and the game leaves battle
mode at f31727 with no rewards. **The escape message needs a press to dismiss** —
without one the battle appears not to end, which cost me a wrong reading first
time round.

**No failure sample yet.** Nine press timings all succeeded, consistent with the
~83% success rate the formula predicts. A failure needs either more timing
variations or a formation with a higher `run_chance`.

#### DEFEND, and the physical_prop clobber

The per-character command menu is **horizontal**, which is why Down presses do
nothing to it — `Win_UpdateCursorUpDown` drives the *main* menu only. Right
steps the cursor and it wraps after five entries:

| Right presses | `Battle_Char_Comd_Index` | `Battle_Command_Data` byte 1 |
|---|---|---|
| 0 | 0 | 1 attack |
| 1 | 1 | 2 technique |
| 2 | 2 | 3 skill |
| 3 | 3 | 4 item |
| **4** | **4** | **5 defend** |
| 5 | 0 | wraps to attack |

Tape 14 selects DEFEND for the party leader (Alys) and watches the property
bytes:

| frame | `alys_phys_prop` | `alys_phys_prop_save` | chaz / hahn |
|---|---|---|---|
| f30641 (before) | `$0202` | 0 | `$0202` |
| f31391 (defending) | **`$0102`** | 0 | `$0202` |
| f31707 (round over) | `$0202` | 0 | `$0202` |
| f31871 (defending again) | **`$0102`** | 0 | `$0202` |

**The clobber is confirmed.** Defending rewrites the high byte of
`physical_prop` from 2 (normal) to 1 (resistant) — and
**`physical_prop_save` is never written; it stays 0 throughout.** That field
exists only for the fork's bugfix (constants:61 says so outright), so retail
overwrites the property with nothing saved. Only the defender's property
changes; the other two party members are untouched, and `dfs_pow` /
`dfs_pow_battle` do not move at all — defend is a damage-class change, not a
defence-power change.

The practical consequence: any implementation that stores an armour-derived
`physical_prop` and lets Defend overwrite it will lose the armour setting the
way retail does. Reproducing that is fidelity; restoring from a save slot is a
deliberate deviation and belongs in the bugfix ledger.

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

### Field object columns, and the wander RNG rule

`Field_RunObjects` (`ps4.asm:89478`) walks **64 slots of `$40`** from
`Field_Objects_Memory` ($FFFFC000), dispatching each through
`FieldObjectsJmpTbl` on `obj_id & $7FFC`. Slots 0-4 are `Character_1..5`,
slot 10 is `Red_Cursor`, slot 11 is `Text_Scroll_Arrow`, and slots 12-63 are
the 52 `Field_Obj_Secondary` entries ($FFFFC300) — the same 52
`Interaction_ChkObjects` scans.

The `objects` group logs **32 secondary slots** (the densest retail map, Jut,
has 31 NPCs), eleven columns each: `id`, `rflags`, `facing`, `map_idx`,
`timer`, `xdur`, `ydur`, `x_px`, `y_px`, `xbnd`, `ybnd`. Tapes 02, 03 and 08
are logged with it.

**Slot-to-record mapping is positional and exact.** `LoadMapObjects` fills the
secondary slots in map-record order, so **secondary slot `i` is the pack's
`npcs[i]`** — verified on map $13 at the spawn frame (f6175) for all 8 objects,
matching pixel position, facing and object id. The comparator can join on the
index; no spawn-cell join is needed. One gotcha: **RAM `obj_id` is
`$8000 | pack object_id`** (mask with `$7FFF` to compare), and positions drift
within a frame or two of spawn as wander starts, so any comparison against pack
values has to be taken at the spawn frame.

#### The third field-mode RNG call is a wander decision

Correlating the RNG census against the new object columns over the 1154
field-control frames of tape 08:

| RNG calls that frame | wandering objects with `timer == 0` | frames |
|---|---|---|
| 2 | 0 | 1122 |
| 3 | 1 | 32 |

**The rule holds on 100% of frames:**

```
calls per frame = 1 (vblank)
                + 1 (field-mode object update)
                + 1 per wandering object whose timer is 0 this frame
```

The reloaded timer values are 1..63 across 31 samples, consistent with a
6-bit mask. A decision can occupy two consecutive frames — slot 0's timer sat
at 0 for f6871 and f6872 before reloading to 24 at f6873, consuming a call on
each — which is why the rule is keyed on "timer is 0", not on the reload edge.

**Object type decides whether a slot wanders at all.** 7 of map $13's 8 objects
wander; slot 7 (`obj_id` 104, `NPCAlysPiata`) holds `timer == 0` permanently
and consumes nothing, so a core that ticks the RNG for every zero-timer object
will over-consume. Whether a slot wanders is a property of its `obj_id`
routine, and the harness identifies it behaviourally as "its timer is ever
non-zero".

For core-lane: this is the portable part. The generator is the per-frame
`UpdateRNGSeed`, and wander rides it by consuming extra calls on decision
frames — so reclaiming tape 02's divergent tail needs the decision *schedule*
to match, not just the generator.

### A window opening is not proof of a talk

`Interaction_ContinueChecks` ends with
`beq.w Interaction_DoPlayerNothingMsg` — **pressing Speak at nothing opens a
window too**, carrying the player's "nothing here" line. `Windows_Opened_Num`
going 0 to 1 therefore proves only that *some* window opened.

That matters for anything reported off this harness: an earlier round used
`Windows_Opened_Num` as the talk indicator. For a *positive* test it happens to
be safe (the facing press really did open NPC dialogue), but it cannot support
a negative test, and it briefly produced a reading that looked like beside-talk
working. **The sound discriminator is `Text_Buffer` ($FFFF7000):** capture a
Speak in open ground as the "nothing here" reference and compare the bytes.

### Beside-press: disproven on hardware

Tape 11. The leader parks at cell (17,7) facing RIGHT, held there by the wall
at (18,7), with a map $13 NPC at rest at (16,7) — orthogonally adjacent,
directly behind the facing. An earlier Speak in open corridor supplies the
reference message.

**The beside press draws byte-identical text to the open-corridor press.** It
took the "nothing here" path; the adjacent NPC was never reached. Combined with
the facing press opening real dialogue, **talk range is facing-only, and Peter's
recollection is disproven with hardware evidence rather than only by reading
the code.**

Getting there needed three attempts, and the reason is worth recording: **the
NPCs wander, so a test staged from pack spawn cells is invalid by the time the
leader arrives.** The tape's wait is therefore *measured* — the run is first
made with the leader parked, the log scanned for a frame where an at-rest
object is orthogonally adjacent but outside the faced cell, and the press
placed at that offset. Two earlier attempts failed silently because the NPC had
moved into the faced cell, turning the "beside" test into a facing test.

### Accept press during the window-open animation

Tape 13 against a control with the press removed, compared on `Text_Buffer`
writes:

| | text-draw frames after the window opens |
|---|---|
| control (no press) | 7197, 7200, 7203, 7206, 7209, … |
| press at f7191 (inside the animation) | 7197, **7198, 7199**, 7202, 7205, … |

**The press is consumed, not swallowed.** It produces two extra character draws
within three frames of the window opening and shifts the whole subsequent draw
cadence by one frame. It does *not* skip the page.

So the retail behaviour is neither "buffered as a page advance" nor "dropped" —
the press reaches the text-draw stage and advances it. Our renderer swallowing
it is a divergence, but the fix is not a queued page-advance either; it is the
draw acceleration. Worth one more tape to characterise the acceleration
properly before implementing.

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
