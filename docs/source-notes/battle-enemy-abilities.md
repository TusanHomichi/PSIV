# Enemy abilities and damage routes

Scope: enemy damage skills and their targeting arms — which
records carry which effect, what the resolver does with them, and
the measured routes that settled each decision.

Index: [source and provenance notes](../../SOURCE_NOTES.md).

## Birth Valley enemy gameplay (2026-09-12)

`EnemyAttack_FlattrPlnt` selects Acid Breath for `$33`; the ROM ability at
`$2834FC` is `01 01 08 18 06 01 00 00`. `BattleObj_AcidBreath` guards its
single damage request with bit 1. Its child is animation-only.
`loc_B75A` supplies a normal hit without an accuracy draw, then
`Enemy_DamageCharacter` / `loc_26DA` / `loc_26F8` select battle strength,
defense and physical resistance. The existing 16-draw damage formula applies.
The objects write MoleAttack `$D5` and EnemyAttack4 `$D8`; native cues are
event-aligned, with the original object timings/art still pending.

Acid Breath's remaining carriers (2026-09-23). `EnemyAttackOffs` (`ps4.asm:19206`)
routes 75 FlattrPlnt, 76 FlyScreamr and 77 TechPlant to `EnemyAttack_FlattrPlnt`
and 85 Piercer and 86 HakenLeft to `EnemyAttack_Piercer` (`ps4.asm:21518`).
`generated/enemies.json` holds ability 51 in the regular lists of 75, 76, 85 and
86 only. That `$33` arm (`loc_F5BE`) has no enemy-id test and no write to
`Current_Target_Index` — unlike its `$34`/`$2A` arms and its `loc_F722` fallback —
so 75 and 76 run the same object chain against the drawn party target.
`EnemyAttack_Piercer`'s `$33` arm (`loc_F2A0`, `ps4.asm:21532`) also leaves
`Current_Target_Index` alone, but loads a different chain: object `$35C`
(`loc_23998`, `ps4.asm:47264`) with the target in `$38(a1)`, then child `$360`
(`loc_24FD2`, `ps4.asm:48883`), copied by `loc_24A20` (`ps4.asm:48452`). The
child makes the reaction write (`move.w #5, $2(a3)`, `$1C = $E`) and raises
`($FFFFEE80)` when it ends; `loc_23AB6` (`ps4.asm:47344`) then makes the same
single damage request as `BattleObj_AcidBreath`'s `loc_24AEC` (`ps4.asm:48507`)
exit — `move.w #$C, $2(a3)` into `Fighter_TakeDamage` (`ps4.asm:3564`), guarded
by bit 7 so it fires once, with `($FFFF416C)` as the handshake. Piercer's arm
writes `SFXID_MoleAttack` `$D5` and then `SFXID_EnemyAttack4` `$D8`;
FlattrPlnt's writes the same pair. `Enemy_DamageCharacter` reads the shared
`EnemySkillData` record, so both arms take the caster's own strength (88
FlyScreamr, 128 Piercer, 164 HakenLeft), the target's defense and its physical
resistance through the same formula, and neither adds a physical attack or a
status. Native: `ACID_BREATH_CARRIERS` in `rust/psiv-core/src/battle/enemy_skill.rs`
now covers 75/76/85/86; 77 TechPlant stays out because its US list never rolls
`$33`, so the gate cannot be reached by an unproven carrier. (2026-09-24: that
constant and the per-record predicate are gone — the gate is now the
`$33`/`$02` `DAMAGE_SKILL_ROUTES` table in
`rust/psiv-core/src/battle/enemy_damage.rs`, with the same four carriers; see
the FLAME BOLT section below.)

`loc_5A98` tests target status `$C4` and `loc_5ACE` redraws from living party
members before the enemy ability roll. `Character_Dead` at `loc_84DE` masks
status with `$10`, then sets bit 6 for androids or bit 2 for humans.
`Battle_DoAttackEffect` only dispatches a plain attack's status after damage
and a successful hit. All nonzero enemy attack-status records are 27/28.
`AbilityEffect_Poison` / `AbilityEffect_Paralyze` pass target offset `$48`;
`Effect_DoPhysicalAttack` compares battle strength to battle strength with
threshold `$70`. Existing poison/paralysis skips the draw, while immunity
still runs it. Paralysis clears sleep bit 3 and sets agility/dexterity to 1.

## Enemy damage-skill routes and FLAME BOLT `$02` (2026-09-24)

`EnemySkillData` `$02` FLAME BOLT is `01 01 08 50 07 03 00 00` at `$283374`:
effect `$01`, stat `$01` (strength), tgt 8, pow 80, res `$07` (magic defense),
el `3` (fire). `EnemyAttackOffs` (`ps4.asm:19206`) gives it two carriers with
their own entries, `$00` (`ps4.asm:19207`) and `$05` (`ps4.asm:19212`);
`generated/enemies.json` holds `$02` in all eight regular slots of 0 Helex and
in slots 5-8 of 5 ForcedFly, and nowhere else, so 14 of the 504 regular
formations can roll it.

