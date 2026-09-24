# Party actions, items and battle flow

Scope: party-side battle behaviour — skills and items, rewards,
entering and leaving a battle, field ability recovery, and the
attack passes' draw counts.

Index: [source and provenance notes](../../SOURCE_NOTES.md).

## 2026-09-12 — VISION and opening character skills

**RETAIL CARTRIDGE BUG 13 — VISION reads the caster's name.** The verified
US skill record 47 at `$2A9E98` uses stat selector zero and effect `$26`.
`Effect_SetupSkillParams` resolves zero through `AbilityStatsOffs` to stats
offset zero, then reads one byte there. `AbilityEffect_DexterityUp` adds
that byte to each eligible target's modified dexterity. Hahn's initial
encoded `H` is `$08`, so normal Vision adds 8, independently of his level
or mental stat. It replaces the previous dexterity buff.

Fresh oracle RAM probes on 2026-09-12 verified this on the US retail ROM.
Starting from tape 07's battle idle at frame 25000, a patched command round
has Alys and Chaz defend while Hahn uses Vision. At frame 26000, Chaz/Alys/
Hahn dexterity is `13/21/13`, up from `5/13/5`; Hahn's uses fall from 5 to 4.
Repeating with only Hahn's first name byte patched to `$1A` changes the
results to `31/39/31`, a +26 buff. These are controlled RAM-patched oracle
fixtures, not natural menu-input captures. The local receipts are
`build/native-skills/{vision,renamed-vision}.ram` and the corresponding
`oracle-*-vision.csv` logs.

The native fix gives Vision a constant +8 when its stat selector is zero.
A mod that supplies another supported stat selector uses that stat instead.
This follows the owner's existing obvious-bug policy in
`docs/RUNTIME_DESIGN.md`; the original JSON remains unchanged.

**RETAIL FINDINGS — VORTEX and EARTH.** A second patched command round uses
Vortex on enemy 6, Earth on enemy 7, and Vision on the party. Enemy 6's HP is
raised to 512 to expose separate hits. It drops once to 452 at frame 25181.
Enemy 7 gains status `$08` at frame 25284 while retaining agility 6; both
before and after, its HP is 25. Final remaining uses are Earth 2, Vortex 4,
Vision 4. See `build/native-skills/oracle-skills.csv`, `oracle-changes.json`,
and `skills.ram`. Vortex's three projectile sprites culminate in one damage
application; Earth's successful object timer sets sleep without the generic
sleep object's agility reduction. `Battle_RestoreStatsAtTurnEnd` rolls once
per occupied sleeping fighter, wakes on odd, restores modified agility, then
clears enemy paralysis without a roll.

**PORT BUG — alphabetical resistance census.** `psiv_tools/battle_pack.py`
emits `enemies.properties` as sorted names. The Rust bridge incorrectly used
that census as element-id order for both enemies and newly seated characters.
For example, Earth (element 11, psychic) read the mechanical immunity at
alphabetical slot 11, so ZoranBult could never be put to sleep. The bridge
now places named values in `ELEMENT_NAMES` cartridge order, validates the
complete name set, and checks all 153 enemy and eleven initialized character
records. This is a port repair, not a cartridge bug or a pack format change.

## 2026-09-12 — Battle items and Fission

**RETAIL FINDING — revival details.** Moon-Dew restores a dead human to
one-quarter maximum HP and preserves technique seal. A controlled US-ROM
command probe changes Hahn from HP 0/status `$1D` to HP 5/status `$90` at
maximum HP 21: seal plus a transient sprite flag. Sol-Dew on a living human
restores HP without curing ailments. Repair-Kit is disposable type 8,
although several equipment activations reuse other item-effect dispatchers.
All 160 item activation headers and disposal types were verified against
the ROM; local evidence is in `build/native-items/`.

