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
2. **A draw-count divergence, a fixed one, and a defect in the trace itself.**
   The cartridge draws rolls no port consumer models, the trace's own `roll`
   column is not the cartridge's roll, and the lane that first wrote this
   ledger found two draw-count divergences. **One of the two is fixed**: the
   second hit pass of Alys's and Kyra's swing (`loc_B6A2` run again from
   `AlysKyraAttack_Init`, `ps4.asm:13975-13976`) is modelled now, so her swing
   consumes all 36 calls the log's frames hold and round 1's Alys action matches
   on the verbatim stream. The other — `Enemy_Attack`'s ability re-roll
   (`$FFFFEEA8`) — is still open, and the trace's roll column is still wrong.
   Each is pinned below with its numbers and its citations.

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
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- --test-threads=1 tape07
rm "Phantasy Star IV (USA).md"
```

This lane ran it in a worktree whose ROM is the project's own checkout
(`/home/peter/PSIV/Phantasy Star IV (USA).md`, sha256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`, the digest
`oracle/verify.sh` insists on) and removed the link afterwards.

## Provenance

| field | value |
|---|---|
| tape | `oracle/tapes/07_first_battle.tape`, 3145 steps, 36360 frames |
| core | Genesis Plus GX `2d7131c5efa606f649d36e1685a8ca47c24f31b3` + `oracle/patches/0001-rng-hv-trace.patch` |
| battle window | frames 24794-30428; the party's first action is f29489 |
| trace | 136 calls in 15 frames, f24807-f30306, sha256 `354d3412af750f75ef0fd85f5a871a9f28a65a7f170547c14ac9efdf76324bf0` |
| trace check | `oracle/rng_trace.py check` passes: every row is `hv + frame_count - seed_lo`, the seed chain closes on the log's `rng_seed`, and the VBlank counter step agrees |
| battle roll stream | 134 of the 136; the encounter's formation draw (f24807) and the post-victory item drop draw (f30306) are recorded outside it |
| start state | frame 24808, the frame the formation was written into RAM |

The fixture carries all of this, plus the two log shas, as `provenance`.

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