`EnemyAttack_ForcedFly` (`ps4.asm:23567`) branches to
`EnemyAttack_MonsterFly` (`ps4.asm:23591`) only for ability 0 and otherwise
falls through into `EnemyAttack_Helex` (`ps4.asm:23574`), which writes object
`$48` (line 23575) — `BattleObj_HelexFlameBolt` (`ps4.asm:30279`). Neither arm
writes `Current_Target_Index`; `Enemy_Attack` (lines 19168-19172) had stored the
drawn target in the attack object's `$38(a1)` before dispatching, so both run
against the chosen party member. The parent only animates: at `$10(a4) == 2`
it loads `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`) and copies `$38`/`$3C`
into it (lines 30301-30302). The child waits out `$2E` = `$110`, makes the
hit-reaction write (`move.w #5, $2(a3)`, `$1C = $C`, lines 30334-30335) and
then the object's only damage request, `move.w #$C, $2(a3)` (`ps4.asm:30342`),
behind a `btst #1, $4(a4)` / `bset #1, $4(a4)` once-guard with `($FFFF416C)` as
the handshake — one reaction and one hit, the shape of
`BattleObj_AcidBreath`'s `loc_24AEC` exit (`ps4.asm:48507`). The arm writes
`SFXID_EnemyAttack3` `$D7` at the parent's load (line 30285) and
`SFXID_FireBreath` `$C2` at the child's (line 30321); the port maps them to the
ability event and the reaction, like Acid Breath's `$D5`/`$D8`.

That request enters `Fighter_TakeDamage` (`ps4.asm:3564`) and
`Figher_DamageCheckActor`, which takes `Enemy_DamageCharacter`'s
`$24(a3) != 0` branch (`ps4.asm:3775`, `loc_26D2`): record byte 1 masked with
`$7F` (`Effect_SetupSkillParams`, `ps4.asm:9580`) selects the caster's stat,
byte 4 is read raw (line 9605) as the target's stat, byte 5 indexes `loc_276A`'s
element offsets for the target's factor, and byte 3 becomes the doubled bonus
of `Battle_CalculateDamage` (`ps4.asm:17374`) under `loc_266C`'s `1..=999`
clamp. No accuracy roll, no status, no extra physical attack — the same
pipeline `$33` uses, so Helex's strength 10 and ForcedFly's 154 are the only
carrier-specific terms.

Native: `resolve_damage_skill` and `DAMAGE_SKILL_ROUTES` in
`rust/psiv-core/src/battle/enemy_damage.rs` replace the `is_acid_breath` record
predicate and the `ACID_BREATH_CARRIERS` gate that used to live in
`enemy_skill.rs`; `engine::roll_enemy_ability` calls the new resolver. The gate
is now the exact `(enemy, ability)` pair, with each table entry citing the
`EnemyAttackOffs` line, the arm, the object and the single damage-request line
that prove it: `$33` for 75/76/85/86 and `$02` for 0/5. The record supplies the
arithmetic, byte 1 is masked with `$7F` as the cartridge masks it, and a record
whose byte 2 is not the proven single-target range (`AbilityRangeOffs`,
`ps4.asm:8903`, index 8 = `AbilityRange_Single`, `ps4.asm:8938`) is refused
even when its pair is listed, so it keeps the explicit `UnsupportedAbility`
path. Acid Breath's behaviour is unchanged: same events in the same order and
the same 16 damage draws.

## The damage-skill gate and the Motavia single-target routes (2026-09-24)

`resolve_damage_skill` (`rust/psiv-core/src/battle/enemy_damage.rs`) used to
accept a listed `(enemy, ability)` pair only when the record's byte 2 was 8. That
was the wrong criterion. Byte 2's low nibble is read by
`Ability_GetEffectAndRange` (`ps4.asm:8886`; the nibble at line 8895), stored in
`Battle_Ability_Range` and dispatched through `AbilityRangeOffs`
(`ps4.asm:8903`) to `AbilityRange_Varied`/`Single`/`Self`/`MultiEnemies`/
`MultiChars` (`ps4.asm:8920-8962`). `Ability_ProcessRange` (`ps4.asm:8975`)
calls the **effect handler** once per fighter the chosen handler selects — and
these records' handler is `$01`, `AbilityEffect_None` (`ps4.asm:9092`), a bare
`rts`. The nibble therefore multiplies nothing, while the HP change comes from
one place only: the arm's object writing `move.w #$C, $2(target)`
(`Fighter_TakeDamage`, `ps4.asm:1033`), which for an enemy actor reaches
`Enemy_DamageCharacter` (`ps4.asm:3775`) and reads the actor's own ability slot
plus the record. That is why 25 nibble-8 records and 33 nibble-9 records are both
`single` when their chains make one request, and why the gate is now the traced
route rather than a record byte.

