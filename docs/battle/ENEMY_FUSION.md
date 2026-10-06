# Fusion and COMBINE: an object reloads the enemy side

Two arms replace the whole enemy side with a formation their object keeps
inline: Fusion (two Zol slugs become a MetaSlug) and COMBINE (BladeRight and
HakenLeft become TwinArms, lane A5, section "COMBINE" below). Both records are
the pack's (`battle/formations.json`'s `inline_formations`, decoded by
`psiv_tools.formations`), looked up by label in `enemy_fusion.rs`; the engine
holds no formation bytes.

## Fusion

Enemy 34 ZolSlug has eight empty regular ability slots and four copies of
conditional ability `$12` FUSION behind `EnemyAI_ZolSlugs`. The Passageway to
Zio's fort (`$081`, group 29; formations `$D2`-`$D4`) is the only map that meets
them, and the campaign runner passed it only because `run_unless_boss` escapes
before the roll (`docs/campaign/RUNNER_LOG.md`, H15). This file is the rule the
port now runs, from `rust/psiv-core/src/battle/enemy_fusion.rs`; the capture is
`BATTLE_ORACLE_ARC.md`. The wider ledger is
[`ENEMY_EFFECT_ABILITIES.md`](ENEMY_EFFECT_ABILITIES.md).

## The chain

1. **The condition.** `EnemyAI_ZolSlugs` (`enemy_ai.rs`, arm `$06`) holds when every
   occupied enemy slot holds a ZolSlug and there are exactly two of them. The
   instruction block runs after the ability roll and writes the conditional id
   over it (`ps4.asm:19157-19168`), so a slug with two slugs on the field never
   attacks: the roll is one draw and the turn is Fusion.
2. **The routine.** `EnemyAttack_Blob` (`ps4.asm:23043`) sends a nonzero ability to
   `loc_10796`, which loads `BattleObj_Fusion` (`ps4.asm:35675`) and
   `BattleObj_Fusion2` (`ps4.asm:35850`) and clears `Current_Target_Index`.
3. **The objects** slide the two sprites together, and at frame `$14`
   (`ps4.asm:35817`) clear the 32 battle objects at `$FFFFD800` and the four
   `Fighter_Enemy_n` words (`35820-35831`), copy `loc_1A2F4` into
   `Enemy_Formation_Data` (`35832-35835`) and call `loc_14D46` (`35839`), which
   rebuilds the enemy side exactly as the battle's own load does
   (`loc_7F22` -> `Battle_FillEnemyStats`, `ps4.asm:11939`) and hands the turn
   back (`Battle_Routine = $16`, line 35841).
4. **The data** (`ps4.asm:35846-35847`) is `00 00 00 00 01 01 00 24 14 FF`:
   ambush 0, run 0, drop rate 0, drop item none, **one** enemy, enemy `$24`
   (36, MetaSlug) at position `$14`. Whatever HP the slugs had and whichever slots
   they stood in, the fight goes on against one full-HP MetaSlug in slot 1 (239
   HP, electric basic attack with the paralyze status, no abilities). The header
   bytes equal the slugs' own formations' (`0xD2`-`0xD4` carry run 0, drop rate 0,
   no item), so nothing about running or drops changes.

No `UpdateRNGSeed2` site lies in either object, and the record (`1F 00 22 24 00 00
00 00`, effect `AbilityEffect_None`) is never dispatched through
`GetEnemySkillEffectAndRange`; its range and hit bytes are the object's own
constants (`$22` is the ZolSlug id the object compares, `$24` the MetaSlug id it
loads).

## The seated fighter skips the round