and the word `ror` rotates is the same one - which is why the trace's
`seed_after` column is right and its `roll` column is not: the host subtracts
the *low* half (`$FFFFEF0E`), a word no instruction reads as the subtrahend.
The two halves differ by a per-frame constant, so every row of the trace is a
per-frame-constant shift of the cartridge's roll. Nothing else in the trace is
affected: `hv`, `frame_count`, `seed_before` and `seed_after` are the values
the emulator produced, and the trace's own 136-row seed chain still closes on
the RAM log exactly (that is what `rng_trace.py check` proves, and it is
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
low half, is not a coincidence: **the trace's `roll` column is wrong and the
fixture derives the roll from the row's raw columns instead.**

`oracle/battle_fixture.py` recomputes both conventions for every row and records
which one the file carries (`provenance.roll_column`: `agrees` vs
`subtracts_low_word`), and refuses a trace that carries neither rather than
replaying something nobody can account for. The fix belongs in
`oracle/host/rng_trace.c` (`roll = hv + frame_count - seed_lo` -> `seed_hi`)
and in `oracle/rng_trace.py`'s `roll_for`, which re-derives the same
arithmetic; neither is in this lane's write set beyond the host, and changing
the column would change the trace's sha256, which this lane's acceptance
requires to stay the one above. Filed as the first follow-up below.

## The verdict: exact match, once the unmodelled calls are accounted for

### Draw counts, per action

| round | action | frame | cartridge drew | roles the port models | left over |
|---:|---|---:|---:|---:|---:|
| 1 | order pass | 29483 | 13 | 13 (9 jitter + 4 `Enemy_TargetCharacter`) | 0 |
| 1 | Alys -> both enemies | 29489 | 36 | 36 (4 hit + 32 damage) | 0 |
| 1 | Chaz -> Enemy1 | 29644 | 17 | 17 (1 hit + 16 damage) | 0 |
| 1 | Enemy2 -> Hahn | 29789 | 19 | 18 (1 ability + 1 hit + 16 damage) | **1** |
| 1 | Hahn -> Enemy2 | 29885 | 17 | 17 | 0 |
| 2 | order pass | 30091 | 13 | 13 | 0 |
| 2 | Alys -> Enemy2 | 30097 | 18 | 18 (2 hit + 16 damage) | 0 |

The four hit rolls at Alys's first swing are `AlysKyraAttack_Init`
(`ps4.asm:13975-13976`) calling `loc_B6A2` a second time: `Character_Attack`
(`ps4.asm:13018`) runs the hit pass, and the swing's animation routine runs it
again, so a two-enemy swing rolls `Battle_CalculateChances` twice per target
and the later pair is what survives in `Fighters_Hit_Flags` (`loc_B6A2` presets
all nine to `$FF` before every pass). In round 2 only one enemy is left standing,
so the two passes cost one roll each. `psiv-core`'s `resolve_attack` drew that
pass once until this lane; it now draws both, for the two attackers the
cartridge sends to `CharAttack_AlysKyra` and for nobody else — see
SOURCE_NOTES.md, "The second hit pass of Alys's and Kyra's attack".
The roll left over at the enemy's turn is `Enemy_Attack` (`ps4.asm:19138`):
`loc_CFE6` (`ps4.asm:19146-19151`) re-rolls the ability while it equals
`$FFFFEEA8`, a word nothing clears (`grep` finds it written nowhere else), so a
first draw of zero in a battle whose RAM holds zero burns a second call.
`psiv-core` starts `last_ability_index` at `None`, so it never re-rolls the
first draw.

The remaining one is a draw-count divergence in the port, not in the fixture:
the engine's own documentation says the *values* may be substituted but the
*count* must not change (`rust/psiv-core/src/battle/engine.rs`, "Roll
accounting"). Fixing it is a rules change with its own consequences
(`last_ability_index` would have to carry the retail word across battles), so
this lane only pins it.

### Values, action by action

With each round fed the rolls of the roles the port models - the order pass,
both hit passes, the ability roll, the damage runs - every action matches:
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
order and with the surplus the log has, the port now matches Alys's whole swing
- both passes, both targets - and stops one action later, inside Enemy2's:

| | cartridge (log) | port, verbatim stream |
|---|---|---|
| Alys -> Enemy1 | normal, 12, HP 13 | normal, 12, HP 13 |
| Alys -> Enemy2 | normal, 10, HP 15 | normal, 10, HP 15 |
| rolls for the swing | 36 (f29489 x4, f29599 x32) | 36 (4 hit + 32 damage) |
| Chaz -> Enemy1 | normal, 15, kill | normal, 15, kill |
| Enemy2 -> Hahn | normal, **6**, HP 15 | **critical**, **10**, HP 11 |
| rolls for that action | 19 (f29789) | 18 (1 ability + 1 hit + 16 damage) |
| Hahn -> Enemy2 | normal, **5**, HP 10 | **miss**, HP 15 |
| rolls for round 1 | 102 | 85 |

`tape07_orders_its_rounds_the_way_the_cartridge_did` asserts everything before
that point - the priority draw, the round-1 queue, Alys's swing at both
enemies, Chaz's swing and its kill, and the enemy's target list - and then
asserts this divergence itself, so the ledger's first row is executable. The
Hahn row is the knock-on: one call missing at f29789 leaves the port's stream
one ahead from there on, so his hit roll reads what the log draws for damage.

**Reading, and what settles it.** The cause is the draw-count divergence above,
not a damage-formula error: the missing call at f29789 leaves the port one roll
ahead, so the enemy's hit roll reads the value the log drew for the *re-roll* —
`58235 & $3F` = 59 where the cartridge's own roll is `52339 & $3F` = 51. With
the enemy's dexterity of 8 against Hahn's agility of 4 that is `(59 + 4) * 2` =
126, past the `$74` critical threshold the cartridge's 110 stays under, so the
verdict changes as well as the sixteen-draw window, which starts one early
(sums 52 and 51). The evidence that the formula itself is right is the same
table twice over: with the modelled stream, every damage in the battle matches —
Alys's swing included, now that both passes are drawn — and on the verbatim
stream the round's count comes out 85 rather than 102, because the shift turns
Hahn's hit roll into a miss where the log has it landing 5. A *value*
consequence of a *count* divergence is exactly what the port's own
roll-accounting rule forbids.

`tape07_full_battle_diverges_at_alyss_swing_draw_count` is the same full-battle
assertion on one verbatim stream, parked behind `#[ignore]`. Its name and its
`#[ignore]` reason are the lane-before-this-one's: they name both causes, and
Alys's swing is no longer one of them — what it waits for now is the
`$FFFFEEA8` ability re-roll alone. Relabelling it belongs to whichever lane
closes that one out.

## What this does and does not prove

Proved: on the cartridge's own rolls, the port's battle resolution is exact -
queue, targets, verdicts, damages, HP, deaths, rewards - for the whole of tape
07's first basement battle, with the single remaining draw-count divergence
narrowed to a named routine (`Enemy_Attack`'s `$FFFFEEA8` re-roll). The second
one, Alys's hit pass, is drawn now: her swing is asserted on the *verbatim*
stream, not only on the modelled one.

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

1. **Fix the host's roll column** (`oracle/host/rng_trace.c`): subtract
   `seed_hi`, and change `oracle/rng_trace.py`'s `roll_for` to match, since it
   re-derives the same arithmetic. That changes the trace's sha256, which is
   why this lane left the column alone: its acceptance requires the capture to
   be O1's byte for byte. Until then, every consumer should derive rolls from
   the raw columns the way `oracle/battle_fixture.py` does.
2. **Decide the remaining draw-count divergence.** `last_ability_index` needs
   the retail `$FFFFEEA8` semantics (a word a battle does not clear) rather
   than a fresh `None`. It is a behaviour change with its own tests to write,
   and it is visible here as a count rather than as a wrong number. (The other
   one - `roll_hits` needing the swing's animation identity so Alys's and
   Kyra's attacks run `loc_B6A2` twice - is done: `resolve_attack` keys on the
   attacker's `Character_Stats` index, see SOURCE_NOTES.md "The second hit
   pass of Alys's and Kyra's attack".)
3. **Fixture the other traced battle.** Tape 09's battle (`oracle` "Battle
   ground truth") has a different formation, a critical and a different seed
   path; the same three steps - trace, extractor, replay - would widen this
   ledger from one battle to two.