The replacement gate has two parts. `(enemy, ability)` must be in
`DAMAGE_SKILL_ROUTES`, and the record's effect byte must be `$01`: the resolver
models the one damage request, so a record whose handler would also do something
(effect `$02` is `AbilityEffect_Death`, `ps4.asm:9098`) stays on the caller's
`BattleEvent::UnsupportedAbility` path, drawing nothing and emitting nothing.
Each `DamageRoute` also carries a `DamageClass`; `Single` — exactly one `#$C`
request against the object's `$38`, i.e. the drawn `Current_Target_Index` that
`Enemy_Attack` stored at line 19172 — is the only class the table carries and the
only one the resolver implements. An all-party chain (five requests in
`loc_24A9E`, `ps4.asm:48483`, or `loc_24BB6`, `ps4.asm:48562`) needs its own
class and its own branch; the resolver's `match` is exhaustive so adding the
variant fails to compile until it has one.

Twenty-one Motavia pairs were read from scratch for this change, each from its
`EnemyAttackOffs` entry through the arm, the object chain and the single guarded
request, and none disagreed with `docs/ENEMY_DAMAGE_ROUTES.md` §3 — no pair was
dropped. `$2E` GIWAT: 71 FrostSaber `loc_F8F2` (`ps4.asm:21987`, object `$2B4` =
`loc_1D5AA`, request at line 40042), 77 TechPlant `loc_F6C4`
(`ps4.asm:21846`, `$2FC` = `BattleObj_EnemyGiwat` + the visual `$2F0`, request at
48513), 91 HewGilla (the else arm `loc_F108`, `ps4.asm:21426`, `$390` =
`loc_22670`, request at 48529), 101 DarkWitch `loc_EE0E` (`ps4.asm:21229`,
`$3C4` = `loc_21852`, request at 45310 in the shared `loc_21D08`), 122 DElmLars
and 123 XeAThoul `loc_DFBE` (`ps4.asm:20300`, `$7B8` = `loc_28F76`, request at
48474). `$37` SAND STORM 81 DesrtLeach and `$39` MAELSTROM 82 Leviathan:
`EnemyAttack_SandWorm`'s `loc_F48A` (`ps4.asm:21692`, `$328` =
`BattleObj_SandStorm`) and else arm `loc_F4FA` (`ps4.asm:21720`, `$338` =
`BattleObj_Maelstrom`), both reaching `loc_24B64` (request at 48547) — one
request each despite tgt 9. `$3F` FLODBREATH: 90 Depcen `loc_F13C`
(`ps4.asm:21443`, `$380` = `BattleObj_FlodBreath`), 91 HewGilla and 92 Elmelew
the `$3F` arm of `EnemyAttack_HewGilla` (`ps4.asm:21395`, `$384` = `loc_22A90`),
all reaching `loc_24B20` (request at 48529), each chain writing MoleAttack `$D5`
then EnemyAttack4 `$D8`. `$40` WAT: 91/92 `loc_F0CC` (`ps4.asm:21412`, `$388` =
`BattleObj_EnemyWat`, request at 48529), 99 TechUser and 100 TechMaster
`loc_EDC4` (`ps4.asm:21211`, `$3C0` = `loc_218D6`, request at 45310), 114 Juza
`loc_E3DE` (`ps4.asm:20593`, `$744` = `loc_2B006`, request at 56553).
`$44` FOI: 99/100 `EnemyAttack_TechUser`'s first arm (`ps4.asm:21156`, `$3B0` =
`loc_21BF0`, request at 45310), 114 `EnemyAttack_Juza`'s first arm
(`ps4.asm:20575`, `$740` = `loc_2B08E`, request at 56553). `$6D` ROUND EYES 147
Rappy and `$6E` LOVEL EYES 148 BlueRappy: `EnemyAttack_Rappy`
(`ps4.asm:19578`) turns the attack object itself into `BattleObj_RoundEyes`
(`ps4.asm:67760`, the `$6D` arm at line 19590) or `BattleObj_LovelEyes`
(`ps4.asm:67729`, the else arm at line 19593) and reaches `loc_D200`
(`ps4.asm:19395`) for the parent pointer; phase 8 jumps to `loc_24B20` (lines
67775 and 67744) for the one request at 48529. All eight records are effect
`$01`, so every pair passes the new gate.

