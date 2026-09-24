# Replaying the oracle tapes' battles in `psiv-core`

What this ledger records: two basement battles from the oracle's tapes, each
replayed inside `psiv-core` on the exact rolls the cartridge drew, compared
action by action with what the oracle's RAM log shows the cartridge doing.

| tape | battle | replay |
|---|---|---|
| `oracle/tapes/07_first_battle.tape` | frames 24794-30428 | `rust/psiv-core/src/battle/engine_tests_replay_tape07.rs` |
| `oracle/tapes/09_second_battle.tape` | frames 25002-31908 | `rust/psiv-core/src/battle/engine_tests_replay_tape09.rs` |

Both read a fixture `oracle/battle_fixture.py` produced from one oracle run's
RNG trace and RAM log (`replay_fixtures/tape07_first_battle.json`,
`replay_fixtures/tape09_second_battle.json`); the harness they share is
`engine_tests_replay.rs`.

Three claims, and they are different claims:

1. **The port's battle rules are exact on the cartridge's rolls.** Given the
   rolls, every action of both battles resolves identically: the queue, every
   swing's target list, every verdict, every damage number, the HP left behind,
   the deaths, the rewards and the outcome.
2. **The draw counts are the cartridge's too.** Every call the log's frames
   hold, in the order it drew them, is consumed by a consumer the port models -
   no roll filtered out and no call left over. The two divergences the first cut
   of this ledger carried are closed: Alys's and Kyra's second hit pass
   (`loc_B6A2` run again from `AlysKyraAttack_Init`, `ps4.asm:13975-13976`) and
   `Enemy_Attack`'s ability re-roll against `$FFFFEEA8` (`ps4.asm:19146-19151`).
