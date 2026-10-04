# Fusion: two Zol slugs become one MetaSlug

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
It gates on the pinned record 18, on the actor being a living ZolSlug, and on the
pack having a MetaSlug; then it emits `EnemySkillUsed`, clears the four enemy
slots (`Roster::clear_enemies`), seats the MetaSlug in slot 1 and emits
`BattleEvent::EnemiesFused { removed, fighter, enemy_id, name, hp, agility }`.
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