Stat width, confirmed rather than changed: the two readers of record byte 1 —
`Enemy_DamageCharacter`'s enemy-skill branch (`ps4.asm:3775`, table `loc_275A`
at `ps4.asm:3892`) and `Effect_SetupSkillParams` (`ps4.asm:9576`, table
`AbilityStatsOffs` at `ps4.asm:9619`) — mask the byte with `$7F` and then compare
the selected offset against `$26`: `atk_pow_battle` `$26`, `dfs_pow_battle` `$2A`
and `magic_dfs_battle` `$2E` are words, `$01`..`$04` (strength, mental, agility,
dexterity at `$1A`, `$1D`, `$20`, `$23`, `ps4.constants.asm:19-34`) are bytes.
The ROM's table at `$275A` is `00 00 00 1a 00 1d 00 20 00 23 00 26 00 2a 00 2e`,
which is the same selection. `technique::stat` already returns the whole word
(`StatPair::battle`, a `u16`) for selectors 5-7 and a byte zero-extended for 1-4,
so it needed no change; the resolver masks byte 1 with `STAT_INDEX_MASK` (`$7F`)
and reads byte 4 raw, and both are now pinned by tests — 81 DesrtLeach's attack
286 is the selector-5 case, and it produces a different number if read as a byte.

Sound: the objects' explicit writes are mapped in
`rust/psiv-runtime/src/battle_interim.rs` the way FLAME BOLT's and Acid Breath's
are. Mapped — MoleAttack `$D5` as the wind-up cue for `$37` (line 48228), `$39`
(line 47800), `$3F` (lines 46343 and 46220), `$40` (lines 46002, 45056, 56445),
`$44` (lines 45238, 56485) and `$6D`/`$6E` (line 67787, the shared `loc_347BE`);
EnemyAttack4 `$D8` at the `$3F` reaction (lines 46366, 46234); TechCast `$BB` at
FOI's request phase (lines 45261, 56509); EnemyAttack1 `$BA` at the eyes' flinch
(line 67795). Left unmapped, listed here rather than invented: every cue of
`$2E`, because its carriers' chains disagree — `$D5` for 71/101/122/123
(lines 39973, 45022, 54235), TechCast `$BB` for 71 and 77 (lines 39987, 38234)
and nothing at all for 91 (`loc_22670`, `ps4.asm:45925`); and WAT's `$BB` at
`BattleObj_EnemyWat` (line 46016), which 91/92 write but `loc_218D6` and
`loc_2B006` do not. The cue table is keyed on the skill and keyed per acting
fighter only for the plain-attack sound, so a skill-wide cue would play for a
carrier whose object writes nothing; a per-carrier cue needs the carrier's
identity in the sound context first.

