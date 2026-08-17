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

**This supersedes an earlier reading in this document.** A single press offset
suggested "the press is consumed and produces two extra draws". Sweeping the
offset across the animation (`anim_sweep.py`) and then holding the button
(tape 15) shows what is really happening, and it is simpler.

Speak lands at f7185, `Window_Render_Mode` goes `$0600` at f7187, and the
window opens at f7196. Draws are counted as `Text_Buffer` writes, relative to
the open frame:

| press offset (from Speak) | inside animation | extra draws vs control |
|---|---|---|
| +4 … +7 | yes | **0 — identical to control** |
| +8, +9 | yes | +1 |
| +10, +11 | yes | +2 |
| +12 and later | no (window already open) | +1 … +2 |

And holding the button rather than tapping it, over a 40-frame hold well after
the window is open:

| | draws in the 40-frame span | gap between draws |
|---|---|---|
| control | 13 | 3 frames |
| holding Speak | **39** | **1 frame** |

**The mechanic is hold-to-accelerate, not a buffered page advance.** Text
normally draws one character every 3 frames; **while Speak is held it draws one
character per frame**, a 3x speed-up, for exactly as long as the button is
down. The "extra draws" in the offset table are just a 4-frame hold overlapping
the draw schedule.

Two consequences for the renderer:

- **A press landing in the first half of the open animation (Speak+4 to
  Speak+7) really is dropped** — those offsets reproduce the control draw
  sequence exactly. So "swallowed" is right for that window.
- What we are missing is not press buffering but **hold-to-fast-forward while
  text is drawing**. Implementing a queued page-advance would be wrong.

### Holding Speak across a page boundary

The NPC line in tape 03 is **two pages**. With no input, page 1 draws 58
characters at one per 3 frames and finishes at f7371; the window then sits
indefinitely. Three runs settle what a held button does (tape 16, measured on
`Text_Buffer` writes):

| run | page 1 draws | page 1 cadence | page 2 |
|---|---|---|---|
| no input | 58, ends f7371 | 3 frames | never starts |
| **hold Speak across the boundary** | 58, ends f7257 | **1 frame** | **never starts** — dead stop while still held |
| press after page 1 completes, keep holding | 58, ends f7371 | 3 frames | **55 draws from f7395, 1-frame cadence** |

**Advancing a finished page needs a fresh press; a held button will not do it.**
Holding drew page 1 three times faster and then stopped at the boundary for the
remaining ~1100 frames with the button still down. The conservative guess in the
code comments was right, and is now measured rather than assumed.

**But the next page does start accelerated if the button is still held.** After
a fresh press advances the page, page 2 drew all 55 of its characters at one per
frame while the hold continued.

So the two behaviours are independent and should be implemented as such:

- **page advance is edge-triggered** — a press, not a level;
- **draw acceleration is level-driven** — it applies to whatever page is
  currently drawing, including a page that has just been advanced into.

