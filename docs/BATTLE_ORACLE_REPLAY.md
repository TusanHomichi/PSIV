# Replaying tape 07's battle with the cartridge's own rolls

What this ledger records: the first basement battle of
`oracle/tapes/07_first_battle.tape` replayed inside `psiv-core` on the exact
rolls the cartridge drew, compared action by action with what the oracle's RAM
log shows the cartridge doing. The replay is
`rust/psiv-core/src/battle/engine_tests_replay.rs`; the fixture it reads is
`rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json`, produced
by `oracle/battle_fixture.py` from one oracle run's RNG trace and RAM log.

Two things came out of it, and they are different claims:

1. **The port's battle rules are exact on the cartridge's rolls.** Given the
   rolls for the roles the port models, every action of both rounds resolves
   identically: the queue, every swing's target list, every verdict, every
   damage number, the HP left behind, the two deaths, the rewards and the
   outcome. This is the end-to-end check the RNG trace alone could not make.
2. **Two draw-count divergences, and one defect in the trace itself.** The
   cartridge draws rolls no port consumer models, and the trace's own `roll`
   column was not the cartridge's roll. The column is fixed and the capture
   regenerated (lane P1, "The fix" below); the two draw-count divergences are
   still open and pinned below with their numbers and their citations.

## Reproducing it

```sh
./oracle/build_core.sh                       # pinned core + oracle/patches
# verify.sh reads the ROM at the repo-relative path, so a checkout without the
# ROM needs the link; the capture steps only need --rom.
ln -sfn "/path/to/Phantasy Star IV (USA).md" "Phantasy Star IV (USA).md"
./oracle/verify.sh                           # builds the host, fast lane
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "/home/peter/PSIV/Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --groups core,battle,bhit,enemy,chars,rng \
    --rng-trace build/tape07_rolls.csv \
    --out  build/tape07_battle.csv
python3 oracle/rng_trace.py check build/tape07_rolls.csv build/tape07_battle.csv
python3 oracle/battle_fixture.py --trace build/tape07_rolls.csv \
    --log build/tape07_battle.csv \
    --out rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 tape07
rm "Phantasy Star IV (USA).md"
```