3. **The re-roll word's value is the battle load's.** `$FFFFEEA8` is one word
   per session on the cartridge, and every battle load clears it
   (`GameMode_LoadBattle`'s page wipe, `ps4.asm:9992-9994`), so a battle's first
   ability draw of **zero** costs a second call. The port models exactly that,
   and nothing more: a battle starts the word at zero and owns it for its own
   lifetime. That is the whole of the f29789 divergence in tape 07, and both
   clears are measured on the cartridge below.

## Reproducing it

```sh
./oracle/build_core.sh                       # pinned core + oracle/patches
# verify.sh reads the ROM at the repo-relative path, so a checkout without the
# ROM needs the link; the fixture steps only need --rom.
ln -sfn "/path/to/Phantasy Star IV (USA).md" "Phantasy Star IV (USA).md"
./oracle/verify.sh                           # builds the host, fast lane
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --groups core,battle,bhit,enemy,chars,rng \
    --rng-trace build/tape07_rolls.csv \
    --out  build/tape07_battle.csv
python3 oracle/rng_trace.py check build/tape07_rolls.csv build/tape07_battle.csv
python3 oracle/battle_fixture.py --trace build/tape07_rolls.csv \
    --log build/tape07_battle.csv \
    --out rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json
# Tape 09 is the same three steps with its own tape and window; the extractor
# needs no change for it.
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/09_second_battle.tape \
    --groups core,battle,bhit,enemy,chars,rng \
    --rng-trace build/tape09_rolls.csv \
    --out  build/tape09_battle.csv
python3 oracle/rng_trace.py check build/tape09_rolls.csv build/tape09_battle.csv
python3 oracle/battle_fixture.py --trace build/tape09_rolls.csv \
    --log build/tape09_battle.csv --tape oracle/tapes/09_second_battle.tape \
    --battle-first 25002 --battle-last 31908 \
    --out rust/psiv-core/src/battle/replay_fixtures/tape09_second_battle.json
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 replay
rm "Phantasy Star IV (USA).md"
```

The lane that closed the re-roll ran it in a worktree whose ROM is the project's
own checkout (`/home/peter/PSIV/Phantasy Star IV (USA).md`, sha256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`, the digest
`oracle/verify.sh` insists on) and removed the link afterwards. It built
`oracle/core/genesis_plus_gx_libretro.so` from the pinned commit in its own
worktree (`./oracle/build_core.sh`, patch applied), then built the host with the
same `gcc` line `verify.sh` uses.

## Provenance

| field | tape 07 | tape 09 |
|---|---|---|
| tape | `07_first_battle.tape`, 3145 steps, 36360 frames | `09_second_battle.tape`, 3147 steps, 37760 frames |
| core | Genesis Plus GX `2d7131c5efa606f649d36e1685a8ca47c24f31b3` + `oracle/patches/0001-rng-hv-trace.patch` | same |
| battle window | frames 24794-30428 | frames 25002-31908 |
| trace | 136 calls in 15 frames, f24807-f30306, sha256 `354d3412af750f75ef0fd85f5a871a9f28a65a7f170547c14ac9efdf76324bf0` | 137 calls in 16 frames, f25015-f31786, sha256 `6dff4e323963e2d047ee6894e5e4a81ca3c6945b472b65742b981218171035d0` |
| trace check | `oracle/rng_trace.py check` passes | passes |
| battle roll stream | 134 of the 136; the encounter's formation draw (f24807) and the post-victory item drop draw (f30306) are recorded outside it | 135 of the 137; formation f25015, item drop f31786 |
| start state | frame 24808, the frame the formation was written into RAM | frame 25016 |
| formation | two ZoranBult (enemy id 10) | Xanafalgue (id 9) and ZoranBult (id 10) |
| party | Alys lvl 7, Chaz lvl 1, Hahn lvl 1 | same |

Each fixture carries all of this, plus the log's sha256, as `provenance`.

## The roll the cartridge computes, and the one the trace writes

`UpdateRNGSeed2` is four instructions (ROM `$04239E`, `ps4.asm:86097`):

```text
30 2D 00 08     move.w  $8(a5), d0        ; d0 = VDP HV counter at $C00008
D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
90 78 EF 0C     sub.w   (RNG_Seed).w, d0   ; d0 = the roll the caller gets
E6 F8 EF 0C     ror     (RNG_Seed).w
4E 75           rts
```

`(RNG_Seed).w` is an absolute-short operand at `$FFFFEF0C`, where `RNG_Seed`
is a **longword**. A 68000 word read at that address is the longword's **high**
half, so the roll is

```text
roll = (hv + frame_count - high_word(RNG_Seed)) & $FFFF
```

and the word `ror` rotates is that same one - which is why the trace's
`seed_after` column is right and its `roll` column is not: the host subtracts
the *low* half (`$FFFFEF0E`), a word no instruction reads as the subtrahend.
The two halves differ by a per-frame constant, so every row of the trace is a
per-frame-constant shift of the cartridge's roll. Nothing else in the trace is
affected: `hv`, `frame_count`, `seed_before` and `seed_after` are the values
the emulator produced, and the trace's own seed chain still closes on the RAM
log exactly (that is what `rng_trace.py check` proves, and it is independent of
this defect).

The cartridge settles which half is right. Both derivations were run against
the RAM log's observations, and only one of them reproduces them:

| observation | high half (cartridge) | low half (trace column) |
|---|---|---|
| `Battle_Turn_Order` f29483: Alys 15+5, Chaz 7+4, Hahn 4+2, Enemy1 6+3, Enemy2 6+5 | jitter draws `% 7` = 5, 4, 2, 3, 5 - **all five** | 6, 3, 1, 0, 0 - none |
| `Battle_Turn_Order` f30091: Alys 15+4, Chaz 7+5, Hahn 4+3, Enemy2 6+2 | 4, 5, 3, 2 - **all four** | 4, 1, 2, 6 - one |
| damage f29599: Alys -> Enemy1 12, -> Enemy2 10 | `Battle_CalculateDamage` on the first and second sixteen-call runs gives 12 and 10 - **both** | 9 and 11 |
| damage f29711: Chaz -> Enemy1 15 | 15 | 16 |
| damage f29831: Enemy2 -> Hahn 6 | 6 | 6 (agrees by chance) |
| damage f29926: Hahn -> Enemy2 5 | 5 | 6 |
| damage f30207: Alys -> Enemy2 11 | 11 | 12 |

The jitter is `loc_5528`'s jitter loop: `lsr.w #1, d1` on the queue's highest
agility as the divisor (`ps4.asm:7792`), one `UpdateRNGSeed2` per entry
(`ps4.asm:7796`) and `divu.w d1, d0` (`ps4.asm:7797`) with the remainder added
to that entry's ordering agility (`ps4.asm:7801`). The damages are `loc_266C` ->
`Battle_CalculateDamage` (`ps4.asm:3750`, `ps4.asm:17374`) with the attack,
defence and element factor the log carries (`Character_DamageEnemy`,
`ps4.asm:3910`, `loc_27A4`, `ps4.asm:3963`). Nine turn-order addends and six
damage numbers, all of them reproduced by the high half and none of them by the
low half, is not a coincidence: **the trace's `roll` column is wrong and the
fixtures derive the roll from the row's raw columns instead.**

`oracle/battle_fixture.py` recomputes both conventions for every row and records
which one the file carries (`provenance.roll_column`: `agrees` vs
`subtracts_low_word`), and refuses a trace that carries neither rather than
replaying something nobody can account for. The fix belongs in
`oracle/host/rng_trace.c` (`roll = hv + frame_count - seed_lo` -> `seed_hi`) and
in `oracle/rng_trace.py`'s `roll_for`, which re-derives the same arithmetic;
changing the column would change the traces' sha256, so it stays the first
follow-up below.

## `$FFFFEEA8`: the ability re-roll word, and how long it lives

`Enemy_Attack` picks an enemy's regular ability by drawing an index and
**re-rolling while that index equals the word at `$FFFFEEA8`** (`ps4.asm:19146`,
ROM `$00CFE6`):

```text
4E B9 00 04 23 9E   jsr     (UpdateRNGSeed2).l
02 40 00 07         andi.w  #7, d0
B0 78 EE A8         cmp.w   ($FFFFEEA8).w, d0
67 F0               beq.s   loc_CFE6            ; -16: REROLL while it matches
31 C0 EE A8         move.w  d0, ($FFFFEEA8).w
```

`grep` finds that address written nowhere else in the listing: **the re-roll
loop is the word's only reader and its only writer.**

### Every clear that reaches it

A grep by address cannot see bulk clears, so
`build/lane-evidence/clears_of_ffffeea8.py` walks all 131 `trap #0` sites in
`ps4.asm` and computes each clear's range from its own `lea` / `move.w #N, d7`
pair (`Trap00Exception`, `ps4.asm:161-165`, is `move.l d0,(a0)+ ; dbf d7`, so a
clear covers `d7+1` longwords). Exactly **one** of them reaches the cell:

| clear | site | range | covers `$FFFFEEA8` |
|---|---|---|---|
| `$FFFFEE00` page, `GameMode_LoadBattle` | `ps4.asm:9992-9994` (ROM `$006A38`) | `$FFFFEE00-$FFFFEEFF` | **yes** |
| `Battle_Objects_Memory` | `ps4.asm:9980-9982` | `$FFFFD000-$FFFFDFFF` | no |
| `Enemy_Sprites` | `ps4.asm:9983-9985` | `$FFFFEA00-$FFFFEBFF` | no |
| `Battle_Palette_Objects` | `ps4.asm:9986-9988` | `$FFFF2A90-$FFFF2C8F` | no |
| `$FFFF4000` (skill list) | `ps4.asm:9989-9991` | `$FFFF4000-$FFFF47FF` | no |
| the other 126 sites | `build/lane-evidence/clears_of_ffffeea8.txt` | none reaching `$FFFFEE00-$FFFFEEFF` | no |

Nine of the 131 are not resolvable by that static scan (a base or a count that
comes from a register); they are listed with their lines in the same file, and
each is a window buffer, the item list or a 64-byte actor window:
`Battle_Item_List` `$FFFF4152` (`ps4.asm:1801`, `5659`), `$FFFF8616`
(`ps4.asm:4430`, `6682`), `Plane_A_Buffer` (`ps4.asm:84461`, 1024 longwords),
`Win_Order_Saved_Old_Party` = `$FFFFE200` (`ps4.asm:121901`, `121924`,
`126035`, `126500`). None of them reaches `$FFFFEEA8`.

The two clears outside the `trap #0` family are the boot's, the other edge of
the word's life (`ps4.asm:376-402`):

| clear | site | range | when |
|---|---|---|---|
| last 256 bytes | `ps4.asm:379-382` | `$FFFFFF00-$FFFFFFFF` | cold boot only, guarded by the `"init"` sentinel at `$FFFFFFFC` (`constants:2438`) |
| the rest of RAM | `ps4.asm:391-395` | `$FF0000-$FFFEFF` | every entry to `MainGameProgram_Continue`, which the four "restart the program" paths also jump to (`ps4.asm:87412`, `88653`, `117117`, `159104`) |

Both cover `$FFFFEEA8`. The ROM bytes back the disassembly at both sites, each
exactly once in the image and immediately after the listing's lines for them:
`41 F8 FF 00 3E 3C 00 3F 42 98 51 CF FF FC` (`lea ($FFFFFF00).w,a0 / move.w
#$3F,d7 / clr.l (a0)+ / dbf d7,-`) at ROM `$000362`, and `41 F8 EE 00 3E 3C 00
3F 4E 40` (the battle-load wipe) at `$006A38`.

The save path does not touch it. `TransferToSRAM` writes a fixed block -
`Event_Flags` through `Vehicle_Stats`, `$27F+1` longwords, `ps4.asm:134830-
134849` - and that block starts at `$FFFFF100` (`constants:2362`) and ends at
`$FFFFFAFF`. `$FFFFEEA8` is below it, so the word is neither saved nor restored
and a continue starts from whatever the boot clear left, which is zero. That
reading is from the bytes rather than from a measurement: the tapes here never
load a save.

### What the oracle measures

`enemy_ability_index` was added to `oracle/ram_map.json` (`$FFFFEEA8`, size 2,
group `battle`) and `ram_map.tsv` regenerated with `oracle/gen_ram_map.py`. With
it in the log the word's whole life is visible frame by frame
(`build/lane-evidence/analyze_ability_index.py`; logs in the same directory):

| tape | power-on (f1) | battle start | during the battle | at the battle's end |
|---|---|---|---|---|
| 07 | `0000` | f24794 `0000` | f29789 `0000 -> 0003` | f30428 `0003`, still `0003` at the end of the tape |
| 09 | `0000` | f25002 `0000` | f31044 `0000 -> 0004`, f31381 `0004 -> 0001` | f31908 `0001` |
| 10, battle 1 | `0000` | f24750 `0000` | f26772 `0000 -> 0004`, f26885 `0004 -> 0000` | f27780 `0000` |
| 10, battle 2 | - | f37150 `0000` | f39056 `0000 -> 0007`, f39160 `0007 -> 0003` | f39836 `0003` |
| 10, battle 3 | - | **f50126 `0003`** | f50129 `0003 -> 0000`, f51190 `0000 -> 0001`, f51340 `0001 -> 0004` | f51942 `0004` |

Tape 10 (`10_levelup.tape`, three encounters back to back) is the measurement
that settles the lifetime: battle 2 ends leaving `0003` in the word, the word
**survives the whole field stretch between the battles**, and it is cleared to
`0000` three frames into battle 3's battle-load window - before that battle's
first ability roll, which is what makes the roll compare against zero. Every
other change in the three tapes falls on a frame where an `Enemy_Attack` ability
roll happens, so no other routine in the ROM touches the cell in play.

Each value is the masked draw of the ability call in the same frame: tape 07's
f29789 draws `5320 & 7 = 0`, re-rolls, and stores `58235 & 7 = 3`; tape 09's
f31044 draws `8084 & 7 = 4` and its f31381 draws `19569 & 7 = 1`. The fixtures
hold those raw rolls, so the RAM log and the RNG trace agree digit for digit.

### Three probes that patch the word

The tapes' own logs can only show a zero being compared against a zero. Three
runs with `--ram-patch` (RAM patched, so experiments rather than natural-route
evidence) close that:

| probe | patch | what the log shows |
|---|---|---|
| boot clear | `--ram-patch 1:FFFFEEA8:0005` | `0005` at f1, `0000` at f2: the ROM's boot loop clears the cell during the second frame of the run (`probe_boot_patch1.csv`) |
| field, then the battle load | `--ram-patch 24000:FFFFEEA8:0005` | `0005` from f24000 through the field and the encounter trigger, `0005` still at f24796, `0000` from f24797 - the battle-load wipe, with nothing in the field touching it (`probe_field_patch24000.csv`) |
| the re-roll is conditioned on the word | `--ram-patch 24850:FFFFEEA8:0005` | the enemy's draw of zero at f29789 is **stored**, so the word goes `0005 -> 0000` instead of `0000 -> 0003`, and Hahn takes 8 damage where the unpatched run has 6 (`probe_battle_patch24850_hits.csv`) |
| ... and what it costs | the same patch, with `--rng-trace` | f29789 holds **2** calls instead of 3 and the run holds 135 rolls instead of 136: patching the word removed exactly the re-roll (`probe_battle_patch24850_rolls.csv`) |

The last two are the rule itself, measured on the cartridge: the comparison
against whatever the word holds decides whether the *next* call is a re-roll,
and therefore what every following roll is used for.

### What the port does with it

`psiv-core` owns the whole rule, and the battle owns the word's lifetime:

* `Battle::last_ability_index` is a `u16` that **starts at zero** — the value
  the battle load's wipe leaves — with both clears cited on the field
  (`ps4.asm:9992-9994`, and the boot's `ps4.asm:376-402`). No caller hands a
  battle a word, `Battle::start`/`Battle::start_vehicle` take none, and nothing
  carries one between battles.
* `choose_ability` (`ai.rs`) compares each draw against it and stores the index
  it settles on; `Enemy_Attack`'s dispatcher keeps it for the whole battle.
* `psiv-runtime` has no plumbing for it at all. A session word would be dead
  weight: the cartridge clears the cell before a battle can read it, so the only
  value a load can pass is the zero the battle already starts with.
* Tape 07's replay ends with the word at `3` and tape 09's at `1` — what the RAM
  logs hold at those battles' ends — and both are asserted in the tests;
  `Battle::last_ability_index` exists for those two assertions and for no
  production reader.

## The verdict: both battles, on the verbatim stream

Feeding each round the cartridge's own rolls in the order it drew them, with
nothing removed, no action diverges in either battle: the queue, every swing's
target list, every verdict, every damage, the HP left, the deaths, the rewards
and the outcome. The port also consumes exactly the calls those frames hold -
134 for tape 07's battle (102 in round 1, 31 in round 2, plus the one priority
draw `Battle::start` takes) and 135 for tape 09's (103, 31, plus the priority
draw).

Draw counts per action, tape 07:

| round | action | frame | cartridge drew | the port's consumers |
|---:|---|---:|---:|---|
| 1 | order pass | 29483 | 13 | 9 jitter + 4 target draws |
| 1 | Alys -> both enemies | 29489 | 36 | 4 hit (two passes over both enemies) + 32 damage |
| 1 | Chaz -> Enemy1 | 29644 | 17 | 1 hit + 16 damage |
| 1 | Enemy2 -> Hahn | 29789 | 19 | 1 ability + **1 re-roll** + 1 hit + 16 damage |
| 1 | Hahn -> Enemy2 | 29885 | 17 | 1 hit + 16 damage |
| 2 | order pass | 30091 | 13 | as round 1 |
| 2 | Alys -> Enemy2 | 30097 | 18 | 2 hit (two passes, one enemy left) + 16 damage |

Tape 09, the same terms:

| round | action | frame | cartridge drew | the port's consumers |
|---:|---|---:|---:|---|
| 1 | order pass | 30883 | 13 | 9 jitter + 4 target draws |
| 1 | Alys -> both enemies | 30889 | 36 | 4 hit + 32 damage |
| 1 | Xanafalgue -> Alys | 31044 | 18 | 1 ability + 1 hit + 16 damage |
| 1 | Chaz -> Xanafalgue | 31148 | 17 | 1 hit + 16 damage, the kill |
| 1 | Hahn -> ZoranBult | 31294 | 17 | 1 hit + 16 damage, a **critical** |
| 1 | ZoranBult -> Alys | 31381 | 2 | 1 ability + 1 hit - a **miss** |
| 2 | order pass | 31571 | 13 | as round 1 |
| 2 | Alys -> ZoranBult | 31577 | 18 | 2 hit + 16 damage, the kill |

Values, tape 07: Alys -> Enemy1 12, -> Enemy2 10; Chaz -> Enemy1 15 (kill);
Enemy2 -> Hahn 6; Hahn -> Enemy2 5; round 2 Alys -> Enemy2 11 (kill). Tape 09:
Alys -> Xanafalgue 13, -> ZoranBult 10; Xanafalgue -> Alys 1; Chaz ->
Xanafalgue 18 (kill); Hahn -> ZoranBult 7 (critical); ZoranBult -> Alys, a
miss; round 2 Alys -> ZoranBult 10 (kill). Rewards: 24 experience and 6 meseta
in tape 07, 21 and 5 in tape 09 - the log's own accumulators, with the pool
split over the living members. The comparator checks each damage against
`Battle_Heal_Damage_List` and the target's HP at the end of the action, with the
log's negative HP floored at zero (the cartridge stores -2 and -1 where the port
reports what a player sees).