**PORT BUG — dormant Igglanova neighbors.** `EnemyInit_Igglanova`
(`ps4.asm:18275`) destroys the adjacent fighter objects without clearing
their cached stats. The old port counted and rendered those full-HP records
as already present. A fresh, input-only retail Academy replay confirms the
boss is initially alone (frame 40650), then grows right/left Xanafalgues
(frames 41200/41850). See `build/native-fission/oracle-receipt.json`.

**RETAIL FINDING — Fission uses cached neighbor identity.**
`EnemyAI_EmptySpace` (`ps4.asm:23524`) uses one random parity draw only when
both sides are empty. `BattleObj_IgglanovaFission` / `loc_14CBE` restores
the chosen slot through `Battle_FillEnemyStats`, retaining its cached enemy
ID. In Guilgenova's formation this restores Gicefalgue, despite the nominal
Fission2 target byte. A replacement begins with status bit 7; the turn
executor's `$EE` mask (`loc_5772`) prevents a newly revived fighter from
acting on an old queued turn. The native engine uses an active-slot state
and same-round revival events, without leaking the transient bit into saves.

## 2026-09-12 — Battle rewards across a connected Academy route

**PORT BUG — displayed meseta never reached the purse.**
`Battle_VictoryMessage` (`ps4.asm:4796..4806`, retail `$30E6` region)
adds the zero-extended 16-bit `$FFFF41D0` pool to `Current_Money` and
caps the result at 9,999,999. The native battle emitted that pool, and Godot
displayed it, but the runtime only absorbed stats and awarded experience.
The consumed battle now pays its pool on confirmed victory, including
vehicle battles; escaping with accumulated kills pays none. Runtime tests
cover the cap, save/reload, and duplicate result-close calls. The connected
START-to-Motavia development route now retains all 86 meseta from its four
battles, ending with 986 after Hahn's 100 and the principal's 300.

**PORT BUG — camp healing used a fixed midpoint and battle-like masks.**
The verified ROM contains `HealingEffectData` at `$67FA4` with exact bytes
`120f0000130e8000140d8000150088001600000017000000180f0000`.
`CalcHealingValue` (`$67FC0`) draws from UpdateRNGSeed, rejects repeats of
the previous masked value (initial zero), and accepts sixteen values.
`ItemUsed_AllAlliesLoop` calls it before inspecting a slot, including the
first empty slot; that empty slot then ends the loop.
Field masks therefore differ from combat: Moon Dew and Sol Dew clear all
status on eligible humans; ordinary healing masks with `0F`. Repair Kit's
explicit ID branch allows only Demi/Wren, while other restorative items
reject androids. Native camp now follows those rules. `Win_ItemUseCharListMain`
also calls `ReorderInventory` after clearing a used slot; the port had left
the hole as if this were a battle command.

## Native battle return and field ability recovery (2026-09-12)

`GameMode_LoadFieldMap` (`ps4.asm:107505`) retains field-object RAM when
`Map_Load_Flags` bit 0 is set, skips `LoadMapObjects` initialization
(`110924`), and still invokes `MapDataManager` (`107597`). Native battle
return now preserves the live cast while applying that map-data walk. The
Academy flag-$0B boss and invisible-block despawns were verified in Godot.

Field TECH/SKILL follow `loc_5FF98`, `Win_LoadTechList`, `SubtractTP`,
`Win_TechUsedMsg`, `loc_61AD4`, `loc_620D2`, and `GetItemTechSkillEffect`.
The field death test is bit 2, distinct from battle's combined death mask.
Both group recovery loops calculate before checking the first `$FF` party
slot, then return immediately. Skills read their selected modified stat;
RECOVER uses strength, MEDICE/MIRACLE mental, MEDIC PW strength. The latter
heals and revives humans, including living targets, while skipping androids.
All 17 field-usable records were compared to local retail ROM bytes; the
two teleport techniques still await implementation.

## The second hit pass of Alys's and Kyra's attack (2026-09-24)

`loc_B6A2` (`ps4.asm:17492`, `$00B6A2`) fills `Fighters_Hit_Flags`
(`ps4.constants.asm:2019`), and a plain Attack runs it **twice** for two of the
eleven party members. The cartridge's damage stage reads that array, so the
second pass is the one that decides: this note records the attacker set, the
order, and what the port changed.

