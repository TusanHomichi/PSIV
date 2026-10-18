# Battle rules measured on the cartridge

The battle rules are read from the disassembly in [BATTLE_RULES.md](BATTLE_RULES.md);
this document holds what the [oracle](../../oracle/README.md) measured on the
cartridge to confirm them: the RNG's per-frame schedule and the first battles'
turn order, damage, misses, criticals, escape, DEFEND and rewards. Frame numbers
cite marks in the named tape's log, which `oracle/verify.sh` regenerates. The
level-up measurement is [PROGRESSION.md](../camp/PROGRESSION.md).

## RNG

**There is one seed, not two.** `RNG_Seed` ($FFFFEF0C, `constants:2328`) is the
only random state in the game: the whole disassembly contains exactly two
writes to it, one in each of the two update routines. What look like two
generators are two *algorithms* over the same 32-bit word.

- **`UpdateRNGSeed`** (`ps4.asm:86066-86092`) rewrites the whole longword.
  Substitute `$2A6D365B` when the low word is zero, multiply the longword by
  41, then store `(low16 + high16)` of the product in the high word and the
  product's low word in the low word. Transcribed in
  `python3 -m oracle.scripts.analyze_rng`.
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

## Battle ground truth

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

### Turn order versus agility

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
`agility_battle` never changed. The numbers above are the observations; the
distribution is modelled in the RNG rules above.

### Damage events

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

### Second battle (tape 09)

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

### Miss and critical samples

The action-level view
(`python3 -m oracle.scripts.analyze_battle <log> --actions`) enumerates each
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

### Escape

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
without one the battle appears not to end.

**No failure sample yet.** Nine press timings all succeeded, consistent with the
~83% success rate the formula predicts. A failure needs either more timing
variations or a formation with a higher `run_chance`.

### DEFEND, and the physical_prop clobber

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

### Rewards, and the EXP divisor

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

### What the battle menu required

`RunBattleRoutines2` dispatches on `Battle_Routine_2` and reads
`Joypad_Pressed`. `Battle_MainOptions` accepts `ButtonSpeak|ButtonCamp` — **C
or A** — and `Battle_Main_Option_Index` defaults to 0 (COMD). The per-character
command and target cursors default to attack and the first living enemy, so
**mashing C is enough to drive a whole fight**: tape 07 does nothing but pulse
C every 16 frames. `Battle_Total_Comd_Input` counts commands entered and
`Battle_ProcessCOMD` moves to `OrderTurns` once it reaches 5, skipping slots
whose character is dead, paralysed or asleep.

`Battle_Routine` values decoded from `BattleRoutines` (`ps4.asm:7524`) are in
`python3 -m oracle.scripts.analyze_battle`; the observed fight ran init ->
ProcessCOMD (three times, once per living member) -> OrderTurns -> the action
routines -> victory.
