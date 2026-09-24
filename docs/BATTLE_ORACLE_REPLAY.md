# Replaying the oracle tapes' battles in `psiv-core`

What this ledger records: two basement battles from the oracle's tapes, each
replayed inside `psiv-core` on the exact rolls the cartridge drew, compared
action by action with what the oracle's RAM log shows the cartridge doing.

| tape | battle | replay |
|---|---|---|
| `oracle/tapes/07_first_battle.tape` | frames 24794-30428 | `rust/psiv-core/src/battle/engine_tests_replay_tape07.rs` |
| `oracle/tapes/09_second_battle.tape` | frames 25002-31908 | `rust/psiv-core/src/battle/engine_tests_replay_tape09.rs` |

Each tape replays a fixture [`oracle/battle_fixture.py`](../oracle/battle_fixture.py)
extracted from one oracle run's RNG trace and RAM log
([`tape07_first_battle.json`](../rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json)
and
[`tape09_second_battle.json`](../rust/psiv-core/src/battle/replay_fixtures/tape09_second_battle.json));
the harness the two share is
[`engine_tests_replay.rs`](../rust/psiv-core/src/battle/engine_tests_replay.rs).
The capture has its own account in
[`oracle/README.md`](../oracle/README.md#rng-trace-the-cartridges-own-battle-rolls),
and the disassembly notes `SOURCE_NOTES.md` refers to are the topic files under
[`docs/source-notes/`](source-notes/).

Three claims, and they are different claims:

1. **The port's battle rules are exact on the cartridge's rolls.** Given the
   rolls, every action of both battles resolves identically: the queue, every
   swing's target list, every verdict, every damage number, the HP left behind,
   the deaths, the rewards and the outcome.
2. **The draw counts are the cartridge's too.** Every call the log's frames
   hold, in the order it drew them, is consumed by a consumer the port models -
   no roll filtered out and no call left over. Both draw-count divergences an
   earlier cut of this ledger carried are closed; each is recorded below with
   its citations and its negative control.
3. **The re-roll word's value is the battle load's.** `$FFFFEEA8` is one word
   per session on the cartridge, and every battle load clears it
   (`GameMode_LoadBattle`'s page wipe, `ps4.asm:9992-9994`), so a battle's first
   ability draw of **zero** costs a second call. The port models exactly that,
   and nothing more: a battle starts the word at zero and owns it for its own
   lifetime.

## Method: a captured-roll replay

The oracle replays a tape on the pinned core and logs the cartridge's work RAM
frame by frame (`oracle/README.md`). `oracle/battle_fixture.py` turns one such
run into a fixture: the battle's start state - the frame the formation was
written into RAM - the rolls in the order the cartridge drew them, each with
the frame and the role it played, the commands the party issued, and what the
log shows every action doing. Every number is transcribed from the two logs;
nothing is fitted to `psiv-core`, so a replay that disagrees with a fixture is a
finding about the port.

`rust/psiv-core/src/battle/replay/` builds the battle that a fixture
describes, checks the logged live state against the port's own derivations of it
(the party against `Stats::from_character`, the enemies against
`Stats::from_enemy`, both reaching the same numbers the RAM does), feeds the
rolls through `SliceRolls` and compares the port's timeline with the log's
action by action. `replay/data.rs` is the one data-driven test: it replays
**every** fixture in `replay_fixtures/` - these two, and the forced captures of
[`BATTLE_ORACLE_FORCED.md`](BATTLE_ORACLE_FORCED.md) - and holds each one
against `replay_fixtures/divergences.json`, which carries the first divergence
of every fixture that does not replay exactly. A fixture that diverges anywhere
else fails the test, and so does an entry whose fixture replays exactly, so the
manifest cannot go stale. Two further tests per tape walk them in more detail:
`tape07_replays_the_cartridges_battle_on_the_verbatim_stream` /
`tape09_replays_the_cartridges_battle_on_the_verbatim_stream` (the whole battle,
asserting per round that nothing diverges **and** that the port drew exactly the
rolls those frames hold) and `every_roll_the_frames_hold_is_one_the_port_consumes`
/ `every_roll_tape09s_frames_hold_is_one_the_port_consumes` (the per-action draw
accounting, both directions). Tape 07's module walks round 1 in more detail in
`tape07_orders_its_rounds_the_way_the_cartridge_did`.

Reproducing both captures, and the replay, from a checkout with the ROM linked
in:

```sh
ROM="/home/peter/PSIV/Phantasy Star IV (USA).md"   # this project's own checkout
./oracle/build_core.sh                       # pinned core + oracle/patches
ln -sfn "$ROM" "Phantasy Star IV (USA).md"   # verify.sh reads the ROM at that path
./oracle/verify.sh                           # builds the host, runs the fast lane

oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "$ROM" \
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
    --rom  "$ROM" \
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

Two conventions matter when comparing a capture against these pins. The log
names the ROM as it was given (`# rom=`, the full path above) and names the tape
and the trace by their basenames (`oracle/host/provenance.h`), because a path a
run was *handed* is not what it observed: the same capture re-run from another
directory now produces byte-identical bytes, traces and logs alike, which is
what makes a log's sha256 worth pinning. Both captures below were taken before
the tape line carried its basename, so they carry an older `# tape=` spelling
and an older `log_sha256`; their `# rom=` line is unchanged and their data is
identical either way.

### Provenance, and the current pins

| field | tape 07 | tape 09 |
|---|---|---|
| tape | `07_first_battle.tape`, 3145 steps, 36360 frames | `09_second_battle.tape`, 3147 steps, 37760 frames |
| core | Genesis Plus GX `2d7131c5efa606f649d36e1685a8ca47c24f31b3` + `oracle/patches/0001-rng-hv-trace.patch` | same |
| battle window | frames 24794-30428; the party's first action is f29489 | frames 25002-31908 |
| start state | frame 24808, the frame the formation was written into RAM | frame 25016 |
| trace | 136 calls in 15 frames, f24807-f30306, sha256 `0d97f6d6917c3440a211be2e0661eb8c363e04b748c2cef4e510ad32fbb9cb91` | 137 calls in 16 frames, f25015-f31786, sha256 `fbe5fa26f02a159108caf46670f546061debc5fdda6e13b869941576d82e0675` |
| RAM log | sha256 `4118ea6efda4cb1a2c5ce74cb04b66323eb26da37f39698a9e1a016156fe9ea4` | sha256 `1c40b5f2967496d4dc6aa472d03957bf965b0db9b2b5f8ce8d9f8d0c9ea55513` |
| trace check | `oracle/rng_trace.py check` passes: every row is `hv + frame_count - seed_high`, the seed chain closes on the log's `rng_seed`, and the VBlank counter step agrees | passes |
| battle roll stream | 134 of the 136; the encounter's formation draw (f24807) and the post-victory item drop draw (f30306) are recorded outside it | 135 of the 137; formation f25015, item drop f31786 |
| formation | two ZoranBult (enemy id 10) | Xanafalgue (id 9) and ZoranBult (id 10) |
| party | Alys lvl 7, Chaz lvl 1, Hahn lvl 1 | same |

Each fixture carries all of this, plus the log's sha256, as `provenance`. Tape
07's trace was re-captured in lane R against the current host and came back
byte-identical to the pin above; tape 09's was captured there with the fixed
host, and its fixture was re-extracted: every field outside `provenance` is
identical to the one it replaces, and `provenance` differs only in
`trace_sha256`, `log_sha256`, the two header lists, `roll_convention` and
`roll_column` (`agrees` 137, `subtracts_low_word` 0).

Tape 07's fixture still pins an older log sha256 -
`e2ed38f191525e27c48dd1e1214ed8baace19a4cd405552d0787cc4931b3c461` - because
`enemy_ability_index` joined `oracle/ram_map.json`'s `battle` group after that
capture. Dropping that one column from the log above reproduces the pinned bytes
exactly, so the two differ in that column and nothing else; see the follow-ups.

## The roll convention, and the fix

### What the cartridge computes

`UpdateRNGSeed2` is four instructions (ROM `$04239E`, `ps4.asm:86097`):

```text
30 2D 00 08     move.w  $8(a5), d0        ; d0 = VDP HV counter at $C00008
D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
90 78 EF 0C     sub.w   (RNG_Seed).w, d0   ; d0 = the roll the caller gets
E6 F8 EF 0C     ror     (RNG_Seed).w       ; $0423AA, same word
4E 75           rts
```

`(RNG_Seed).w` is an absolute-short operand at `$FFFFEF0C`, where `RNG_Seed`
(`ps4.constants.asm:2328`) is a **longword**. A 68000 word read at that address
is the longword's **high** half, so the roll is

```text
roll = (hv + frame_count - high_word(RNG_Seed)) & $FFFF
```

and the word `ror` rotates is the same one. The trace's `seed_after` column was
right even while its `roll` column was not, for exactly that reason: it is
`ror16` of the word the routine rotates, which no convention can get wrong.

### Which word the cartridge subtracts: the measurements

The cartridge settles it. Both derivations were run against tape 07's RAM log,
and only one of them reproduces what it records:

| observation | high half (cartridge) | low half |
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
low half, is not a coincidence: **the trace's `roll` column was wrong, and the
fixture derives the roll from the row's raw columns instead.**

Tape 09 is the same story on a second seed path: its fixture's rolls come from
those same raw columns, and the replay reproduces the battle's own turn order
(round 1's recorded 21, 11, 8, 8, 6; round 2's 20, 9, 8, 4) and all of its
damages. Its trace's `roll` column is the fixed one too: the low-word derivation
of those same rows differs from the cartridge's roll in all 137 of them.

### The defect, and what now stops it

The host wrote `roll = hv + frame_count - seed_lo` (the word at `$FFFFEF0E`,
which no instruction reads as the subtrahend) until lane P1 fixed it where it
was written, in `rng_trace_roll` (`oracle/host/rng_trace.h`, called by
`oracle/host/rng_trace.c`), and in `oracle/rng_trace.py`'s `roll_for`. The two
halves differ by a per-frame constant, so every row of the pre-fix column was a
per-frame-constant shift of the cartridge's roll - and `rng_trace.py check` could
not see it, because its own `roll_for` repeated the same subtraction: the host
wrote it, `check` re-derived it, and the two agreed with each other and with
nothing else. That is the shape of failure two of lane P1's changes block, and a
third makes a capture pinnable across lanes:

- `oracle/battle_fixture.py` no longer accepts a column it merely recognises.
  Every row's `roll` is checked against the derivation above, and the first row
  that disagrees aborts the extraction with its frame and call, so a capture
  carrying the low half cannot reach a fixture at all
  (`provenance.roll_column` is `{"agrees": n, "subtracts_low_word": 0,
  "neither": 0}`).
- `tests/test_oracle_rng_trace.py` builds a probe from
  `oracle/host/rng_trace.h` and compares what the C host computes with the
  checker's `roll_for` against numbers written out from the disassembly, so the
  two cannot drift into the same mistake again.
- `oracle/host/provenance.h` writes the paths the run *names* as basenames -
  the tape now included, not only `--rng-trace` - so two runs that differ only
  in their directories are byte-identical. `tests/test_oracle_force_battle_provenance.py`
  is the control: it reads the `# tape=` call site and compiles
  `path_basename` into a probe, and the capture measurement is a copy of the
  Helex capture's tape replayed from another directory, whose log and trace come
  back byte-identical
  (`docs/BATTLE_ORACLE_FORCED.md` §3a, `build/lane-evidence/group_compare/`).

### The negative controls of the data-driven replay

`replay_fixtures/divergences.json` is what keeps the one data-driven test
honest, and both directions are checked by running it:

* **A bogus entry for a fixture that replays exactly.** Adding
  `tape07_first_battle` to the manifest fails the test with "carries an entry
  for f29489 (value) but the fixture replays exactly - the manifest is stale";
* **A changed roll in a captured fixture.** Flipping the sixteen damage draws of
  the Helex capture's first FLAME BOLT (`forced_5e_helex.json`, f25043) fails it
  at that action with the log's 78 against the port's 77.
* **The vehicle's swing removed again.** Putting a vehicle back on
  `Character_Attack`'s weapon check - `resolve_attack`'s dispatch to
  `vehicle_attack` deleted - fails the test at exactly the finding the deleted
  entry carried: `forced_53_desrtleach: the port diverges at f25026 (round 1,
  no-swing)`, the log's actor 1 resolving one target against the port's no
  swing. Restoring it passes. Transcripts in the lane's
  `build/lane-evidence/03-negative-control.log` and
  `04-replay-after-restore.log` (not committed).

Both were run against the tree this ledger describes; the transcripts are in
`build/lane-evidence/negative_controls.log` and
`build/lane-evidence/negative_control_b.log` of the lane that wrote them.

### The negative control

A capture carrying the pre-fix column is rejected rather than replayed. Lane R's
control takes tape 07's regenerated trace and replaces every row's `roll` with
the low-word value - what the pre-fix host wrote - leaving every other column
alone, which is the whole of the difference the defect made; the two commands
below then run against that flipped file:

```text
$ python3 oracle/rng_trace.py check build/tape07_rolls_lowword.csv build/tape07_battle.csv
  FAIL  f24807 call 0: roll 1CA6 is not (hv + frame_count - seed_high) & $FFFF = 1814
$ python3 oracle/battle_fixture.py --trace build/tape07_rolls_lowword.csv --log build/tape07_battle.csv --out /tmp/f.json
FixtureError: trace f24807 call 0: the roll column reads 1CA6, the low-half
subtraction (hv + frame_count - seed_low) & $FFFF = 1CA6, ... the cartridge's
roll is (hv + frame_count - seed_high) & $FFFF = 1814
```

The suite has the same control without needing a capture:
`tests/test_oracle_rng_trace.py`'s
`TestCheckRejects::test_a_low_word_roll_column_is_rejected` and
`::test_the_low_word_column_is_rejected_at_the_row_that_uses_it` (the checker,
naming the frame and call),
`HostAgreement::test_the_host_does_not_subtract_the_seeds_low_half` (the C host
probe), and
`tests/test_oracle_battle_fixture.py::test_a_low_word_roll_column_is_rejected_with_its_frame_and_call`
(the extractor).

## The reorganization, and what stayed put

The extractor and the replay harness were reorganized under the repository's
file-size rule: `oracle/battle_fixture.py` is a thin CLI over `oracle/fixture/`,
`oracle/force_battle.py` keeps the capture tool's CLI over its own package, and
`rust/psiv-core/src/battle/replay/` holds the fixture's shape, the battle
builder, the verbatim-stream driver, the comparator and the data-driven test
(the wiring stays in `engine_tests_replay.rs`). Two measurements say nothing
moved with it:

* **The tapes' data is byte-stable.** Re-extracting tapes 07 and 09 from fresh
  captures with the reorganized extractor reproduces every field the committed
  fixtures already had - zero differences, provenance aside
  (`build/lane-evidence/tape07-09-recheck.log`);
* **Every test name survives.** The names in the two tape modules and
  `engine_tests_replay.rs` are unchanged (the diff in
  `build/lane-evidence/test-names.log` is additions only: the new
  data-driven test and the new modules' constructors).

The fixture schema is additive, so the tapes' fixtures were not touched:
`kind`, `ability`, `vehicle`, `outcome.defeat`, `outcome.dead_party_ids`,
`animation_hit_pass_ids` and the provenance notes are new keys, and a fixture
without them reads as a battle whose every action is a physical attack.

## Three divergences, all closed

### Alys's and Kyra's second hit pass

The four hit rolls of Alys's first swing are `AlysKyraAttack_Init`
(`ps4.asm:13975-13976`) calling `loc_B6A2` a second time: `Character_Attack`
(`ps4.asm:13018`) runs the hit pass, and the swing's animation routine runs it
again, so a two-enemy swing rolls `Battle_CalculateChances` twice per target and
the later pair is what survives in `Fighters_Hit_Flags` (`loc_B6A2` presets all
nine to `$FF` before every pass). In round 2 only one enemy is left standing, so
the two passes cost one roll each. The attacker set is the character and not the
weapon: `Character_AttackActionOffs` (`ps4.asm:13056-13068`) sends `Character_Stats`
indices 1 and 9 - Alys and Kyra - to `CharAttack_AlysKyra` (`ps4.asm:13958`),
and no other entry's routine calls `loc_B6A2`. The instruction-level reading, the
unreachable single-target case and the other `loc_B6A2` callers are in
[`battle-party.md`](source-notes/battle-party.md#the-second-hit-pass-of-alyss-and-kyras-attack-2026-09-24),
"The second hit pass of Alys's and Kyra's attack".

`psiv-core` draws the pass where the cartridge does:
`rust/psiv-core/src/battle/action.rs`'s `takes_second_hit_pass(roster, actor)` and its
`SECOND_HIT_PASS_CHARACTERS` (`Character_Stats` indices 1 and 9, keyed on the
roster's own `Fighter::character`), with `resolve_attack` drawing the first pass,
then the second when the rule says so, and using the last - both before any
damage draw. The rule's own tests are
`rust/psiv-core/src/battle/action_second_pass_tests.rs` (seven tests, registered
from `action.rs`), and the tapes' replays are what show it in
place: Alys's 36-call swing and round 2's 18.

**Negative control:** removing the second pass fails eleven tests in
`psiv-core`'s lib test binary (450 of its 461 pass, and cargo stops there), and
both tapes' verbatim replays fail with the pass's own frames: tape 07's first
divergence is back at f29489 - Alys -> Enemy2, `Normal, Some(11)` where the log
has 10 - and tape 09's at f30889 (9 against 10). The other nine are `action_second_pass_tests`' six (all but
`an_unlisted_attacker_draws_one_pass`, which passes in both states and exists to
be that control) and `rust/psiv-core/src/battle/action.rs`'s
`a_multi_target_swing_rolls_both_hit_passes_then_damages_each` and
`a_multi_target_swing_can_never_crit`.

### The vehicle's swing: command 6, and three hit passes

A forced capture found this one: the `$53` Desrt Leach battle
([`BATTLE_ORACLE_FORCED.md`](BATTLE_ORACLE_FORCED.md)), where the port's vehicle
party-side fighter spent every turn as `TurnSkipped { reason: Unarmed }` while
the log showed it swinging. The cause was route, not equipment: a vehicle's
Attack is command 6, whose fighter routine is `loc_AF9C` (`ps4.asm:16810`)
rather than `Character_Attack`, so the weapon check that skipped the swing never
runs for it, and the swing draws `loc_B6A2` **three** times - state 4's
`jmp loc_B6A2` as the attack object is created, state 5's `jsr loc_B6A2`
(`loc_9848`, `ps4.asm:14964`), and state 5 again once that object hands the
vehicle's `action_routine` back (`move.w #5, $32(a0)`, `ps4.asm:82975` and its
two siblings). Its damage is not the weapon path either: `loc_280A`
(`ps4.asm:4016`) reads the **target's** element-2 property and nothing of the
attacker's hands.

`psiv-core` models it in `rust/psiv-core/src/battle/vehicle_attack.rs`, with
`resolve_attack` dispatching to `resolve_vehicle_attack` for a fighter whose
`Fighter::character` is one of `loc_78EE`'s ids (`Vehicle_Index + $B`). The
instruction-level reading, the six rounds' arithmetic and the tests are in
[`source-notes/battle-party.md`](source-notes/battle-party.md#the-vehicles-own-attack-command-6-three-hit-passes-2026-09-24),
"The vehicle's own attack".

One reading of the fixture changed with it. A per-target `hit` byte is the
value at the action's **hit frame** (`oracle/fixture/observations.py`), so for a
swing whose passes arrive in three frames it holds the *first* pass's verdict,
while the damage holds the last one's: round 3 of the capture is a `$01` over a
154 that only a normal hit's arithmetic produces. `replay/compare.rs`'s
`flag_lags_the_swing` reads such a byte as a reach claim instead of a verdict,
and the damage stays the thing the verdict is checked through. The case the byte
can no longer check is pinned by
`battle/vehicle_attack_tests.rs`'s
`the_logs_third_round_needs_the_last_passs_verdict` on round 3's own rolls.

### `$FFFFEEA8`: the ability re-roll word, and how long it lives

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

`grep` finds that address written nowhere else in the listing: **the re-roll loop
is the word's only reader and its only writer.**

#### Every clear that reaches it

A grep by address cannot see bulk clears, so the listing's 131 `trap #0` sites
were scanned - each clear's range read from its own `lea ..., a0` /
`move.w #N, d7` pair, since `Trap00Exception` (`ps4.asm:161-165`) is
`moveq #0,d0 / move.l d0,(a0)+ / dbf d7,Trap00Exception` and therefore covers
`d7+1` longwords from whatever `a0` holds. Exactly **one** of them reaches the
page the word lives in, `$FFFFEE00-$FFFFEEFF`:

| clear | site | range | covers `$FFFFEEA8` |
|---|---|---|---|
| `$FFFFEE00` page, `GameMode_LoadBattle` | `ps4.asm:9992-9994` (ROM `$006A38`) | `$FFFFEE00-$FFFFEEFF` | **yes** |
| `Battle_Objects_Memory` | `ps4.asm:9980-9982` | `$FFFFD000-$FFFFDFFF` | no |
| `Enemy_Sprites` | `ps4.asm:9983-9985` | `$FFFFEA00-$FFFFEBFF` | no |
| `Battle_Palette_Objects` | `ps4.asm:9986-9988` | `$FFFF2A90-$FFFF2C8F` | no |
| `$FFFF4000` (skill list) | `ps4.asm:9989-9991` | `$FFFF4000-$FFFF47FF` | no |
| the other 126 sites | `ps4.asm` | none of them reaches `$FFFFEE00-$FFFFEEFF`; six are not fully resolvable statically (below) | no, with one exception to argue rather than read |

Six of the 131 are not fully resolvable by that static read, and five of them
cannot reach the cell either: two (site lines 4430 and 6682) clear a window
buffer from one of two known bases, `$FFFF8616` or `$FFFF8916`, with `d7` `$47`;
one (line 84461) clears `Plane_A_Buffer` (`$FFFF8000`) for `$3FF` longwords; and
two (lines 121901 and 121924) clear a 64-byte actor window whose base is a
register. The sixth is the exception: line 75181 (`loc_39DAA`, a battle object's
DMA helper) clears from `RAM_Start` for a length it reads out of a record, so
the static read cannot bound it - the whole 64 KiB is arithmetically reachable.
What keeps it away from the cell is the measurement below, not the scan: the
word survives the whole field stretch between tape 10's battles and moves only
on frames where an ability roll happens.

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

#### What the oracle measures

`enemy_ability_index` was added to `oracle/ram_map.json` (`$FFFFEEA8`, size 2,
group `battle`; see [`oracle/README.md`](../oracle/README.md#ram-map)) and
`ram_map.tsv` regenerated with `oracle/gen_ram_map.py`. With it in the log the
word's whole life is visible frame by frame - the transitions below are re-read
in lane R from captures of these tapes with the current map:

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

#### Three probes that patch the word

The tapes' own logs can only show a zero being compared against a zero. Runs
with `--ram-patch` (RAM patched, so experiments rather than natural-route
evidence; re-run in lane R on tape 07) close that:

| probe | patch | what the log shows |
|---|---|---|
| boot clear | `--ram-patch 1:FFFFEEA8:0005` | `0005` at f1, `0000` at f2: the ROM's boot loop clears the cell during the second frame of the run |
| field, then the battle load | `--ram-patch 24000:FFFFEEA8:0005` | `0005` from f24000 through the field and the encounter trigger, `0005` still at f24796, `0000` from f24797: the battle-load wipe, with nothing in the field touching it |
| the re-roll is conditioned on the word | `--ram-patch 24850:FFFFEEA8:0005` | the enemy's draw of zero at f29789 is **stored**, so the word goes `0005 -> 0000` instead of `0000 -> 0003`, and Hahn takes 8 damage where the unpatched run has 6 (21 -> 13 against 21 -> 15) |
| ... and what it costs | the same patch, with `--rng-trace` | f29789 holds **2** calls instead of 3 and the run holds 135 rolls instead of 136: patching the word removed exactly the re-roll |

The last two are the rule itself, measured on the cartridge: the comparison
against whatever the word holds decides whether the *next* call is a re-roll,
and therefore what every following roll is used for.

#### What the port does with it

`psiv-core` owns the whole rule, and the battle owns the word's lifetime:

* `Battle::last_ability_index` is a `u16` that **starts at zero** - the value
  the battle load's wipe leaves - with both clears cited on the field
  (`ps4.asm:9992-9994`, and the boot's `ps4.asm:376-402`). No caller hands a
  battle a word, `Battle::start`/`Battle::start_vehicle` take none, and nothing
  carries one between battles.
* `choose_ability` (`rust/psiv-core/src/battle/ai.rs`) compares each draw against it and stores the index
  it settles on; `Enemy_Attack`'s dispatcher keeps it for the whole battle.
* `psiv-runtime` has no plumbing for it at all. A session word would be dead
  weight: the cartridge clears the cell before a battle can read it, so the only
  value a load can pass is the zero the battle already starts with.
* Tape 07's replay ends with the word at `3` and tape 09's at `1` - what the RAM
  logs hold at those battles' ends - and both are asserted in the tests.

**Negative control:** starting the word anywhere other than the wipe's zero
breaks the tape that carries the case. With the word seeded to "no previous
index" (`0xFFFF`, so no first draw can match it), `cargo test -p psiv-core`
fails three tests in the lib binary (458 of its 461 pass): the two replay tests
above and `rust/psiv-core/src/battle/engine_tests_abilities.rs`'s
`a_battle_starts_with_the_ability_reroll_word_at_zero`. Tape 07's verbatim
replay fails at f29789 - the enemy's action, which is exactly the frame the
missing re-roll shifts - with `Value { frame: 29789, actor: FighterId(7),
target: FighterId(3), port: Critical, port_damage: Some(10), log_hit: 0,
log_damage: Some(6) }` against the log's 6. Tape 09 does **not** fail under
this control: neither of its ability draws is zero, so the cartridge re-rolls
there too and the word's starting value never shows.

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
- **The queue is the cartridge's.** The log's own pair list is already in
  resolution order and descending, and the port's sort reproduces it - including
  tape 07's tie at 11 between Chaz (7+4) and Enemy2 (6+5), which the log
  resolves in **Chaz's** favour, and tape 09's tie at 8 between Chaz and Hahn,
  which it also resolves in Chaz's.
- **The enemies' stats are the record's.** The fixtures' live RAM values are
  asserted against what `Stats::from_enemy` derives from the pack record, and
  the party's against `Stats::from_character` - the port's fixture records and
  the oracle's RAM agree digit for digit.

### The `$FF` hit flag: a miss and an untargeted slot

`Fighters_Hit_Flags` reads `$FF` both for a slot a swing missed and for a slot
the swing never reached, because `loc_B6A2` blanks all nine flags to `$FF`
before every pass. Tape 07's battle has no misses, so the first cut of this
ledger simply skipped `$FF` slots - and tape 09's battle is what makes that
insufficient. At f31381 the second enemy draws ability index 1 and then a hit
roll of `33419`, and the log leaves Alys's flag at `$FF` with her damage word
unmoved and her HP at 52: a miss.

The comparator walks the port's own target list instead. Every slot the log
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
  available ([`RUNTIME_DESIGN.md`](RUNTIME_DESIGN.md#rng-design), "RNG design").
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

## Follow-ups still open

Closed at integration: `battle-party.md`'s dated "moves from f29489 to f29789"
sentence now carries a dated correction, and tape 07's fixture was re-extracted
against today's RAM map (same data; `log_sha256` `4118ea6e...`; trace still
`0d97f6d6...`).

1. **Fixture the remaining battle tapes.** `oracle/battle_fixture.py` needs no
   change for a new tape - `--tape`, `--battle-first`, `--battle-last` and the
   two logs are enough. Tape 10's three encounters in a row are the natural next
   one: three more battles on their own seed paths, and a chance at a second
   instance of the case tape 07 carries - a battle whose first ability draw is
   zero, against the word the load left.
2. **Measure the save/continue path's word.** It is read from the bytes today
   (see above). A tape that saves, powers off and continues would put the
   word's third edge on the same footing as the boot and the battle load.