Tests and evidence. Core: `enemy_damage_tests.rs` gains one test per ability
pinning the exact damage from the record (`giwat_reads_the_masked_mental_selector_for_every_carrier`
through `lovel_eyes_reads_the_attack_word`), the stat-width test
`the_stat_selectors_read_a_word_for_attack_defense_and_magic_defense`, the
round-level `every_motavia_route_resolves_in_an_ordinary_round` (all 21 pairs,
30 draws, no swing), the flipped refusal control
`a_listed_route_resolves_with_its_record_on_the_nibble_9_target` and the new
refusal `a_listed_route_with_an_effect_handler_is_refused`. Runtime
(`rust/psiv-runtime/tests/combat_enemy_attacks.rs`):
`motavia_single_target_abilities_resolve_in_their_real_formations` drives
formation `$53` (one DesrtLeach, which resolves `$37`) and formation `$2A` (two
TechUsers, which resolve `$40` and `$44`, both seen in the eight-round search at
the fixed seed) with no `UnsupportedAbility` for those abilities and no swing by
the caster. The negative control for the gate: deleting one new route entry
(81 DesrtLeach's `$37`) fails `sand_storm_reads_the_attack_word_on_a_nibble_9_record`,
`every_motavia_route_resolves_in_an_ordinary_round` and the runtime formation
test, and restoring it passes all three again.

Deviations worth recording. (1) One pre-existing test changed with the gate:
`acid_invalid_definition_or_dispatcher_cannot_silently_damage_or_spawn` used
`skill.target = 9` as its "invalid definition" case, which is exactly the check
this change removes, so that case now moves the effect byte instead (`$02`). Its
other case (an unlisted carrier) and every other Acid Breath and FLAME BOLT test
are untouched. (2) `docs/ENEMY_DAMAGE_ROUTES.md`'s parenthetical notes of the
form "`bne.s loc_X` at line N" cite the test line (`cmpi.w`/`tst.w`) rather than
the branch line, one line later; the code comments here cite label lines and
request lines, which were verified against the file.

## FloatMine2's regular Fission2 roll (2026-09-24)

**RETAIL FINDING — `EnemyAttack_FloatMine`'s fall-through spends the turn.**
`EnemyAttackOffs` `$32` (line 19257) sends 50 FloatMine2 to
`EnemyAttack_FloatMine` (`ps4.asm:22675`), whose only arms test `$24(a4)` for
`$14` (line 22677), `$18` (22690), `$19` (22707) and `$1A` (22766). Its eight
regular slots are `07 07 07 07 17 17 17 17` (`$58(a3,d0.w)`, line 19153 in
`Enemy_Attack`, `ps4.asm:19138`; `generated/enemies.json` id 50): the index roll
always lands on one of those two, neither of which is an arm, so this enemy
never loads an attack object at all and reaches the fall-through `loc_10406`
(`ps4.asm:22781`) every time:

```
	movea.l	$38(a1), a0
	clr.w	(Current_Target_Index).l
	clr.w	$24(a4)
	move.w	#$16, (Battle_Routine).l
	subq.w	#2, $2(a4)
	rts
```

No object, no `LoadPLC1`, no palette write, no `Sound_Index`, no message window.
`$16` is `Battle_DoAttackEffect` (`ps4.asm:8553`; table entry at line 7536,
`BattleRoutines` `ps4.asm:7524`); the ability word it reads is the one just
cleared and `loc_B6A2` (`ps4.asm:17492`) left all nine `Fighters_Hit_Flags`
(`$FFFF4150`, `ps4.constants.asm:2019`) at `$FF`, so it takes `loc_5DD2`
(`ps4.asm:8625`) → `Battle_Routine` `$12` = `loc_6672` (`ps4.asm:9689`) → `$1E` =
`loc_66B8` (`ps4.asm:9709`). `Ability_GetEffectAndRange` is never called for that
turn: no damage, no status, no ailment, and the `$12`/`$1E` pair does the ordinary
end-of-action wait and advances the turn order. The actor's `fighter_routine`
drops 6 → 4 (`Fighter_DoNothing`) exactly as the shared `loc_D200`
(`ps4.asm:19395`) tail does after a real attack, so the turn is *spent*, not
skipped. `$17` Waiting is the same path for 44 FloatMine, 46 VopalSphre and 50
FloatMine2 — which is what the ability's name says — while `$19` on 45 CommndBall
(the routine's fourth carrier) has an arm and is unaffected.

**Draw accounting — nothing after the ability index.** `Enemy_Attack` spends the
index roll (`loc_CFE6`, rerolling the index only), then the AI instruction block
at `$50(a3)`..`$53(a3)`; all four of FloatMine2's bytes are 7 =
`EnemyAI_PhysicalAtkReceived` (`ps4.asm:22816`, table `ps4.asm:19364`), which
draws nothing and only replaces the rolled id when `reaction_flags` bit 0 is set.
Back in `Enemy_Attack`, `loc_B6A2` clears the flags and, with
`Current_Target_Index` now 0, runs its slot loop **once** for `d6 = 0`
(`moveq #0, d7` then `bpl.s loc_B6D4`): `Battle_GetFighterAddr(0)` is
`Obj_Fighters - $40` = `$FFFF43C0`, the phantom slot 0 of the 1-based shadow
array `loc_5C04` (line 8454) indexes into. The battle start wipes
`$FFFF4000`-`$FFFF47FF` (lines 9989-9993; `Trap00Exception`, line 161, clears
`d7+1` longwords), and every write to that array in the disassembly lands at
`$FFFF43C0 + n*$40` with n ≥ 1 (`loc_2515C`, line 49017, pre-increments;
`loc_2A88`, line 4241, starts at `$40(a1)`), so the phantom slot reads 0: empty
slots consume no chance roll, and the verdict byte lands in
`Fighters_Hit_Flags[-1]` (`$FFFF414F`), which nothing reads. Nine ordering rolls,
four enemy-target rolls, the ability index — then nothing.

**Fork check.** No `if bugfixes`/`if grand_cross` arm exists between line 18356
and the routine at 22675, nor inside it: the body above is the disassembly's one
unconditional body, so there is no alternative arm a retail build would take and
no fork-only branch to strip. The enemy ability bytes and the routine's arm ids
agree with the ROM-extracted `generated/` tables. No emulator, tape or hardware
capture backs this entry, and none is claimed: it is a static read of the
disassembly plus the extracted records.

**PORT CHANGE.** `enemy_skill::resolve_no_effect_turn` now witnesses that
fall-through: it requires the record to be `$07` Fission2 (`is_fission`) or `$17`
Waiting (`is_waiting`, record 23 = `22 00 00 00 00 00 00 00` at `0x28341C`) and
the actor's enemy id to be one of the four `EnemyAttack_FloatMine` carriers (44,
45, 46, 50), emits `BattleEvent::EnemyAbilityWasted { actor, ability, name }` and
clears the actor's ability slot (`clr.w $24(a4)`), so the physical fallback no
longer runs for those two ids. 12/13 Fission and everything else keep their
existing path, `fission_neighbor` included. Evidence: core
`enemy_skill_tests::floatmine2_rolling_fission2_spends_the_turn_without_an_effect`
(14 draws for the round; no `Attacked`, `Resolved` or `UnsupportedAbility`),
`waiting_spends_the_turn_for_every_float_mine_carrier` (44/46/50),
`the_no_effect_witness_needs_the_traced_record_and_carrier`, and the negative
controls `an_unproven_float_mine_arm_still_falls_back_to_a_physical_attack` and
`a_non_carrier_keeps_the_fallback_for_the_fission2_roll`; runtime
`combat_fission::floatmine2_formations_spend_fission2_and_waiting_turns_without_a_swing`
drives pack formation 263 (FloatMine2, Tower, FloatMine2 — Tower's slots are all
zero) and sees both `$07` and `$17` in the trace, and
`a_float_mine_arm_on_a_real_formation_keeps_the_physical_fallback` pins formation
292 (45 CommndBall's `$19`).

**Two deliberate limits.** (1) The port emits one event per turn and models no
per-frame timing; `loc_6672` also sets the `$FFFF418A` wait to `$F` when the
ability slot is empty, and `loc_66B8` decrements it once a frame, so retail
advances the turn sixteen frames later than after an attack. That is pacing from
the unimplemented object-timing layer (`docs/BATTLE_ANIMATIONS.md`), not a rule.
(2) `EnemyAI_PhysicalAtkReceived` and
`reaction_flags` are not modelled: a FloatMine2 hit physically since its last
action has its rolled `$07` replaced in retail by the `$18` Explosion conditional
(`$54(a3)`), an arm whose object is still untraced, so the port spends the turn
there too. `docs/ENEMY_ABILITIES.md` records both limits under Port gaps.

## All-party enemy damage routes: SPIRAL BLD `$08` and EARTHQUAKE `$38` (2026-09-24)

`resolve_damage_skill` (`rust/psiv-core/src/battle/enemy_damage.rs`) resolved one
`move.w #$C` request against the object's `$38`. `docs/ENEMY_DAMAGE_ROUTES.md` §2
classes 58 of its 146 pairs `all-party` — one request per party slot from a
`moveq #4, dN` loop over `Obj_Fighters` — and this change adds that class and the
three Motavia pairs of it: 15 Fanbite `$08` SPIRAL BLD, 80 SandWorm `$38`
EARTHQUAKE and 149 KingRappy `$38` EARTHQUAKE.

Records. Both abilities share the eight bytes `01 05 09 00 06 01 00 00`:
effect `$01` (`AbilityEffect_None`, `ps4.asm:9092`), stat `$05` (attack, which
both readers take as a word), target nibble 9, power byte 0, resistance `$06`
(defense), element `1` (physical). `$08` is at `$2833A4` and `$38` at `$283524`
(`EnemySkillData` at `ps4.asm:319311`, `generated/enemy_skills.json`). Power byte
0 means the doubled bonus term of `Battle_CalculateDamage` (`ps4.asm:17374`) is
zero: the number is the carrier's attack word through the element multiply minus
the target's defense.

Carriers and arms. `generated/enemies.json` gives ability 8 to 15 Fanbite alone
(slots 7-8, 6 of the 504 regular formations: 54, 55, 63, 64, 65, 66) and ability
56 to 80 SandWorm (slots 6-8, regular formation 59) and 149 KingRappy (slots 3-4,
one boss formation). Each pair's arm clears `Current_Target_Index` before
loading its object, which is what makes the five-slot loop lethal to more than
the drawn target:

- 15 Fanbite: `EnemyAttackOffs` `$0F` (`ps4.asm:19222`) →
  `EnemyAttack_Locusta` (`ps4.asm:23472`) has no ability-id test at all — after
  the three approach objects, `tst.w ability(a4)` (line 23486) picks the
  nonzero-ability body, which writes `$FFFF` to `Current_Target_Index`
  (line 23488), loads object `$A0` into the second object bank (line 23492,
  `BattleObjsGroup2Ptrs` line 26770) = `BattleObj_LocustaSpiralBld`
  (`ps4.asm:29175`) and copies the stored target pointer into its `$38`
  (line 23491). Its state 4 (`loc_1468A`, `ps4.asm:29255`) walks in until
  `$2C(a4) >= $1BF`, then ORs the five party slots' `$1C` timers (lines
  29266-29274) and, only when all of them read zero, writes `#$C` to the five
  slots from `$FF4400` (lines 29275-29280) and sets `($FFFF416C)`. On the way it
  flinches each living member it passes (`$1C = $C`, routine 5, line 29316) in
  the `loc_147A0` walking order (line 29334: slots 4, 2, 1, 3, 5) and writes
  `SFXID_EnemyAttack1` `$BA` per flinch (line 29314); the wind-up wrote
  `SFXID_Slasher` `$B7` once (line 29206).
- 80 SandWorm: `EnemyAttackOffs` `$50` (`ps4.asm:19287`) →
  `EnemyAttack_SandWorm` (`ps4.asm:21658`), arm `loc_F4D4` (line 21710) tests
  `$38` (line 21711), clears the index (line 21713) and writes object `$330` into
  the attack object itself (line 21718; `BattleObjsGroup5Ptrs` line 43704 =
  `BattleObj_Earthquake`, `ps4.asm:47884`). Its state table `loc_2427A`
  (`ps4.asm:47901`) reaches `loc_243C8` (line 47992) for the request: five
  `#$C` writes from `Obj_Fighters` (lines 47995-48000) plus `($FFFF416C)`, then a
  wait for that flag before the follow-through. It writes `SFXID_GraveOpening`
  `$DD` every fourth frame while the ground shakes (lines 47917, 47954, 47961).
- 149 KingRappy: `EnemyAttackOffs` `$95` (`ps4.asm:19356`) →
  `EnemyAttack_KingRappy` (`ps4.asm:19596`), arm `loc_D516` (line 19608) clears
  the index (line 19609) and writes `$904` (line 19610;
  `BattleObjsGroup10Ptrs` line 66419 = `BattleObj_KingRappyEarthquake`,
  `ps4.asm:67513`). Its state table `loc_34428` (line 67523) sends state `$C`
  straight to `jmp (loc_24BB6).l` (line 67527) — the shared all-party tail at
  `ps4.asm:48562`, whose `$1C` gate (lines 48565-48571) the object has just
  satisfied by clearing all five party timers itself (lines 67603-67607). The
  tail's five `#$C` writes are at lines 48572-48577 and its flag at line 48578.
  The wind-up writes `SFXID_GraveOpening` (line 67567) and loads sound object
  `$8EC` carrying `SFXID_Slasher` (lines 67541-67544).

What the class means, each from the instruction that decides it. The survey's
three rows are unchanged; `docs/ENEMY_DAMAGE_ROUTES.md` §3 now records these four
readings in full.

- **Slots.** All five party slots get the `$C` write, empty and dead included,
  but `Battle_UpdateFighters` (`ps4.asm:987`) runs no routine for a slot whose
  object word is zero, and `Fighter_TakeDamage` (`ps4.asm:3564`) returns early
  (`move.w #4, $2(a4)`, line 3603) on a negative `Fighters_Hit_Flags` byte
  (`ps4.constants.asm:2019`). `loc_B6A2` (`ps4.asm:17492`) fills that array
  after the arm ran: with the index cleared, the enemy-actor branch takes
  `d7 = 4, d6 = 1` (lines 17510-17511) so the loop covers slots 1-5, and
  `loc_B75A` (line 17559) writes 0 for a slot whose status carries neither
  `StatusDead_Mask` `$04` nor `StatusAndroidDead_Mask` `$40`
  (`ps4.constants.asm:77-81`) and `$FF` for one that does. `loc_B6D4`'s
  `cmpi.b #6` combo escape (line 17513) does not divert an enemy turn: the
  command word is the enemy's own `$0100` entry by then (`loc_56A8`,
  `ps4.asm:7928`, writes it at line 7932, and `loc_576A`'s
  `move.w d1, ($FFFF4146)` at line 8051 copies it into `Current_Command`), so its
  high byte — the command index the `cmpi.b` reads — is 1, and
  `tst.w $24(a4)` (line 17515) sends every nonzero ability to no-roll
  `loc_B75A`. An empty or out slot therefore takes no hit and draws nothing.