A single 24-frame gap separates the pages when advanced by a tap (page 1 ends
f7371, the tap lands f7393, page 2's first draw is f7395), so the advance costs
about two frames beyond the press itself.

### Object slots on a chest-bearing map

Tape 02's map has no chests, so the comparator had never exercised the slot
offset chest maps introduce. `AcademyBasement` ($15) carries 1 NPC and 2
chests, which is enough to settle the layout.

Measured at the spawn, before anything moves:

| slot | raw id | masked | cell | offscreen | rflags | timer |
|---|---|---|---|---|---|---|
| 0 | `$8184` | 388 | (44,18) | 1 | `$44` | 2 |
| 1 | `$80A0` | **`$A0`** | (40,8) | 0 | `$4C` | 0 |
| 2 | `$80A0` | **`$A0`** | (14,19) | 1 | `$4C` | 0 |

**NPCs fill the pool first, then chests, each in pack record order.** Slot 0 is
the Xanafalgue (`object_id` 388, `npcs[0]`); slots 1 and 2 are
`treasure_chests[0]` (Dimate, cell (40,8)) and `treasure_chests[1]` (100
meseta, cell (14,19)). So a chest's slot index is `npc_count + chest_index`.

The chest object ids are confirmed from `Interaction_ChkIfTreasureChest`:
**`$A0` = `FieldObj_TreasureChest`, `$1D4` = `FieldObj_WhiteTreasureChest`**.
Both basement chests are the plain kind; no white chest appears on this map, so
`$1D4` is unobserved here.

Two incidental checks fall out. Chests carry **`timer` = 0 permanently**, which
is consistent with the wander rule above — they are exactly the "never wanders"
case, and a core that ticks the RNG for every zero-timer object would
over-consume on every chest map. And `offscreen_flag` tracks visibility
sensibly: the near chest reads 0 and the far one 1 from the spawn.

### Opening a chest

The tape walks to (41,8), turns to face the chest at (40,8), and presses Speak.
(A straight walk west from the spawn does **not** reach it — a wall at column 46
stops the party at (47,8). The route is planned around the chest's own cell,
since objects block.)

The interaction itself is clean and gives core-lane's `open_chest` path its
timing:

| frame | event |
|---|---|
| f24665 | `Game_Mode_Routine` = `$08` (`FieldRoutine_Interaction`) |
| f24666 | `$24` (`FieldRoutine_ItemFound`); `Interaction_Event_Flag` = `$18` (24), `Interaction_Event_Type` = 1 (chest), `Found_Item` = `$7D` (125, Dimate), `Found_Item_Location` = 0 (from a chest) |
| f24675 | window opens and **`Inventory[0]` becomes `$7D`** — the item is granted 9 frames after the routine starts |
| f25069 | window closes on the dismissal press |
| f25080 | back to `FieldRoutine_Controls` |

Every value matches the pack record: `chest_flag` 24 = `ChestFlag_PiataMonomate`
($18), item 125 = Dimate.

**The chest flag is written inline, to `$FFFFF123`.** An earlier revision of
this section reported "no chest flag is written", on the strength of watching
`$FFFFF140-$FFFFF17F` — the bank the constants file calls `Chest_Flags`. That
was the wrong eight bytes. Diffing all 64KB of work RAM before the open against
after a round trip found the survivor at **`$FFFFF123`**, in the bank the clone
labels `Extended_Event_Flags`.

With that address logged, the write is exactly where the code says:
**`$FFFFF123` goes `00` -> `$80` at f24675 — the same frame the Dimate lands in
`Inventory[0]`.** Grant and flag are simultaneous, not deferred to a transition.
Byte 3, bit 7 is flag id 24 under the same `bit = 7 - (id & 7)` rule.

`$FFFFF143` — where id 24 would live if `$F140` were the chest bank — stays
`00` through the whole tape. Both facts are pinned in `verify.sh` so the earlier
mistake cannot recur silently.

**The lesson, worth generalising:** a negative result about RAM is only as good
as the address range it watched. "No write observed" needed a whole-RAM diff to
be trustworthy, and `--dump-ram <frame>:<path>` now exists on the host for
exactly that — dump two frames, diff, find what survives.

#### The chest is open on return, and there is no duplication

Tape 20 opens the chest, presses it again where it stands, then walks out to
`PiataAcademyNearBasement` and back:

| frame | event |
|---|---|
| f24667 | chest object's `facing_dir` 0 -> **4** (opened), one frame into ItemFound |
| f24675 | Dimate to `Inventory[0]` **and** `$FFFFF123` bit 7 set |
| f25573 | **re-press on the same visit**: routine `$08` -> `$24`, ItemFound entered again |
| f25582 | its window opens 9 frames later, as on the first press — but `Inventory[1]` stays `00` and `$FFFFF123` stays `$80` |
| f26381 | warp to map $12 |
| f27157 | back on map $15 |
| f27192 | **slot 1 (the opened chest) spawns with `facing` = 4; slot 2 (the untouched one) spawns `facing` = 0** |

So the chest **stays open** across the round trip, the item is kept, and it
cannot be taken again on either visit. No duplication, and no ledger material
here.

(One authoring trap worth recording, because it silently voided the first cut
of this tape: the re-press opens a window, and an undismissed window pins
`Game_Mode_Routine` at `$24` indefinitely. The party took no further step and
the whole round trip below it never happened, while every position assertion
still passed because the party was standing where it was supposed to end up.
The tape now dismisses that window before walking.)

#### The no-duplicate guard

Two mechanisms, and they are separate:

- **The visible state is the chest object's own `facing_dir` (offset `$6`)**,
  which `ItemFound` overwrites with 4 (`ps4.asm`, `move.w #4, $6(a4)`). It is
  the only chest-slot byte that changes on opening. On reload it comes back as
  4 for the opened chest, so the load path initialises it from the flag.
- **The grant guard is the flag test at `ItemFound`'s entry** (`$66B2A`), not a
  refusal to spawn the chest as interactable. This is measured, and it is the
  one place the two candidate guards give different observable answers:
  re-pressing still reaches routine `$24` and still opens a window, so the
  object is fully interactable — it is the grant inside that is skipped. So
  "entered ItemFound" is never evidence of a grant; the inventory write is.

  The code agrees exactly. `FieldRoutine_ItemFound` (`ps4.asm:137246`) opens
  with `jsr (ChestFlags_Test)` and, on a set bit, branches to `loc_66E0C`,
  which creates a window, renders the string at `loc_2AA70A` and waits for a
  press — a full interaction with no grant in it. That is the second window the
  tape sees.

**And the same routine explains the whole bank question.** Its tail dispatches
on `Interaction_Event_Type` (`ps4.asm:137463-137475`):

| `Interaction_Event_Type` | call | door |
|---|---|---|
| 0 | `EventFlags_Set` | `$F100` |
| 1 | `ChestFlags_Set` | `$F120` |
| else | `TempEveFlags_Set` | `$F140` |

`Interaction_ChkIfTreasureChest` (`ps4.asm:118579`) hardcodes
`move.b #1, (Interaction_Event_Type)` for both chest object ids (`$A0` and
`$1D4`), so a chest always takes the middle row. Three distinct setters, three
distinct banks, chosen by a field the chest path pins to 1 — which is why a
chest flag and a "temp" flag of the same id can never be the same bit.

**For `psiv-core`:** grant and flag write are simultaneous, at routine-start
+ 9 frames. A chest's rendered open/closed state is `facing_dir`, not a
separate field, and it is derived from the flag at map load.

#### The second chest, and what it pays

Tape 21 opens `AcademyBasement`'s other chest, at (14,19), to test the bit rule
on a second id (see the retraction section above for why one chest is not
enough):

| frame | event |
|---|---|
| f25129 | `Game_Mode_Routine` = `$08` |
| f25130 | `$24`; `Found_Item` = 1, **`Found_Item_Type` = 1 (meseta, not an item)**, `Treasure_Addr` = `$FFFFC380` (slot 2) |
| f25131 | chest `facing_dir` 0 -> 4 |
| f25139 | **`$FFFFF123` bit 6 set** and `Current_Money` 600 -> 700 |

Identical shape to the item chest — routine-start +1 to open, +9 to pay — with
`Found_Item_Type` discriminating meseta from inventory. The `Found_Item` value
is a **multiplier of 100**, not an amount and not an item id:
`mulu.w #100, d0 / add.l d0, (Current_Money)` (`ps4.asm:137348-137351`), so this
chest's `1` pays 100.