The attacker set is the character, not the weapon. `Character_Attack`
(`ps4.asm:12996`) is fighter routine 6 — the Attack command — and runs its pass
at `ps4.asm:13018`. It then dispatches `Character_AttackActionOffs`
(`ps4.asm:13056-13068`) by `fighter_id - 1`
(`ps4.asm:13044-13050`): entry 2 (`CharAttack_AlysKyra`) and entry 10 (the same
routine) are Alys and Kyra, `Character_Stats` indices 1 and 9; the other nine
entries are one per character and none of them calls `loc_B6A2`.
`CharAttack_AlysKyra` (`ps4.asm:13958`) writes `$FFFF` to
`Current_Target_Index` and dispatches `AlysKyraAttackRoutines[action_routine]`
(`ps4.asm:13968-13972`), whose step 0 is `AlysKyraAttack_Init` (`ps4.asm:13975`)
— first instruction `jsr loc_B6A2(pc)` at `ps4.asm:13976`. It runs in the same
frame as the first pass: `Character_Attack` falls straight through to its
action dispatch, which is why tape 07 shows all four of her swing's hit rolls at
f29489.

What the second pass does, instruction for instruction. `loc_B6A2` presets all
nine flags to `$FF` (`ps4.asm:17493-17498`), so the first pass is overwritten
wholesale, not merged; then, for each slot in its window, a slot that is empty
or carries `StatusDead_Mask`/`StatusAndroidDead_Mask` is written `$FF` with no
roll (`loc_B704`, `ps4.asm:17528-17530`) and every other slot gets one `loc_B716` roll
(`ps4.asm:17536-17554`): the actor's `dexterity_battle` against the target's
`agility_battle`, scale 2, miss ≤ 8, crit > `$74`. It is the same computation,
the same one roll per living target, and it runs for **every** Alys/Kyra attack,
single-target weapon or not. One difference from the first pass is worth naming:
`smi ($FFFFEE49).w` (`ps4.asm:17502`) records whether the window came from a
negative `Current_Target_Index`, and `CharAttack_AlysKyra` has just made it
negative, so a critical rolled in the second pass is demoted to a normal hit
(`loc_B754`, `ps4.asm:17555-17557`) — and the window is the actor's four enemy
slots (`ps4.asm:17504-17511`), not the chosen target.

The damage stage is what makes the second pass matter. `Fighter_TakeDamage`
(`ps4.asm:3564`) gates on the slot's own flag byte (`bmi` ⇒ no damage,
`ps4.asm:3569-3571`), and the flags it reads are the second pass's; Alys's and
Kyra's damage trigger is the slasher props their animation throws
(`AlysKyraAttack_ThrowSlasher`, `ps4.asm:14008-14046`, spawns
`BattleObj_SlasherTrace`/`SlasherAttack1` for each set bit of `$31(a4)`), whose
landing object reaches `BattleObj_CloseRangeDamageMain` (`ps4.asm:70832`) and
writes routine `$C` to all four enemy slots (`ps4.asm:70841-70845`).

Why the port draws the second pass over *its own* target list, and what that
leaves out. Every weapon Alys or Kyra can equip is item type 2 — the one-handed
multi-target class (`Battle_AttackCommand`'s type test, `ps4.asm:2244-2248`;
`runtime-pack/battle/equipment.json` lists seven type-2 weapon records and all
seven are Alys/Kyra-only, `usable_by_mask` `0x0202`) — and `loc_168C`
(`ps4.asm:2293-2301`) stores `$FFFF` as the command's target for those, which
`loc_57C4` (`ps4.asm:8045-8053`) copies into `Current_Target_Index` at turn
setup. Both passes therefore always cover every living enemy, which is exactly
what the port's `Reach::All` produces for the same weapons. A single-target
weapon in either hand is unreachable in retail (no equip mask grants one, and
the type-2 test in `AlysKyraAttack_Init` picks the animation frames,
`ps4.asm:13986-14002`), and in that unreachable state the cartridge would roll
the second pass over all four enemy slots while the props that trigger damage
are skipped — so the port's modelled
target list is the same set in every reachable case, and its one-target
behaviour is a documented model, not a claimed match.