- **Order and frames.** `Battle_UpdateFighters` walks `Obj_Fighters` upward by
  `obj_size` `$40` for twelve slots, one routine call each, once per frame
  (`GameMode_Battle`, `ps4.asm:947`, calls it at line 953;
  `GameMode_LoadBattle`, `ps4.asm:9911`, at line 10027). The request phase writes all five slots in one frame, so all
  five `$C` routines — and with them all five `Enemy_DamageCharacter`
  (`ps4.asm:3775`) calls and their sixteen `Battle_CalculateDamage` draws each —
  run in the same frame in slot order 1-5.
- **Per target.** `Enemy_DamageCharacter` reads the resistance stat and the
  element factor from `a1`, the taking fighter's own stats
  (`Figher_DamageCheckActor`, line 3737), and Defend is that fighter's own
  physical property; one member defending changes one number.
- **Deaths.** `Fighter_TakeDamage` only computes. The hit points come off in
  `FighterShowDamage_DecreaseHP` (`ps4.asm:3640`), which routine `$D` reaches
  through `loc_295C`'s third phase, three window phases after the request
  (`Fighter_OpenDamageWindow`, `ps4.asm:3587`, and `loc_24BE`, line 3622, which
  drops routine `$E` to `$D`). Every fighter advances one phase per frame, so
  all five reach the subtraction in a later, single frame, after every roll has
  been drawn: a member that empties its HP there cannot stop a later member's
  computation.