The ROM in every capture above is the project's own checkout
(`/home/peter/PSIV/Phantasy Star IV (USA).md`, sha256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`, the digest
`oracle/verify.sh` insists on); O1 and O2 ran the block in a worktree that had
linked it at the repo-relative path for `verify.sh` and removed the link
afterwards, while P1's lane worktree came with that link already made and left
it in place. The `--rom` argument is quoted in full above because it is an
*input*: the log records it verbatim, so the pinned log - and the fixture's
`log_header` - reproduce under that spelling and not under a relative one.
Output paths are not recorded that way: the log names the trace by basename
alone (`oracle/host/provenance.h`), which is why the two captures P1 made into
different directories are byte-identical, traces and logs alike.

## Provenance

| field | value |
|---|---|
| tape | `oracle/tapes/07_first_battle.tape`, 3145 steps, 36360 frames |
| core | Genesis Plus GX `2d7131c5efa606f649d36e1685a8ca47c24f31b3` + `oracle/patches/0001-rng-hv-trace.patch` |
| battle window | frames 24794-30428; the party's first action is f29489 |
| trace | 136 calls in 15 frames, f24807-f30306, sha256 `0d97f6d6917c3440a211be2e0661eb8c363e04b748c2cef4e510ad32fbb9cb91` (lane P1's fixed host; the same tape and core captured with the pre-fix low-half subtraction differ in the `roll` field of all 136 rows and in no other field. O1's pin `354d3412af750f75ef0fd85f5a871a9f28a65a7f170547c14ac9efdf76324bf0` is superseded; that file was not retained, so it cannot be re-derived to compare against) |
| RAM log | sha256 `e2ed38f191525e27c48dd1e1214ed8baace19a4cd405552d0787cc4931b3c461`; the same tape, map and `--groups` as O1/O2, and identical to the same run written to a second directory |
| trace check | `oracle/rng_trace.py check` passes: every row is `hv + frame_count - seed_high`, the seed chain closes on the log's `rng_seed`, and the VBlank counter step agrees |
| battle roll stream | 134 of the 136; the encounter's formation draw (f24807) and the post-victory item drop draw (f30306) are recorded outside it |
| start state | frame 24808, the frame the formation was written into RAM |

The fixture carries all of this, plus the two log shas, as `provenance`.

## The roll the cartridge computes, and the one the trace writes

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

and the word `ror` rotates is the same one - which is why the trace's
`seed_after` column was right even while its `roll` column was not: O1's host
subtracted the *low* half (`$FFFFEF0E`), a word no instruction reads as the
subtrahend. The two halves differ by a per-frame constant, so every row of that
capture was a per-frame-constant shift of the cartridge's roll. Nothing else in
it was affected: `hv`, `frame_count`, `seed_before` and `seed_after` are the
values the emulator produced, and its 136-row seed chain still closed on the
RAM log exactly (that is what `rng_trace.py check` proves, and it is
independent of this defect).

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
low half, is not a coincidence: **the trace's `roll` column was wrong, and the
fixture derived the roll from the row's raw columns instead.**

## The fix: the capture now carries the cartridge's roll

Lane P1 fixed the derivation where it was written - `rng_trace_roll`
(`oracle/host/rng_trace.h`, called by `oracle/host/rng_trace.c`) and
`oracle/rng_trace.py`'s `roll_for` - regenerated the tape 07 capture, and closed
the loop that hid it:

- `oracle/battle_fixture.py` no longer accepts a column it merely recognises.
  Every row's `roll` is checked against the derivation above and the first row
  that disagrees aborts the extraction with its frame and call, so a capture
  carrying the low half cannot reach a fixture at all
  (`provenance.roll_column` is `{"agrees": 136, "subtracts_low_word": 0,
  "neither": 0}`).
- `tests/test_oracle_rng_trace.py` builds a probe from
  `oracle/host/rng_trace.h` and compares what the C host computes with the
  checker's `roll_for` against numbers written out from the disassembly, so the
  two cannot drift into the same mistake again - which is exactly how the
  low-half subtraction survived O1: the host wrote it, `check` re-derived it,
  and the two agreed with each other and with nothing else.
- The negative control is in the same lane: a scratch host built with the
  low-half subtraction reproduces the pre-fix column, the two captures differ in
  the `roll` field of all 136 rows and in nothing else (each shift being exactly
  `seed_lo - seed_hi`), and `oracle/rng_trace.py check` now rejects that capture
  at its first row - `f24807 call 0: roll 1CA6 is not (hv + frame_count -
  seed_high) & $FFFF = 1814`.

The regenerated trace keeps everything else: captured with the low-half
subtraction by an otherwise identical host, the 136 rows differ from it in the
`roll` field alone, each shift being exactly `seed_lo - seed_hi`, and the two
rows the previous oracle README quoted from O1's capture come back byte for
byte, `seed_after` included. The seed chain still closes on the RAM log
exactly. `oracle/battle_fixture.py` produced the same fixture from it -
`rolls`, `rounds`, `formation`, `party` and `outcome` are byte-identical to the
one above, and `provenance` changed only where it records the capture itself
(trace sha256, log sha256, the two headers, the `roll_column` counts). The
replay is undisturbed:
`tape07s_actions_match_once_the_unmodelled_rolls_are_removed` passes on the
regenerated fixture and the pinned divergence test stays ignored. The one
assertion that fails is the pre-fix `roll_column` pin in
`engine_tests_replay.rs` - a three-line edit, and the last piece of the
follow-up below.

## The verdict: exact match, once the unmodelled calls are accounted for

### Draw counts, per action

| round | action | frame | cartridge drew | roles the port models | left over |
|---:|---|---:|---:|---:|---:|
| 1 | order pass | 29483 | 13 | 13 (9 jitter + 4 `Enemy_TargetCharacter`) | 0 |
| 1 | Alys -> both enemies | 29489 | 36 | 34 (2 hit + 32 damage) | **2** |
| 1 | Chaz -> Enemy1 | 29644 | 17 | 17 (1 hit + 16 damage) | 0 |
| 1 | Enemy2 -> Hahn | 29789 | 19 | 18 (1 ability + 1 hit + 16 damage) | **1** |
| 1 | Hahn -> Enemy2 | 29885 | 17 | 17 | 0 |
| 2 | order pass | 30091 | 13 | 13 | 0 |
| 2 | Alys -> Enemy2 | 30097 | 18 | 17 (1 hit + 16 damage) | **1** |

The two rolls left over at Alys's first swing are `AlysKyraAttack_Init`
(`ps4.asm:13975-13976`) calling `loc_B6A2` a second time: `Character_Attack`
(`ps4.asm:13018`) runs the hit pass, and the swing's animation routine runs it
again, so a two-enemy swing rolls `Battle_CalculateChances` twice per target
and the later pair is what survives in `Fighters_Hit_Flags`. `psiv-core`'s
`resolve_attack` -> `roll_hits` models the single pass, which is the first one.
The roll left over at the enemy's turn is `Enemy_Attack` (`ps4.asm:19138`):
`loc_CFE6` (`ps4.asm:19146-19151`) re-rolls the ability while it equals
`$FFFFEEA8`, a word nothing clears (`grep` finds it written nowhere else), so a
first draw of zero in a battle whose RAM holds zero burns a second call.
`psiv-core` starts `last_ability_index` at `None`, so it never re-rolls the
first draw.

Both are draw-count divergences in the port, not in the fixture: the engine's
own documentation says the *values* may be substituted but the *count* must not
change (`rust/psiv-core/src/battle/engine.rs`, "Roll accounting"). Fixing them
is a rules change with its own consequences (`roll_hits` would need the swing's
animation identity, and `last_ability_index` would have to carry the retail
word across battles), so this lane only pins them.

### Values, action by action

With each round fed the rolls of the roles the port models - the order pass,
one hit pass, the ability roll, the damage runs - every action matches:
`tape07s_actions_match_once_the_unmodelled_rolls_are_removed` asserts the whole
battle and that the port consumes exactly those rolls, no slack and no surplus.
The comparison covers, for every action: the actor, the target list, the
verdict (`$00` normal / `$01` critical), the damage, the target's HP after
(floored at zero: the cartridge stores -2 and -1 where the port reports what a
player sees), and each death. It also covers both rounds' queues, the rewards
(24 experience over three living members, 6 meseta - the log shows Chaz
0 -> 8, Hahn 0 -> 8, Alys 2457 -> 2465 and the purse 600 -> 606) and the
victory.

Three checks in the same test are worth naming, because each one could have
failed on its own:

- **The opening draw is the cartridge's.** `Battle::start`'s single `loc_B62A`
  draw (`ps4.asm:10043`) on the fixture's f24840 roll yields
  `Battle_Priority` 0, which is what the RAM log holds for the whole battle.
- **The queue is the cartridge's.** Nine jitter draws and four target draws
  produce `Battle_Turn_Order`'s own order in both rounds - including the tie
  between Chaz (7+4) and Enemy2 (6+5) at 11, which the log resolves in Enemy2's
  favour.
- **The enemies' stats are the record's.** The fixture's live RAM values
  (attack 16, defence 2, agility 6, strength 18, mental 4, dexterity 8, HP 25)
  are asserted against what `Stats::from_enemy` derives from the pack record,
  and the party's against `Stats::from_character` - the port's fixture records
  and the oracle's RAM agree digit for digit.

## The first divergence on the verbatim stream

Feeding each round the cartridge's rolls **as the trace holds them**, in the
order and with the surplus the log has, the port stops matching inside Alys's
first swing:

| | cartridge (log) | port, verbatim stream |
|---|---|---|
| Alys -> Enemy1 | normal, 12, HP 13 | normal, 12, HP 13 |
| Alys -> Enemy2 | normal, **10**, HP 15 | normal, **11**, HP 14 |
| rolls for the swing | 36 (f29489 x4, f29599 x32) | 34 (2 hit + 32 damage) |
| rolls for round 1 | 102 | 85 |

`tape07_orders_its_rounds_the_way_the_cartridge_did` asserts everything before
that point - the priority draw, the round-1 queue, Alys's swing at both
enemies, and the first target's verdict, damage and HP - and then asserts this
divergence itself, so the ledger's first row is executable.

**Reading, and what settles it.** The cause is the draw-count divergence above,
not a damage-formula error: the extra two calls at f29489 push the port's
sixteen-draw window for each target two rolls early, so Enemy1 still lands on
12 by coincidence and Enemy2 reads 10's window one step off (sums 66 and 60
where the cartridge's two runs sum 71 and 50). The evidence that the formula
itself is right is the same table twice over: with the modelled stream, every
damage in the battle matches, and on the verbatim stream the round's count comes
out 85 rather than 102, because the shift makes Chaz's and Hahn's hit rolls
miss where the log has them killing - a *value* consequence of a *count*
divergence, which is exactly what the port's own roll-accounting rule forbids.

`tape07_full_battle_diverges_at_alyss_swing_draw_count` is the same full-battle
assertion on one verbatim stream, parked behind `#[ignore]` with this section
as its reason: it is the test that passes once the port draws the second hit
pass and the ability re-roll.

## What this does and does not prove

Proved: on the cartridge's own rolls, the port's battle resolution is exact -
queue, targets, verdicts, damages, HP, deaths, rewards - for the whole of tape
07's first basement battle, with the two draw-count divergences narrowed to
named routines.

Not proved, and not claimed:

- **The port's rolls are the cartridge's roll *stream*.** They are, for the
  frames this battle occupies: the rolls come from the emulator's HV counter
  reads. Nothing here says anything about frames the trace does not cover, or
  about the substitute a headless port would use where the beam position is not
  available (`docs/RUNTIME_DESIGN.md`, "RNG design").
- **Every command the party issued.** The RAM log carries menu cursors, not
  the chosen commands; every party action in this battle lands damage on an
  enemy slot with no TP or status movement, and the tape holds C through the
  command phase, so the fixture records `attack` for each member and says so in
  `provenance.undetermined`.
- **A miss versus an untargeted slot.** `Fighters_Hit_Flags` reads `$FF` for
  both. This battle has no misses, so nothing here rests on the distinction,
  and the comparator skips `$FF` targets rather than guessing.
- **Everything the battle does outside the turn engine.** The formation draw
  and the item drop draw are recorded outside the battle's stream because no
  battle routine consumes them; the port models neither.
- **Behaviour beyond these two rounds and one formation.** Two ZoranBult, a
  party of three, four actions in round one and one in round two. Other tapes
  and other formations are other fixtures.

## Follow-ups

1. **Fix the host's roll column** - **done** (lane P1). `oracle/host/rng_trace.h`'s
   `rng_trace_roll` subtracts `seed_hi` and `oracle/rng_trace.py`'s `roll_for`
   matches it, the tape 07 capture was regenerated (sha256
   `0d97f6d6917c3440a211be2e0661eb8c363e04b748c2cef4e510ad32fbb9cb91`; the same
   tape and core captured with the low-half subtraction differ from it in the
   `roll` field of all 136 rows and in no other field), and
   `oracle/battle_fixture.py` refuses
   any capture whose column is not the cartridge's. What is left of it: the
   `roll_column` assertion in
   `rust/psiv-core/src/battle/engine_tests_replay.rs:672` still pins the pre-fix
   counts (`subtracts_low_word` 136 / `agrees` 0); the fixture now carries
   `agrees` 136 / `subtracts_low_word` 0, so that assertion - and only that
   one - fails, and flipping those two numbers is the whole repair. That file is
   outside P1's write set (it belongs to lane P2a), so P1 left the three-line
   patch at `build/lane-evidence/engine_tests_replay-roll-column.patch` in its
   worktree and verified the replay passes with it applied.
2. **Decide the two draw-count divergences.** `roll_hits` needs the swing's
   animation identity to know that Alys's and Kyra's weapons run `loc_B6A2`
   twice, and `last_ability_index` needs the retail `$FFFFEEA8` semantics (a
   word a battle does not clear) rather than a fresh `None`. Both are behaviour
   changes with their own tests to write, and both are visible here as counts
   rather than as wrong numbers.
3. **Fixture the other traced battle.** Tape 09's battle (`oracle` "Battle
   ground truth") has a different formation, a critical and a different seed
   path; the same three steps - trace, extractor, replay - would widen this
   ledger from one battle to two.