Every other `loc_B6A2` caller runs it once, and none of them is reachable from
Alys's or Kyra's Attack:

- **`Character_Attack`** (`ps4.asm:13018`) — every party member's Attack, and
  the first pass above. **`AlysKyraAttack_Init`** (`ps4.asm:13976`) — the second
  pass, Alys and Kyra only.
- **`loc_9848`** (`ps4.asm:14964`) — the shared "hit pass, then routine 5 on the
  window's slots" step of the character ability chains: `Character_TechActionOffs`
  step 5 (`ps4.asm:14178`) for every technique a party member casts, and the
  skill, item and combo tables `loc_9BF0` (`ps4.asm:15264`),
  `loc_A796` (`ps4.asm:16168`), `loc_AC02` (`ps4.asm:16536`),
  `loc_AFE4` (`ps4.asm:16837`) and `loc_B1FA` (`ps4.asm:17032`). Those commands
  are fighter routines `$A`/`$B`/`$10`/`$11` (`ps4.asm:1043-1050`), so
  `Character_Attack` never runs for them and the one pass is the only pass.
- **Skill and item prop objects**: `loc_9EE0` (`ps4.asm:15475`, reached by
  `SkillObj_Crosscut`, `SkillObj_Rayblade` and `SkillObj_Airslash`),
  `SkillObj_DblSlash` (`ps4.asm:15484`), `loc_9FAC` (`ps4.asm:15527`),
  `loc_A002` (`ps4.asm:15554`), `loc_A012` (`ps4.asm:15569`,
  `SkillObj_Vortex`), `SkillObj_Astral` (`ps4.asm:15575`),
  `SkillObj_Disrupt` (`ps4.asm:15593`), `loc_A95C` (`ps4.asm:16328`),
  `loc_AD3C`/`loc_AD5C` (`ps4.asm:16622`, `16632`), `loc_AE4E`
  (`ps4.asm:16712`), `loc_AF00` (`ps4.asm:16763`), `loc_B16A`
  (`ps4.asm:16980`), `loc_B2DE` (`ps4.asm:17103`) — one pass each, driven by the
  caster's skill or item object, never by the Attack command.
- **`loc_38BE4`** (`ps4.asm:73727`) — step 4 of `BattleObj_Gra`'s table
  `loc_38B0E` (`ps4.asm:73656`), the Gra technique's own object.
- **`loc_D09E`** (`ps4.asm:19200-19201`) — the tail of `Enemy_Attack`
  (`ps4.asm:19138`), every enemy's plain attack: one pass, and no enemy is a
  party character.

Native. `action.rs` keeps `roll_hits` as one pass and adds
`takes_second_hit_pass(roster, actor)` plus `SECOND_HIT_PASS_CHARACTERS`
(`Character_Stats` indices 1 and 9), keyed on the roster's own
`Fighter::character` — the identity the caller already seats. `resolve_attack`
draws the first pass, then the second when the rule says so, and uses the last;
both come before any damage draw.