Native. `DamageClass::AllParty` joins `Single` in `DAMAGE_SKILL_ROUTES`, and
`resolve_damage_skill` splits into a class dispatch plus one per-target helper
that both classes share — so an all-party hit computes exactly what a
single-target hit computes for the same target. `AllParty` ignores `intended`
(the arm cleared the index) and walks every occupied, living party slot in
`Roster::side(Side::Party)` order, which is `Battle_UpdateFighters` order, one
`Resolved` per slot after one `EnemySkillUsed`. The effect-`$01` requirement is
unchanged and is tested for the new class too.

Deviations, all recorded rather than silent. (1) The cartridge defers all five
hit-point subtractions to the show-damage frame and the port applies each
target's damage as it resolves it; the draws, the order and the target set are
identical, and no later target's computation reads an earlier target's hit
points, so nothing observable changes. (2) The port has no per-frame object
timeline, so each object's cues are attached to the event: SPIRAL BLD's
`SFXID_Slasher` `$B7` and EARTHQUAKE's `SFXID_GraveOpening` `$DD` ride
`EnemySkillUsed` (the GraveOpening repeat every fourth frame of the tremble is
not reproduced), and SPIRAL BLD's per-flinch `SFXID_EnemyAttack1` `$BA` rides
that member's own `Resolved` — one cue per living member either way, but in the
resolver's slot order rather than the object's walking order (4, 2, 1, 3, 5).
(3) KingRappy's `SFXID_Slasher` is left unmapped: only that chain loads sound
object `$8EC`, SandWorm writes nothing there, and the sidecar's only per-fighter
key is the plain-attack sound, so a skill-wide `$B7` would play for a carrier
that never wrote it — the same reason WAT's TechCast is unmapped. (4) `$08`'s
`$B7` is safe as a skill-wide cue because 15 Fanbite is that ability's only
carrier.