`loc_14D46` sets bit 7 of the status byte of all four enemy slots
(`bset #7, ($FFFF4216)` and its three siblings, `ps4.asm:29744-29747`).
`loc_5772` (`ps4.asm:8021`) tests the status with `$EE` - including bit 7 - at
line 8040 before a queued fighter acts, and skips an empty slot (`tst.w (a3)`), so the MetaSlug seated in the middle of a round does not take the
queue entry its slot still holds. The RAM log shows the bit land on slots 6 and 7
in the Fusion action (`status` 0 -> `$80`) and the round carries that one action
and no other. The port keeps the bit out of `Stats` (as it does for revived
fighters) and answers it in `take_turn`, which skips a fighter that an
`EnemyReplenished` or `EnemiesFused` event of the same round names.

## What the port does

`enemy_fusion::resolve_fusion` is reached after Fission in `roll_enemy_ability`.
It gates on the `(carrier, ability)` arm (`RELOADS`), the record's effect byte
(`$1F`, `AbilityEffect_None`) and the actor being alive, looks the arm's record
up by label (`BattleData::inline_formation`, an error when the pack lacks it),
then emits `EnemySkillUsed` (Fusion only: COMBINE's arm clears the id first),
clears the four enemy slots (`Roster::clear_enemies`), seats the record's enemy
in its slot and emits
`BattleEvent::EnemiesFused { removed, fighter, enemy_id, name, hp, agility }`.
The engine then takes the record's run byte as `Enemy_Run_Chance`: the copy is
the whole record, and `Enemy_Run_Chance` is `Enemy_Formation_Data + 1`
(`ps4.constants.asm:2039-2041`).
The battle view follows it (`session/battle/mod.rs`): the removed slots stop
being drawn, slot 1 takes the MetaSlug's name.

The oracle's effect columns show the new fighter's agility (`e1_agi_bat` 14 ->
12), which is why the event carries it; the replay comparator checks it
(`replay/compare.rs`, `effect_divergence`).

## Not modelled

Nothing in this arm depends on state the port lacks. Rewards need no change:
each enemy adds its own record's experience and meseta when it dies, and the
slugs that fused never die.

The shell draws the MetaSlug from the event's `position` (`$14`) and enemy id
through `EnemyStatus`; the certified pair is `battle-fusion`
([`BATTLE_COMMAND_UI.md`](BATTLE_COMMAND_UI.md)). The slide-together animation is
not modelled.

## COMBINE

`EnemyAI_HakenLeftExists` (`$0D`, named by 84 BladeRight) fires when exactly one
other enemy object is a HakenLeft and none is a second BladeRight;
`EnemyAI_BladeRightExists` (`$0E`, 86 HakenLeft) is the same with the ids swapped
(`ps4.asm:21557-21653`). They write `$3A` and `$3B`. `EnemyAttack_Ripper`'s
fall-through `loc_F3B6` (`ps4.asm:21620-21624`) and `EnemyAttack_Piercer`'s
`loc_F2E4` (`ps4.asm:21549-21553`) clear `$24(a4)` and `Current_Target_Index` and
turn the attack object into `$354` (`loc_23C84`, `ps4.asm:47469`), which on its
first frame clears the 32 objects at `$FFFFD800` and the four enemy fighter
words, copies `loc_23D00` (`00 00 00 00 01 01 00 57 14 FF`: one enemy, 87
TwinArms, position `$14`) over `Enemy_Formation_Data` and calls `loc_14D46`
(`ps4.asm:47483-47503`). No `UpdateRNGSeed2` call sits in the object or the
routines it calls.

Because the arm clears the id, no frame of the log shows `$3A`/`$3B`: the
extractor files the turn as an attack that resolved no slot, the comparator takes
`EnemiesFused` as the port's answer for it, and the capture
(`replay_fixtures/air_castle/combine.json`, formation `$1BA`) is checked by the
enemy the reload seats. The run byte changes there: `$1BA`'s `$3C` becomes the
record's 0. In the Air Castle no formation pairs the two, so COMBINE does not
fire on that stretch; the port's arms fired for a BladeRight with no partner until
lane A5 ([`ENEMY_ABILITIES_AIR_CASTLE.md`](ENEMY_ABILITIES_AIR_CASTLE.md)).
