# Crawler POISON dispatch

Ability `$11` (17, POISON) was unsupported: a Caterpillr that rolled it made the
port emit `BattleEvent::UnsupportedAbility` and substitute a physical attack.
The crawler family's shared routine now reaches the same object path its `$10`
THREAD sibling uses, and the port applies the poisoned status through the one
chance roll the record describes.

## What the bytes show

`EnemyAttack_Crawler` (`ps4.asm:23081`) has three arms. Ability 0 loads the
plain-attack pair `BattleObj_CrawlerAtk` `$130` + `BattleObj_CrawlerAtk2` `$12C`;
`cmpi.w #$10, $24(a4)` (`ps4.asm:23108`) sends THREAD to `$134`
(`loc_1084A`, `ps4.asm:23113`); and `loc_10836` (`ps4.asm:23107`) sends every
other nonzero ability to `$138`. `BattleObjsGroup3Ptrs` (`ps4.asm:30968`) maps
`$138` to `BattleObj_Poison` (`ps4.asm:36279`), so `$11` is the third arm and
`Current_Target_Index` is left exactly where target selection put it.

`BattleObj_Poison` is `BattleObj_Thread` (`ps4.asm:36186`) with a different
sound and different artwork. Both initialise with `bset #7,(a4)` /
`clr.w $14(a4)` / `bset #1,$2(a4)` and then run the same three-entry phase table
— POISON's is `loc_1A8E4` (`ps4.asm:36300`), pointing at the shared `loc_1A572`
(`ps4.asm:36045`), its own `loc_1A8F0` (`ps4.asm:36304`) and the shared
`loc_1A816` (`ps4.asm:36240`). Phase 1 writes `SFXID_EnemyAttack4` (`$D8`,
`ps4.constants.asm:1012`) into `Sound_Index` (`ps4.asm:36310`) where THREAD
writes `EnemyAttack5` (`$DA`, `ps4.asm:36209`), then calls `loc_25474`
(`ps4.asm:49303`), which loads `d2` with the actor's `$24(a4)` ability, followed
by `GetEnemySkillEffectAndRange` (`ps4.asm:8687`) — once, at the animation
handoff. It then sets `Battle_Routine_2` to `$27`, sets `$FFFFEE82` and moves to
phase 8. The remaining differences from THREAD are cosmetic: `bset #4,$2(a4)`, a
palette write, and the frame counts (`loc_1A946` `02 06`, `ps4.asm:36324`,
against `loc_1A7E8` `04 0B`, `ps4.asm:36223`). No arm of the object, and none of
the animation helpers it calls (`loc_12618`, `loc_15E1C`, `loc_256AE`,
`loc_25474`, `loc_25978`, `QueueDMACommands`), writes a damage request or jumps
into the shared damage tails, so the crawler's turn produces the effect and
nothing else.

The same flag is a second reason `Battle_DoAttackEffect` never runs the attack
rider for that turn — it returns at `tst.b ($FFFFEE82).w` / `bne.w loc_5DDA`
(`ps4.asm:8554`) — but the decisive test is the ability field: `tst.w $24(a0)` /
`bne.w loc_5DD2` (`ps4.asm:8576`) leaves the routine as soon as the acting
fighter rolled a nonzero ability, which is exactly what `roll_enemy_ability`
mirrors by returning before `resolve_attack`.

`GetEnemySkillEffectAndRange` indexes the record as `id*8`: byte 0 is the effect
id and byte 2's low nibble is the range. Record 17 at `0x2833EC` is
`1b 01 08 40 01 0d 00 00` — effect `$1B`, STR, target 8, hit chance `$40`, STR
resistance, element 13 (efess) — and `8` is `AbilityRange_Single`:
`Current_Target_Index`, skipped when the slot carries the dead bits
(`Ability_ProcessRange`, `ps4.asm:8975`).

`AbilityEffectsOffs` (`ps4.asm:9036`) sends effect `$1B` to
`AbilityEffect_Poison` (`ps4.asm:9410`). Its first instruction tests
`StatusPoisoned` and returns immediately for a target that already has it, so an
already-poisoned target spends no roll at all. Otherwise it calls
`Battle_ProcessEffect` (`ps4.asm:9513`) with effect type 1 — the type
`GetEnemySkillEffectAndRange` installs — which reaches `Effect_DoEnemySkill` →
`Effect_SetupSkillParams` (`ps4.asm:9576`). That routine builds the roll from
the record: actor stat from byte 1 masked with `$7F` (strength), resistance stat
from byte 4 (strength), the target's element factor from byte 5 = 13, hit chance
from byte 3 = `$40`, and the effect id itself in `d5`. Element 13's factor is
stats offset `$2E + 2*13 = $48`, `efess_prop` (`ps4.constants.asm:48`) — the
same byte the handler passes in `d3`. `loc_6652` (`ps4.asm:9670`) then calls
`Battle_CalculateChances` (`ps4.asm:17338`): `v = (r + actor − target) *
element`, `r` in 0..=63, a miss when `v <= $40`, and an unreachable third arm
because `$40 > $1B`. One draw; `v > 64` lands the status, and a non-negative
verdict is what makes `AbilityEffect_Poison` set the poisoned bit.