Tests and evidence. Core: `action_second_pass_tests.rs` (registered from
`action.rs` with `#[path]`) holds seven tests —
`the_second_pass_is_alys_and_kyra_only` (the rule for Alys, Chaz, Hahn and an
enemy, plus Kyra seated from `party_fixtures::records()[9]` on a real round:
36 draws), `a_two_target_alys_swing_draws_the_pass_twice` (36 draws: 4 hit + 32
damage, and both targets resolve from the *second* pass's rolls),
`a_one_target_alys_swing_draws_the_pass_twice` (18 draws, the port's own
`Reach::Single`), `a_missed_first_pass_lands_on_the_second`,
`a_landed_first_pass_misses_on_the_second` (2 draws, no damage),
`a_dead_slot_costs_neither_pass_a_roll` and
`an_unlisted_attacker_draws_one_pass` (Chaz and an enemy: 17 draws each).
`action.rs`'s own roster helper now seats each record under its own
`Character_Stats` index, which is what makes Alys's swings in those tests the
cartridge's 36-call shape. `engine_tests_replay.rs` counts both passes as
modelled: Alys's swing's 36 calls, and on the verbatim stream the ledger's first
divergence moves from f29489 to f29789 (the `$FFFFEEA8` ability re-roll).
(Correction, later on 2026-09-24: lane P2b closed that divergence as well, so
both tapes 07 and 09 now replay on the verbatim stream; see
[`BATTLE_ORACLE_REPLAY.md`](../BATTLE_ORACLE_REPLAY.md).)
`docs/BATTLE_ORACLE_REPLAY.md` carries the updated tables.

Negative control. Removing the second pass (keeping the first pass's verdicts
for every attacker) fails ten tests: both replay tests — the verbatim stream's
first divergence returns to f29489 (`Normal, Some(11)` where the log has 10) and
`tape07s_actions_match...` reports the same one against `None` — six of the
seven new core tests, each on its count
(`a_two_target_alys_swing_draws_the_pass_twice` 2 against 36,
`a_one_target_alys_swing_draws_the_pass_twice` 17 against 18,
`the_second_pass_is_alys_and_kyra_only` 34 against 36,
`a_missed_first_pass_lands_on_the_second` 1 against 18,
`a_landed_first_pass_misses_on_the_second` 17 against 2,
`a_dead_slot_costs_neither_pass_a_roll` 17 against 18), and the two `action.rs`
tests that ride the same shape (`a_multi_target_swing_rolls_both_hit_passes_then_damages_each`
34 against 36, `a_multi_target_swing_can_never_crit`). The seventh,
`an_unlisted_attacker_draws_one_pass`, passes in both states — that is what it is
for. Restoring the pass gives 456 passed, 0 failed, 1 ignored. Logs:
`build/lane-evidence/negative-control-second-pass-removed.txt` and
`negative-control-restored.txt` (not committed; `build/` is ignored).

## The vehicle's own attack: command 6, three hit passes (2026-09-24)

A vehicle's Attack is **not** `Character_Attack`. The Attack command is command
index 1, and `loc_5B8E` (`ps4.asm:8410-8416`) turns a command index into a
fighter routine through its own seven-entry table (`loc_5BD4`,
`ps4.asm:8430`): `$06, $0A, $0B, $10, $0F, $12, $13` for indices 1..7, i.e.
`Character_Attack`, `Character_DoTech`, `Character_DoSkill`,
`Character_DoItem`, `Character_Defend`, `loc_AF9C` and `loc_B1D4` —
`BattleCharacterRoutinePtrs` entries 6, $A, $B, $10, $F, $12 and $13
(`ps4.asm:1039-1052`). The vehicle menu writes **command 6** for its first
option: `Battle_VehMainOptions` (`ps4.asm:2036-2049`) sends option 0 to
`loc_684A` (`ps4.asm:9852-9878`), whose tail stores `#6` and then
`($FFFFF43D).w` — the vehicle index — in `Battle_Command_Data`. So a vehicle's
Attack is `loc_AF9C` (`ps4.asm:16810`), and the weapon check in
`Character_Attack` (`ps4.asm:13002-13019`) never runs for it.

`Character_AttackActionOffs` (`ps4.asm:13056-13068`) is not reached either, and
its index would be out of range if it were: `loc_78EE` (`ps4.asm:11408-11414`)
seats the vehicle's `fighter_id` as `Vehicle_Index + $B` — `$0C`, `$0D` or
`$0E` — and the dispatch at `ps4.asm:13044-13050` indexes the table by
`fighter_id - 1`, i.e. `$0B`..`$0D` against a table of eleven characters.
Command 7, the menu's second option (`Battle_VehOpenSkills`/`Battle_VehSkills`,
`ps4.asm:7386-7460` and `loc_68C0`, `ps4.asm:9889-9904`), is `loc_B1D4` — the
vehicle skill path the port already models in `battle/vehicle_skill.rs`.