Three checks inside the tape 07 walk-through are worth naming, because each
could have failed on its own:

- **The opening draw is the cartridge's.** `Battle::start`'s single `loc_B62A`
  draw (`ps4.asm:10043`) on the fixture's f24840 roll yields `Battle_Priority`
  0, which is what the RAM log holds for the whole battle.
- **The queue is the cartridge's.** Sorting the log's own ordering values,
  stable, reproduces the order it recorded them in - including the tie between
  Chaz (7+4) and Enemy2 (6+5) at 11, which the log resolves in Enemy2's favour,
  and tape 09's tie at 8 between Chaz and Hahn, which it resolves in Chaz's.
- **The enemies' stats are the record's.** The fixtures' live RAM values are
  asserted against what `Stats::from_enemy` derives from the pack record, and
  the party's against `Stats::from_character` - the port's fixture records and
  the oracle's RAM agree digit for digit.

## The `$FF` hit flag: a miss and an untargeted slot

`Fighters_Hit_Flags` reads `$FF` both for a slot a swing missed and for a slot
the swing never reached, because `loc_B6A2` blanks all nine flags to `$FF`
before every pass. Tape 07's battle has no misses, so the first cut of this
ledger simply skipped `$FF` slots - and tape 09's battle is what makes that
insufficient. At f31381 the second enemy draws ability index 1 and then a hit
roll of `33419`, and the log leaves Alys's flag at `$FF` with her damage word
unmoved and her HP at 52: a miss.