Enemy 32 Caterpillr is the carrier: its ability list is
`00 00 00 00 11 11 11 11`, and `ENEMY_ABILITIES.md` §4 lists its 7 of 504
formations (pack ids `0x38`/`0x39`/`0x3a` are the 2/3/4-Caterpillr groups). The
sibling record `$24` PoisonMist shares the effect byte, both stat selectors and
the element and differs only in the hit-chance byte; it belongs to
`BattleObj_PoisonMist` and its own routine, so it stays out of this dispatch.

## What the port does

`enemy_skill::resolve_poison` (`rust/psiv-core/src/battle/enemy_skill.rs`) runs
from `roll_enemy_ability`, immediately after `resolve_thread` and before the
`UnsupportedAbility` fallback, so a dispatched POISON ends the enemy's turn
without reaching `resolve_attack`. The `is_poison` record predicate pins all
eight bytes: id 17, effect 27, STR, target 8, hit chance 64, STR resistance,
element 13. Any deviation — including PoisonMist's hit-chance byte — leaves the
record unsupported and keeps the diagnostic.

The status rule is the port's existing poison rule, not a second copy of it. The
chance maths is the shared `calculate_chances` kernel; the plain-attack rider
that already applied poison (`action.rs::resolve_enemy_attack_status`) calls the
same kernel with the forced `$70` hit chance of `Effect_DoPhysicalAttack`, while
this path passes the record's `$40`, and both read element 13's factor and pass
the effect id as the upper threshold. The status bit (`stats::status::POISONED`)
and the `BattleEvent::StatusInflicted` notification are likewise the ones the
rider already uses, so the Godot timeline's "poisoned!" narration covers this
path without new rendering.

Three rule details are deliberate:

- **An already-poisoned target draws nothing.** `AbilityEffect_Poison` returns
  before its call, and the resolver returns before `calculate_chances`.
- **An immune target still spends the draw.** A zero efess factor makes every
  roll a miss without skipping the roll, exactly as `loc_6652` runs before the
  verdict is read.
- **A failed roll emits nothing beyond `EnemySkillUsed`.** The cartridge stores
  no effect for a miss (`loc_6652` skips the store on a negative verdict), and
  the port's other poison path is equally silent. THREAD's resolver reports a
  failed roll as `Resolved { verdict: Miss, damage: None }`; a status attempt
  has no damage outcome to report and the object's own animation is still
  unimplemented, so no substitute cue is invented for it.

The object's wind-up sound is bound at the event level:
`battle_interim::battle_sound_events` maps `EnemySkillUsed { skill: 17 }` to
`$D8` at that event's index, as it already does for THREAD's `$DA`. This claims
the original sound id, not the object's frame timing.

## What it proves

`enemy_skill_poison_tests.rs` (six tests) covers the threshold boundary with
equal strengths and a factor of 2 — `r = 32` misses at `v = 64`, `r = 33` lands
— one draw, unchanged HP, and no attack/damage event; an already-poisoned target
consuming zero draws against an immune target consuming one and still not
applying anything; a missing, dead or enemy target consuming the ability with no
draw; the engine-level round showing `EnemySkillUsed` + `StatusInflicted` with no
`Attacked` and no `UnsupportedAbility`; and a failed dispatch for PoisonMist's
hit-chance byte and for another enemy id. The negative control is the same
crawler and the same ability id carrying that unproven record: it still emits
`UnsupportedAbility { ability: 17 }` and still swings physically.

`rust/psiv-runtime/tests/combat_enemy_poison.rs` drives pack formation `0x39`
(three Caterpillrs) from pack data with a fixed seed. The captured round log in
`build/lane-evidence/poison-formation-probe.log` shows the real record reached in
round 1: `EnemySkillUsed { actor: 7, skill: 17 }` with SFX `$D8`, then
`StatusInflicted { target: 1, status: 1 }` on Alys, the caster never appearing in
an `Attacked` event, and no `UnsupportedAbility` in any round. The regression
test then escapes, returns to the field, and asserts the field roster still
carries the poison bit and that a save round trip preserves it.

This is pack-level and core-level evidence. There is no native or original-
hardware capture for this change, and none is claimed: state results and any
future visual comparison stay separate.

## What remains unimplemented

- `BattleObj_Poison`'s artwork, its `02 06` frame table and the object timeline,
  including the phase-8 teardown (`clr.w (a4)`, `Battle_Routine = $16`) and the
  `Battle_Routine_2 = $27` handoff.
- The object's palette write and `bset #4,$2(a4)` sprite flag (cosmetic).
- The sibling `$24` PoisonMist and every other effect outside this record; the
  general effect dispatcher is still absent.
- Field poison consequences are unchanged and live elsewhere
  (`field_status.rs`): 1 HP per four steps on a poison map.