**The vehicle record has no equipment, and the attack never reads it.**
`loc_77AE` (`ps4.asm:11295-11404`) is what builds `Vehicle_Stats` (`$FFFF4700`)
from `VehicleData` (`ps4.asm:321154-321174`), 32 bytes per record
(`lsl.w #5, d3`): the `curr_hp`/`max_hp` word (`$02E4`, `$03C0` and `$02A8` for
the Land Rover, Ice Digger and Hydrofoil), strength / mental / agility /
dexterity, one byte each written to the stat, its `_mod` and its `_battle`
copy; attack / defence / magic defence as `atk_pow+1`, `dfs_pow+1` and
`magic_dfs+1` — the **low byte** of each word, the record holding one byte per
stat (`$C8`/`$50`/`$32` for the Land Rover = 200/80/50) — the fourteen element
property bytes, written to both halves of each word, and the skill mask and
eight use counts. It writes **nothing** at `$4C`/`$4D`: `right_hand` and
`left_hand` sit immediately past the record's element block, and the vehicle
build leaves them at whatever the RAM held. `Battle_GetItemType` is never
called on the vehicle, and the element a vehicle's attack uses comes from the
*target* — see `loc_280A` below. The port's `vehicle::battle_member` leaving
`equipment` zero is therefore not what made its swing skip; routing it through
`Character_Attack` was.

**The swing draws three `loc_B6A2` passes, and the third decides.**
`loc_AF9C` (`ps4.asm:16810-16814`) stores the command's low byte, the vehicle
index, in the actor's `ability` (`$24`), and runs the ten-state machine
`loc_AFE4` (`ps4.asm:16831-16841`). The frames that call `loc_B6A2`:

* **state 4** — `loc_B14C` (`ps4.asm:16957`) dispatches on the same index
  through `loc_B15C` (`ps4.asm:16964-16967`): `1` → `loc_B166`
  (`ps4.asm:16973`), `2` → `loc_B17E`, `3` → `loc_B184`. Each loads the
  vehicle's attack object (`$644` `BattleObj_LandRoverAtk`, `$648`
  `BattleObj_IceDiggerAtk`, `$64C` `BattleObj_HydrofoilAtk`, table
  `ps4.asm:74660-74662`) with the vehicle as its parent, and then `jmp
  loc_B6A2`.
* **state 5** — `loc_9848` (`ps4.asm:14964-14965`): `jsr loc_B6A2`, then
  routine 5 on the target window.
* **state 5 again** — the object's own first state hands the vehicle back:
  `move.w #5, $32(a0)` (`ps4.asm:82975`, `83048`, `83109`) once its wind-up has
  run (12 frames for the Land Rover and the Hydrofoil, none for the Ice Digger),
  so `loc_9848` runs a third time.

`loc_B6A2` blanks all nine `Fighters_Hit_Flags` before every pass
(`ps4.asm:17493-17496`), so each pass overwrites the last one's verdicts and
the **third** pass is what the damage stage finds. The object's second state
then sets the vehicle's `action_routine` to 7 (`ps4.asm:82986` and its two
siblings), and state 7 `loc_B1A6` (`ps4.asm:16999-17008`) writes routine `$C`
— `Fighter_TakeDamage` — to the target, which is the frame `loc_266C` runs
`Battle_CalculateDamage`. The command's own check is what keeps the rolls:
`loc_B6D4` routes command 6 to the rolling path *before* the `tst.w $24(a4)`
that would have turned the nonzero ability into a free hit
(`ps4.asm:17513-17516`), and `loc_B716` (`ps4.asm:17536-17554`) rolls the
actor's `dexterity_battle` against the target's `agility_battle` with the
physical constants, exactly as it does for a character.