The comparator now walks the port's own target list instead. Every slot the log
*resolved* must appear there in the log's order, and every other slot the port
swung at must have been resolved as a `Miss` with no damage - a hit where the
log has `$FF` is still a divergence (`Divergence::Value` with `log_hit: 0xFF`),
so the relaxation covers only the ambiguity the byte really has. `hp_after` is
still compared for a missed slot: a miss may not move HP either.

## What this does and does not prove

Proved: on the cartridge's own rolls, the port's battle resolution is exact -
queue, targets, verdicts, damages, HP, deaths, rewards - for two whole battles
in two formations, and the port consumes exactly the calls those battles' frames
hold, the ability re-roll included. The word's lifetime is measured at both of
its edges (boot and battle load) and its effect on the stream is measured by
patching it.

Not proved, and not claimed:

- **The port's rolls are the cartridge's roll *stream*.** They are, for the
  frames these battles occupy: the rolls come from the emulator's HV counter
  reads. Nothing here says anything about frames a trace does not cover, or
  about the substitute a headless port would use where the beam position is not
  available (`docs/RUNTIME_DESIGN.md`, "RNG design").
- **Every command the party issued.** The RAM log carries menu cursors, not the
  chosen commands; every party action in both battles lands damage on an enemy
  slot with no TP or status movement, and the tapes hold C through the command
  phase, so the fixtures record `attack` for each member and say so in
  `provenance.undetermined`.