No question in this class was left open: the three pairs are implemented, and
`docs/ENEMY_DAMAGE_ROUTES.md` §3 carries the same four readings with their
citations.

Tests and evidence. Core: `enemy_damage_all_party_tests.rs` (registered from
`enemy_damage.rs` with `#[path]`, so `enemy_damage_tests.rs` gains nothing and is
otherwise untouched) holds nine tests —
`spiral_bld_hits_every_living_party_slot_with_its_own_roll_and_order` (three
targets, three different defense words and three different physical factors,
48 draws, one `Resolved` per slot in slot order),
`the_drawn_target_does_not_narrow_the_all_party_loop`,
`a_dead_party_slot_takes_no_hit_and_draws_nothing` (32 draws, no event for the
dead slot), `defending_one_member_changes_that_members_number_only`,
`a_death_mid_loop_leaves_later_targets_computed` (48 draws, `Died` between the
first and second `Resolved`), `every_all_party_pair_deals_its_own_record_number`,
`a_listed_all_party_route_with_an_effect_handler_is_refused`,
`an_unlisted_pair_for_the_same_two_records_is_refused` and
`every_all_party_route_resolves_in_an_ordinary_round` (62 draws for three living
members: 13 ordering and target draws, the ability index, 48 damage draws; no
`Attacked` and no `UnsupportedAbility`). Runtime
(`rust/psiv-runtime/tests/combat_enemy_attacks.rs`):
`motavia_all_party_abilities_resolve_in_their_real_formations` drives formation
`$37` (two Fanbites, `$08`) and formation `$3B` (one SandWorm, `$38`) through
`start_battle_timeline` at fixed seeds, checks every ability use in twelve
rounds, and requires one `Resolved` per party member in slot order with the
object's wind-up cue (`$B7` / `$DD`) on the `EnemySkillUsed` event and no swing
by the caster. KingRappy's `$38` has no regular formation (its only one is a boss
formation, which scenes start), so it is covered by the core tests alone.

Negative control. Reversing the target order inside the `AllParty` branch (a
one-line `targets.reverse()`) fails seven of the nine core tests, led by
`spiral_bld_hits_every_living_party_slot_with_its_own_roll_and_order`
("Fanbite: slot order, left `FighterId(3)`, right `FighterId(1)`") and
`every_all_party_route_resolves_in_an_ordinary_round` ("the whole party, in slot
order, left `[3, 2, 1]`"). Restoring the order passes all nine again. The two
refusal tests pass in both states, which is what they are for: they draw nothing.
Logs: `build/lane-evidence/negative-control-reversed-order.txt` and
`negative-control-restored.txt` (not committed; `build/` is ignored).