**The damage is the ability arm, at element 2, from the target's own
properties.** `Figher_DamageCheckActor` (`ps4.asm:3737-3749`) reaches
`Character_DamageEnemy` (`ps4.asm:3910`), whose `bne.w loc_27D4`
(`ps4.asm:3917-3918`) is taken because `ability` is nonzero. `loc_27D4`
(`ps4.asm:3988-3998`) reads `Current_Command`'s index, subtracts two and
indexes the table at `loc_27FC` (`ps4.asm:4004-4010`); command 6 selects its
fifth entry, `loc_280A` (`ps4.asm:4016-4018`), whose whole body is
`moveq #0, d3 / move.b $32(a1), d3 / bra.s loc_27A4`. `a1` is the **target's**
stats and `$32` is the second `element_props` word — `element_factor(2)`,
energy. `loc_27A4` (`ps4.asm:3963-3969`) then supplies `atk_pow_battle` of the
actor, `dfs_pow_battle` of the target and `atk >> 2` when the slot's flag is
`$01`, and jumps to `loc_266C` — the same damage and the same sixteen draws as
every other path.

One target, and no critical demotion. `loc_B6A2` takes its four-enemy window
only when `Current_Target_Index` is negative (`smi ($FFFFEE49).w`,
`ps4.asm:17502-17511`), and a vehicle's command always carries a target index:
`loc_1152` (`ps4.asm:1824-1844`) writes the first present enemy slot's index —
the first enemy's word 0 is 8 (`moveq #8, d1` at `ps4.asm:11448`,
`_move.w d1, 0(a1)` at `11456`), so its index is 6 — and
`Battle_PickTargetEnemy` writes the cursor's when the player chose one. So one
roll per pass, one target, `$FFFFEE49` zero, and a critical stays a critical.
The `move.b d1, d4` before `lsr.w #2` truncates the bonus to the attack's low
byte; every vehicle's attack is at most `$FF`, so the truncation is invisible
here (`critical_bonus` is the shared expression).

**What the capture pins.** The forced `$53` Desrt Leach capture
([`BATTLE_ORACLE_FORCED.md`](../BATTLE_ORACLE_FORCED.md)) shows the vehicle
swinging on its turn in all six rounds, three hit-pass frames each
(f25059/f25060/f25072, f25209/f25210/f25222, …), and the rounds are what
separate the passes:

| round | first pass | second | third | log `hit` at the hit frame | log damage |
|---|---|---|---|---|---|
| 1 | normal (r 24) | normal (r 42) | normal (r 0) | `$00` | 165 |
| 2 | critical (r 63) | normal (r 2) | critical (r 60) | `$01` | 234 |
| 3 | **critical (r 61)** | normal (r 1) | **normal (r 2)** | `$01` | **154** |
| 4 | normal (r 21) | normal (r 5) | normal (r 41) | `$00` | 184 |
| 5 | normal (r 14) | normal (r 20) | normal (r 21) | `$00` | 165 |
| 6 | **critical (r 58)** | normal (r 22) | **normal (r 6)** | `$01` | **189** |

Round 3 is the discriminating case in both directions. Its damage, 154, is the
normal hit's arithmetic — `((45+8)*200)>>6 + 200 = 365`, `365*2>>2 - 28 = 154`
— and a critical's would be 204 (`365 + 2*50 = 465`, `465*2>>2 - 28 = 204`), so
the flags the damage stage read were the *third* pass's. Its `hit` byte is
`$01`, the *first* pass's, because the fixture samples that byte at the
action's hit frame (`oracle/fixture/observations.py`'s `action_record`) and the
cartridge only overwrites it 12 and 24 frames later. Round 2 is the other
direction: pass 2 read normal and pass 3 critical, and the log's `$01` and 234
are the third pass's. The same arithmetic pins the vehicle's attack and element
against the Land Rover's own record row: with `dfs_pow_battle` 28 of the Desrt
Leach, round 1's 165 and round 2's 234 are satisfied by exactly three
`(attack, element)` pairs — (80, 5), (100, 4) and (200, 2) — and the record's
`$C8` attack byte with `loc_280A`'s element 2 is the only one the cartridge can
produce, which also shows the record's byte landed in `atk_pow_battle` with a
zero high byte.