- **Everything the log cannot see.** A damage word rewritten to the value it
  already held, and the frame an HP write lands on, are recorded per fixture
  under `provenance.undetermined`.
- **Everything the battles do outside the turn engine.** The formation draw and
  the item drop draw sit outside the battle's stream because no battle routine
  consumes them; the port models neither.
- **Behaviour beyond these two battles.** Two ZoranBults, and a Xanafalgue with
  a ZoranBult, a party of three, five and six actions over two rounds. Other
  tapes and other formations are other fixtures.
- **The save/continue path's word.** It is read from the bytes, not measured.

## Follow-ups

1. **Fix the host's roll column** (`oracle/host/rng_trace.c`): subtract
   `seed_hi`, and change `oracle/rng_trace.py`'s `roll_for` to match, since it
   re-derives the same arithmetic. That changes the traces' sha256, which is why
   the column is still there; until then every consumer derives rolls from the
   raw columns the way `oracle/battle_fixture.py` does.
2. **`SOURCE_NOTES.md`'s `$FFFFEEA8` paragraph** (`SOURCE_NOTES.md:1444`) still
   says the ability re-roll moves the divergence "from f29489 to f29789" and
   does not name the clears. The file is over the repo's 1,000-line rule and
   awaiting reorganization, so this lane recorded its notes here instead; that
   paragraph should be corrected in the same pass that splits the file.
3. **Fixture the remaining battle tapes.** `oracle/battle_fixture.py` needs no
   change for a new tape - `--tape`, `--battle-first`, `--battle-last` and the
   two logs are enough. Tape 10's three encounters in a row are the natural next
   one: three more battles on their own seed paths, and a chance at a second
   instance of the case tape 07 carries - a battle whose first ability draw is
   zero, against the word the load left.