Native. `rust/psiv-core/src/battle/vehicle_attack.rs` is the rule:
`is_vehicle_fighter` reads the roster's own `Fighter::character` for the
cartridge's `Vehicle_Index + $B` ids (`0x0C`, `0x0D`, `0x0E`),
`VEHICLE_HIT_PASSES` is 3, `VEHICLE_ATTACK_ELEMENT` is 2, and
`resolve_vehicle_attack` draws the three passes with `roll_hits` (single
target, `multi_target` false), keeps the last, and lands
`calculate_damage(attack, target's defence, target's element 2, atk >> 2 when
critical)` through `clamp_damage` — the same 1..=999 clamp. `resolve_attack`
dispatches to it for a vehicle fighter before its weapon check, which is the
only change to the party path. The profiles it reads are `vehicle.rs`'s
`VehicleProfile` rows, transcribed from `VehicleData` and unchanged.

Tests and evidence. Core: `battle/vehicle_attack_tests.rs` (registered from
`vehicle_attack.rs` with `#[path]`) holds thirteen tests. On the capture's own
draws: `the_land_rover_replays_the_captures_first_swing` (165, HP 875),
`the_land_rover_replays_the_captures_second_swing` (234, HP 806),
`the_logs_third_round_needs_the_last_passs_verdict` (154 and *not* 204) and
`only_the_third_passs_verdicts_survive` (round 2's first two rolls with the
third swapped: 184). On the shape: `the_swing_costs_three_passes_then_one_damage_run`
(three passes then sixteen draws, one `Attacked` for the cursor's target),
`every_vehicle_swings_with_its_own_attack_byte` (the Land Rover's 200 lands 84,
the Ice Digger's 250 lands 112, the Hydrofoil's 150 lands 56 on sixteen zero
draws), `the_cannon_is_energy_elemental_and_reads_the_target` (the Desrt
Leach's energy 2 lands 165, an immune 0 lands the 1-damage floor, a very weak 4
lands 359), `a_swing_reaches_the_cursor_or_the_first_living_enemy`,
`a_kill_is_reported_for_the_engine_to_award`,
`a_vehicle_round_swings_instead_of_skipping` (the engine's own round),
`the_vehicle_identity_is_loc_78ee_s_fighter_ids` and
`the_element_is_the_targets_second_property_word`. Negative control:
`an_unarmed_character_still_skips_its_turn` — Hahn with both hands empty is
still `TurnSkipped { reason: Unarmed }` with no roll.

Replay. `replay_fixtures/divergences.json` no longer carries an entry for
`forced_53_desrtleach`: the data-driven test replays the capture's six rounds
and all 285 rolls exactly, and the manifest is now **empty** — the worklist
this machinery existed to produce is closed. `replay/compare.rs` gained
`flag_lags_the_swing`, which reads a per-target `hit` byte as a *reach* claim
(and the damage as the verdict) for a swing whose `loc_B6A2` passes arrive in
more than one frame — the vehicle's three — since that byte was sampled before
the swing's last pass. The verdict for those rounds is pinned by the core test
on round 3's own rolls above, and by the damage comparison the comparator still
makes.

The negative control for the vehicle rule is the old manifest entry's own
finding. With `resolve_attack`'s dispatch to `vehicle_attack` removed —
a vehicle back on `Character_Attack`'s weapon check — the data-driven test fails
exactly where the deleted entry said it would:
`forced_53_desrtleach: the port diverges at f25026 (round 1, no-swing): the log
has actor FighterId(1) resolving 1 target(s), the port no swing`. Restoring the
dispatch gives 1 passed, 0 failed. Transcripts:
`build/lane-evidence/03-negative-control.log` and
`04-replay-after-restore.log` (not committed; `build/` is ignored).
