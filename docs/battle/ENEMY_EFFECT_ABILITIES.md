# Enemy status and stat abilities (the Motavia arc, and the Zelan route)

What this ledger records: the enemy abilities that change a status bit, a
battle stat or a fighter's life - VOICE, RIMIT, EVIL EYE, STASISBALL, DORAN,
SEALS, GELUN, DEBAN, VOL, GIRES and the Zol slugs' Fusion - how the cartridge
runs each one, what the port does, and where the forced-formation captures that
prove it are. It is the status/stat half of
[issue #58](https://github.com/TusanHomichi/PSIV/issues/58); the damage half is
[`ENEMY_DAMAGE_ROUTES.md`](ENEMY_DAMAGE_ROUTES.md). The inventory these rows come
from is [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md); the capture table is
[`BATTLE_ORACLE_ARC.md`](../oracle/BATTLE_ORACLE_ARC.md).

Code: `rust/psiv-core/src/battle/enemy_effect.rs` (the status and stat handlers),
`enemy_skill.rs` (`resolve_res`, which now heals for GIRES as well as RES) and
`enemy_fusion.rs` (Fusion). Nothing here is decided from an ability's name; every
rule is a line of `reference/ps4disasm/ps4.asm`, and every pair was seen run in a
capture (`BATTLE_ORACLE_ARC.md`).

The Zelan-to-Kuran route (issue #58 class, lane A4, 2026-10-04) added POISONMIST,
SLEEP GAS, SHADOWBIND and the refill WARNING to the same table and moved the
crawlers' THREAD and POISON into it, so that every status or stat ability the
port runs goes through one handler per effect byte: [section 7](#7-the-zelan-route-pairs).

## 1. How an effect ability runs

An enemy turn is `Enemy_Attack` (`ps4.asm:19138`), which rolls the ability and
hands the actor to `EnemyAttackOffs[enemy_id]` (`ps4.asm:19206`). For the abilities
below the routine's arm loads an attack object whose handoff state calls
`GetEnemySkillEffectAndRange` (`ps4.asm:8687`) **once**. That call, not the
object's animation, is where everything that matters happens:

1. It copies the record's byte 2 (low nibble) into `Battle_Ability_Range`, clears
   the nine `Battle_Ability_Effects` words (`Battle_ClearEffects`) and runs
   `Ability_GetEffectAndRange` (`ps4.asm:8886`).
2. `AbilityRangeOffs` (`ps4.asm:8903`) chooses the fighters to visit:
   range `8` is `AbilityRange_Single` (`Current_Target_Index`), `9`
   `AbilityRange_MultiChars` (party slots 1-5), `2` `AbilityRange_MultiEnemies`
   (enemy slots 6-9).
3. `Ability_ProcessRange` (`ps4.asm:8975`) visits them in slot order and skips an
   empty slot (`tst.w (a0)`) and one whose status holds `StatusDead` or
   `StatusAndroidDead` (`andi.b #$44, d0`) **before** the handler, so a dead
   party member costs no draw.
4. For each visited fighter the record's effect byte indexes `AbilityEffectsOffs`
   (`ps4.asm:9036`) and the handler runs. `Effect_SetupSkillParams`
   (`ps4.asm:9576`) reads the actor's stat through record byte 1 **masked with
   `$7F`** (the `$82` of a magic record is selector 2, `mental_battle`), and - when
   byte 4 names a resistance stat - calls `Battle_CalculateChances`
   (`ps4.asm:17338`) with the target's stat, the target's element factor (byte 5),
   the record's byte 3 as the miss threshold and the effect id as the critical
   threshold. Byte 4 zero skips the roll entirely (`tst.b $4(a0,d0.w) / beq.s
   loc_6564`, `ps4.asm:9594-9595`) and the handler lands without a draw.

The handlers the table below needs:

| effect | handler | rule |
|---|---|---|
| `$02` | `AbilityEffect_Death` (`ps4.asm:9098`) | returns before the roll when dead; else one roll that only writes the effect word; the object then kills (below) |
| `$03` | `AbilityEffect_AttackDown` (`ps4.asm:9109`) | roll; on success `atk_pow_battle = atk_pow - d1`, cleared when not positive (`bhi`) |
| `$06` | `AbilityEffect_AgilityDown` (`ps4.asm:9139`) | roll; on success `agility_battle = agility_mod - d1` (byte), 1 when not above (`bhi`) |
| `$07` | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | returns before the roll when bit 3 (asleep) or bit 1 (paralyzed) is set; else one roll that only writes the effect word |
| `$08` | `AbilityEffect_SealTech` (`ps4.asm:9167`) | returns before the roll when sealed; else roll, then `bset #StatusTechSealed` |
| `$0A` | `AbilityEffect_DefenseUp` (`ps4.asm:9203`) | `dfs_pow_battle = dfs_pow + d1` (word), from the derived value |
| `$1C` | `AbilityEffect_Paralyze` (`ps4.asm:9424`) | returns before the roll when paralyzed; else roll, then paralyzed set, sleep cleared (`bclr #StatusAsleep`), `agility_battle` and `dexterity_battle` both 1 |

`d1` is the actor's power stat. One implementation per handler serves every
carrier: `enemy_effect::apply` is keyed on the record's effect byte and the route
table (`ROUTES`) gates which `(enemy, ability)` pairs may reach it.

## 2. The pairs

| ability | carriers | routine and arm | object | handler | range |
|---|---|---|---|---|---|
| `$2A` RIMIT | 115 Greneris; 77 TechPlant | `EnemyAttack_Juza`, arm `loc_E4C4` (`ps4.asm:20648`); `EnemyAttack_FlattrPlnt`, arm `loc_F65E` (`ps4.asm:21822`) | object `$754`, `loc_2ACD6` (`ps4.asm:56212`), child `$758`, `loc_2AC22` (`ps4.asm:56160`); `BattleObj_EnemyRimit` (`ps4.asm:38366`) | `$07` + `loc_25074`; `$07` + `loc_24CFE` | 9 |
| `$4C` EVIL EYE | 106 Haunt, 107 Spector | `EnemyAttack_Haunt` (`ps4.asm:21005`), arm `loc_EAC6` (`ps4.asm:21023`) | object `$700`, `loc_2CB1C` (`ps4.asm:58346`) | `$07` + `bset #3` on the stored target | 8 |
| `$2F` VOL | 115 Greneris, 72 BloodSaber, 88 SoldrFiend | arms `loc_E47A` (`ps4.asm:20630`), `loc_F93C` (`ps4.asm:22005`), `loc_F1E2` (`ps4.asm:21484`) | `loc_2AE8E` (`ps4.asm:56333`) with child `loc_2AE0C` (`56296`); `loc_1D2F4` (`ps4.asm:39774`); `loc_23216` (`ps4.asm:46736`) | `$02` + `loc_25048` | 8 |
| `$34` VOICE | 76 FlyScreamr | `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`), arm 21803 | `BattleObj_Voice` (`ps4.asm:38465`) -> `loc_24CC8` (`ps4.asm:48646`) | `$07` + `loc_25074` | 9 |
| `$0B` STASISBALL | 19 Blauzen, 21 Goldine, 26 LifeDeletr | `EnemyAttack_Blauzen` `.stasisball` (`ps4.asm:23404`); `EnemyAttack_LifeDeletr` `.ability` (`ps4.asm:23140`) | `BattleObj_BlauzenStasisBall` (`ps4.asm:28127`) and four children; `BattleObj_LifeDeletrStasisBall` (`ps4.asm:26956`) and two | `$1C` | 8 |
| `$28` DORAN | 115 Greneris | `EnemyAttack_Juza` (`ps4.asm:20575`), arm 20708 | object `$768`, `loc_2A814` (`ps4.asm:55873`) | `$06` | 9 |
| `$57` GELUN | 115 Greneris | same routine, arm 20700, same tail (`loc_E5E6`) | `loc_2A814` | `$03` | 9 |
| `$29` SEALS | 115 Greneris, 116 Radhin | arm 20728 | object `$76C`, `loc_2A74A` (`ps4.asm:55820`) | `$08` | 9 |
| `$2D` DEBAN | 116 Radhin; 70 ShadowSabr | `EnemyAttack_Juza` arm `loc_E666` (`ps4.asm:20747`); `EnemyAttack_ShadowSabr` (`ps4.asm:21933`) arm `loc_F89A` (`ps4.asm:21966`) | `loc_2A660` (`ps4.asm:55760`); `loc_1D7D8` (`ps4.asm:40098`) | `$0A` | 2 |
| `$3E` GIRES | 100 TechMaster | `EnemyAttack_TechUser` (`ps4.asm:21156`), fall-through `loc_EEAA` (`ps4.asm:21266`) | object `$3D4`, `loc_213DC` (`ps4.asm:44701`) | `NormalLogic` heal | 1 |
| `$12` Fusion | 34 ZolSlug | `EnemyAttack_Blob` (`ps4.asm:23043`), `loc_10796` | `BattleObj_Fusion` (`ps4.asm:35675`), `BattleObj_Fusion2` (`ps4.asm:35850`) | none | - |
| `$10` THREAD | 31 CarrionCr | `EnemyAttack_Crawler` (`ps4.asm:23081`), arm `loc_1084A` (23113) | `BattleObj_Thread` (`ps4.asm:36186`) | `$06` | 8 |
| `$11` POISON | 32 Caterpillr | the same routine, arm `loc_10836` (23107) | `BattleObj_Poison` (`ps4.asm:36279`) | `$1B` | 8 |
| `$24` POISONMIST | 57 Mistralgec | `EnemyAttack_SandNewt` (`ps4.asm:22267`), arm `loc_FD06` (22287) | `BattleObj_PoisonMist` (`ps4.asm:42202`) | `$1B` | 8 |
| `$25` SLEEP GAS | 63 GerotLux | `EnemyAttack_AbeFrog` (`ps4.asm:22246`), arm `loc_FCA0` (22257) | `BattleObj_SleepGas` (`ps4.asm:41714`) -> `loc_24D76` (`ps4.asm:48702`) | `$07` + `bset #3` | 9 |
| `$4B` SHADOWBIND | 111 ChaosSorcr, 138 ChaosSorcr2 | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`), arm `loc_E83E` (20868) | object `$724`, `loc_2BCEA` (`ps4.asm:57360`) | `$06` | 9 |

A carrier appears in `ROUTES` only if a capture saw it use the ability
(`BATTLE_ORACLE_ARC.md` section 2); the ones that are not routed, and why, are
that ledger's section 3, and they stay on the explicit `UnsupportedAbility` path.
Radhin's SEALS and DEBAN (the `116` entries above) are the deferred pairs.

## 3. What each ability does

### VOICE (`$34`, FlyScreamr)

`EnemyAttack_FlattrPlnt`'s `$34` arm clears `Current_Target_Index` to `$FFFF` and
loads `BattleObj_Voice`; its state `loc_1C206` ends in `jmp (loc_24CC8)`, the
shared tail that calls `GetEnemySkillEffectAndRange` and then jumps to
`loc_25074` (`ps4.asm:48929`). Record 52 is `07 01 09 80 02 0B`: range 9, actor
STR against each target's MEN, psychic element factor, miss threshold 128.

`AbilityEffect_SleepParalyze` never sets a status bit. The bit is the object's:
`loc_25074` walks the five party fighters and does `bset #3, $16(a1)` for every
one whose effect word is nonzero - i.e. every target whose roll landed. It does
**not** touch `agility_battle` (the `BattleObj_SleepEffect` object of the party's
own sleep technique does, `ps4.asm:74837`; VOICE does not). Consequences, all
seen in the captures: a member that is already paralyzed (the crawlers and
FlyScreamr's own basic attack paralyze) takes no roll at all, so a VOICE over three
members can draw one roll; sleep ends the usual way, by the end-of-round wake roll
(`Battle_RestoreStatsAtTurnEnd`, `ps4.asm:9792`), which also restores agility.

### RIMIT (`$2A`) and EVIL EYE (`$4C`)

Both are `AbilityEffect_SleepParalyze` (effect `$07`) with a sleep bit the object
sets, exactly VOICE's shape. Neither writes a damage request: the census classed
them `damage` from the handler-and-object rule of its section 1, and the damage
lane's reading of the chains found no `move.w #$C` in any of them - they write the
flinch (`move.w #5`) and nothing else - so `ENEMY_ABILITIES.md` now carries them as
status/stat effects.

- RIMIT, record 42 `07 82 09 20 02 0B`: range 9, MEN against MEN, psychic,
  miss threshold 32. Greneris's arm clears `Current_Target_Index` and loads
  object `$754` (`loc_2ACD6`). Its state 2 spawns the child `$758`
  (`loc_2AC22`, `ps4.asm:56160`), which calls `GetEnemySkillEffectAndRange` once
  (line 56167); the parent's state 8 (`loc_2ADE2`, line 56282) then calls
  `loc_25074` - the VOICE sleep tail - on the effect words. TechPlant's RIMIT is
  `BattleObj_EnemyRimit` (`ps4.asm:38366`): one call (line 38414) and the
  `loc_24CFE` tail's `bset d0, $16(a0)` with `$FFFFEEA2` = 3 - the same bit.
- EVIL EYE, record 76 `07 02 08 40 02 0B`: range 8, mental against mental,
  psychic, threshold 64 (byte 1 is `$02`, not `$82`; the `$7F` mask changes
  nothing). `EnemyAttack_Haunt`'s `$4C` arm keeps the drawn target and loads
  `$700` (`loc_2CB1C`). The object's state `loc_2CB5E` calls
  `GetEnemySkillEffectAndRange` once (line 58375) and reads the *stored target's*
  effect word to decide between its two states; the landing state (`loc_2CCE0`,
  line 58495) does `bset #3, $16(a1)` on that target unless the miss flag
  (bit 0 of `$4(a4)`) is set.

Draws, as the captures show them: RIMIT one chance roll per living member that
is neither asleep nor paralyzed (four rolls in all over three members, three over
the two Greneris had left); EVIL EYE one chance roll. A sleeping or paralyzed
target takes none (the handler returns first).

### VOL (`$2F`)

Record 47 `02 82 08 50 02 0A`: range 8, MEN against MEN, biological, threshold 80,
`AbilityEffect_Death` (`ps4.asm:9098`): `btst #StatusDead / bne` returns before
the roll, else one roll that writes the effect word. Three objects
(`loc_2AE0C`'s child of `loc_2AE8E`, `loc_1D2F4`, `loc_23216`) call
`GetEnemySkillEffectAndRange` once, read the stored target's effect word, and on
a landing call `loc_25048` (`ps4.asm:48916`): `clr.w $E(a3)` (HP to zero),
`move.w #7, $2(a3)` (the fighter's routine 7, `Character_Dead`) and `bset #2` -
or `bset #6` for an android (`cmpi.w #5, $6(a3)`) - on the status. The routine-7
handler is the port's `Fighter::mark_defeated` (the seal survives, every other
ailment goes); the port emits `Died`. A dead target takes no roll
(`Ability_ProcessRange` skips it); an immune one (biological factor 0) still
spends its roll and cannot land.

### STASISBALL (`$0B`)

Blauzen and Goldine share `EnemyAttack_Blauzen`; its `.stasisball` arm
(`ps4.asm:23404`) keeps `Current_Target_Index` and loads five objects.
Exactly one of them calls `GetEnemySkillEffectAndRange` - `BattleObj_BlauzenStasisBall5`
at its state `loc_1346A` - and `BattleObj_LifeDeletrStasisBall` does the same at
`loc_1286C` for the other routine. Record 11 is `1C 01 08 40 01 0D`: range 8
(the drawn target), STR against STR, efess, miss threshold 64, handler
`AbilityEffect_Paralyze`. A target already paralyzed takes no roll, so the third use
in the Blauzen capture draws only the ability roll.

### DORAN, GELUN and SEALS (`$28`, `$57`, `$29`, Greneris and Radhin)

`EnemyAttack_Juza` compares the ability id arm by arm. `$57` and `$28` write the same
palette tail (`loc_E5E6`), clear the target index and load object `$768`
(`loc_2A814`); `$29` loads `$76C` (`loc_2A74A`). Each object calls
`GetEnemySkillEffectAndRange` once from its state 1 and then only animates the
targets whose effect word is nonzero. The records:

| id | bytes | effect | MEN vs MEN, psychic, miss threshold |
|---|---|---|---|
| `$28` DORAN | `06 82 09 50 02 0B` | AgilityDown | 80 |
| `$57` GELUN | `03 82 09 40 02 0B` | AttackDown | 64 |
| `$29` SEALS | `08 82 09 50 02 0B` | SealTech | 80 |

Where the effects live: `agility_battle` and `atk_pow_battle` are battle-only
cells that `Battle_RestoreStatsAtTurnEnd` does not touch - only `physical_prop` and
sleep - so a debuff lasts until the next `FillBattleStats` (`ps4.asm:11272`),
i.e. the end of the battle; both are recomputed **from the derived value**
(`move.b agility_mod, agility_battle / sub.b d1`), so a second cast does not
stack. A seal is a status bit; `Character_Dead` keeps it
(`Fighter::mark_defeated`) and the revive items clear everything but it
(`andi.b #$90`, `ps4.asm:74506`), but it does not outlive the battle: see
section 5.

### DEBAN (`$2D`, Radhin and ShadowSabr)

Record 45 is `0A 82 02 00 00 00`: range 2 (every enemy slot), MEN selector, **no
resistance selector**, so there is no roll and no draw. `AbilityEffect_DefenseUp`
sets `dfs_pow_battle = dfs_pow + MEN` on every occupied living enemy, the caster
included, again from the derived value.

ShadowSabr's arm (`loc_F89A`) tests first: `move.w $28(a2), d3 / cmp.w $2A(a2), d3 /
bcs.w loc_F826` - when the actor's derived defence is **below** its battle defence
(already raised) the arm branches to `loc_F826` (`ps4.asm:21936`), which clears
`$24(a4)` and loads the ordinary attack objects. A ShadowSabr whose reaction flag
brings DEBAN up again therefore swings instead; in the capture it casts DEBAN once,
at its first turn, and attacks every turn after. The port reports that as
`EffectTurn::Swing`, which returns `false` from `roll_enemy_ability` so the engine
takes the ordinary attack path with `fighter.ability` cleared.

### GIRES (`$3E`, TechMaster)

`EnemyAttack_TechUser` has dedicated arms for `$44`, `$35`, `$48`, `$40`, `$2E` and
`$47`; every other id reaches `loc_EEAA` (`ps4.asm:21266`), which clears
`Current_Target_Index` and picks the object by comparing `$3E` and `$45`: both load
object `$3D4`. GIRES is therefore RES with a different record - `12 82 01 40 00 00`,
effect `$12`, no resistance stat, hit byte 64 - healing the enemy with the lowest
`curr_hp` (`loc_21504`, the earlier slot on a tie) through `Battle_CalcHealing`.
`enemy_skill::resolve_res` is the one implementation for both (GIRES simply adds its record to `is_tech_heal`).

### Fusion (`$12`, ZolSlug)

See [`ENEMY_FUSION.md`](ENEMY_FUSION.md).

## 4. Party defeat is "every member paralyzed or dead"

The check after every action (`loc_66B8`, `ps4.asm:9709`) tests the party first:
every occupied party slot with `status & $46` (paralyzed, dead, android-dead,
`loc_674E`, line 9752) makes `Battle_Routine` `$1A`, the defeat. Only then does it
test the enemy side, for `$44` (`loc_6772`, line 9767, `$18`). Sleep is not in the
mask. STASISBALL and the crawlers' paralysis can therefore end a battle with
nobody at zero HP - the VOICE capture (`f_D7`) is exactly that fight - and the
port's `Battle::settle_outcome` now has the cartridge's rule and its order
(defeat before victory) instead of "no party member has HP".

### Three further rules the captures corrected

- **A missed swing does not mark the enemy physically attacked.**
  `EnemyAI_PhysicalAtkReceived` (`ps4.asm:22816`) reads bit 0 of the reaction
  flags, which `Character_DamageEnemy` sets (`ps4.asm:3946`). `Fighter_TakeDamage`
  returns before it when the slot's hit byte is negative (`tst.b (a0,d0.w) /
  bmi.s`, `ps4.asm:3571-3572`), and a miss leaves `$FF`. The port set the flag on
  every slot a party swing covered, a miss included; the BloodSaber captures
  (the tape party misses it for rounds, and its conditional DEBAN never fires)
  showed it, and `action.rs` now sets the flag on a hit only.
- **The round-end routines do not run in the round that decided the battle.**
  `Battle_RestoreStatsAtTurnEnd` (`ps4.asm:9792`) is reached when the queue runs
  out (`loc_5366`, `ps4.asm:7596`); the check after every action (`loc_66B8`) has
  already sent a decided battle to its victory or defeat routine. A sleeper's wake
  roll is not drawn in that round (`FB_d2`: a victory with Alys asleep, 48 rolls
  in the log, 49 in the port before the fix).
- **The replay compares deaths and battle cells.** See
  [`BATTLE_ORACLE_ARC.md`](../oracle/BATTLE_ORACLE_ARC.md) section 4.

## 5. Lasting state, and who clears it

| state | where it lives | cleared by |
|---|---|---|
| asleep (`$08`) | `Stats::status` | the end-of-round wake roll (`recover_round_status`: odd wakes, agility restored); paralysis (`bclr #3`); the end of the battle |
| paralyzed (`$02`) | `Stats::status`, `agility.battle` = `dexterity.battle` = 1 | enemies: end of round, no draw; party: CURE items/techniques and ANTI (`resolve_recovery`, which also restores AGI/DEX); **not** the end of the battle |
| sealed (`$10`) | `Stats::status` | revival does not (`andi.b #$90`); the end of the battle does |
| AGI down | `agility.battle` | `refresh_battle_stats` (battle end), sleep wake, CURE/ANTI restore |
| ATK down | `attack.battle` | battle end |
| DEF up | `defence.battle` | battle end |

**The end of the battle clears sleep and seal, and only those.** Victory and escape
both end in `Battle_LastMessage` (`ps4.asm:6350`, routine `$2F`): on the press
that closes the window it walks the five party slots with `andi.b #$E7, $16(a0)`
(line 6366) - bits 3 and 4 - and then loads the field (`Game_Mode_Index` 8, line
6390). Poison and paralysis go out with the party (the field's own
`field_status` steps clear them). The first Zio's `$914` object returns to the
field without that routine, so a scripted exit keeps every bit. The port's
`Battle::into_party` was handing the status byte back whole - harmless while no
enemy could sleep or seal a party member, wrong the moment VOICE or SEALS landed
- and now applies the mask (`enemy_effect_tests`: `a_seal_and_a_sleep_end_with_the_battle...`).

## 6. What is not modelled

- **A sealed caster's cast fizzles.** GIRES/RES (`loc_2141C`, frame `$F`) and
  ShadowSabr's DEBAN (`loc_1D83A`, frame `$14`) test `btst #4, $16(a1)` on the
  caster and skip the effect when it is sealed. The port has no way to seal an enemy
  yet (no party technique with effect `$08` is supported), so the branch is
  unreachable; it needs the test the day one exists.
- **The enemies' battle attack and defence cells are not in the log.** The
  oracle's fixture extractor reads `$24` and `$28` (the derived cells) for the
  effect columns, which a battle buff never moves, and no column reads an enemy's
  `$26` / `$2A`. DEBAN's effect on its own side is therefore proven through the
  damage the party deals afterwards (`D8_d0`; a defence raised by one point instead of
  eighteen turns a hit of 1 into a hit of 4); GELUN's, on the party, through the party's
  `atk_bat` cells that `oracle/sweep/arc.py` adds from the log. Adding both sides'
  cells to `oracle/fixture/observations.py`'s `EFFECT_FIELDS` is outside this
  lane's write set and would make the DEBAN proof direct.
- **Carriers no capture has seen use the ability** are not routed
  (`BATTLE_ORACLE_ARC.md` section 3 lists them).
- **SoldrFiend's VOL kill** is read, not observed: the one capture of it is a
  miss. Its object ends in the same `loc_25048` the two observed carriers use
  (`ps4.asm:46878`).

## 7. The Zelan route pairs

What the route from Zelan to Kuran meets and how it was derived is
[`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md) section 6; the captures that saw each
pair are [`BATTLE_ORACLE_ZELAN.md`](../oracle/BATTLE_ORACLE_ZELAN.md).

### One handler per effect byte

THREAD (`$10`, effect `$06`) and POISON (`$11`, effect `$1B`) used to have a
resolver of their own in `enemy_skill.rs`; DORAN's `$06` and no `$1B` handler lived
here. They are `ROUTES` entries now (`31 CarrionCr`, `32 Caterpillr`), and
`enemy_skill::resolve_thread` / `resolve_poison` are gone, so POISONMIST and
SHADOWBIND share a handler with the crawler arm of the same effect instead of
copying it. The seven POISON fixtures of the Motavia sweep replay through the
new path unchanged; THREAD has no committed capture (no CarrionCr formation is in
the swept set), so its evidence stays the unit tests in
`enemy_effect_crawler_tests.rs` and the chain read in `THREAD.md`. A route is
the `(enemy, ability)` pair plus the record's effect byte and range nibble, which
is weaker than the old full-record pins; the record still drives every number.

### POISONMIST (`$24`, Mistralgec)

Record 36 `1B 01 08 50 01 0D`: effect `$1B` (`AbilityEffect_Poison`,
`ps4.asm:9410`), range 8 (the drawn target), STR against STR, efess factor,
miss threshold 80. `EnemyAttack_SandNewt` (`ps4.asm:22267`) keeps
`Current_Target_Index`, loads object `$244` = `BattleObj_PoisonMist`
(`ps4.asm:42202`), whose state `loc_1F69E` (`ps4.asm:42285`) calls
`GetEnemySkillEffectAndRange` once at its fourth frame (`ps4.asm:42293`). The
handler tests `StatusPoisoned` first (an already poisoned target takes no
roll), rolls once and sets the bit. No `move.w #$C`/`#5` is written by the
object or by the helpers it calls. Mistralgec's own list is `[0 x5, 36 x3]`. 58
FlameNewt shares the routine and its arm but is not on the route and no capture
saw it cast POISONMIST, so it is not routed. Poison does nothing more in battle;
its field consequence (1 HP per four steps) is `field_status.rs`.

### SLEEP GAS (`$25`, GerotLux)

Record 37 `07 01 09 50 01 0B`: `AbilityEffect_SleepParalyze` (the VOICE shape),
range 9, STR against STR, psychic factor, threshold 80. `EnemyAttack_AbeFrog`
(`ps4.asm:22246`) clears `Current_Target_Index` for every nonzero ability
(`loc_FCA0`, 22257) and loads object `$250` = `BattleObj_SleepGas`
(`ps4.asm:41714`): its state `+4` jumps to `loc_24D76` (`ps4.asm:48702`), which
makes the one call (48706) and then sets bit 3 of the status of every party
fighter whose effect word is nonzero (48714-48724) - the same `bset #3` as
`loc_25074`. A sleeping or paralyzed member takes no roll. The sleepers wake by
the end-of-round roll like VOICE's.

### SHADOWBIND (`$4B`, ChaosSorcr and ChaosSorcr2)

Record 75 `06 02 09 40 02 0B`: `AbilityEffect_AgilityDown`, range 9, **MEN against
MEN** (byte 1 is `$02`, not `$82`), psychic factor, threshold 64.
`EnemyAttack_ChaosSorcr` (`ps4.asm:20816`), arm `loc_E83E` (20868), clears
`Current_Target_Index` and loads object `$724` = `loc_2BCEA` (`ps4.asm:57360`);
at frame `$F` of its first state it calls `GetEnemySkillEffectAndRange` once
(57411) and spawns a child `$3F8` (`loc_20A0C`, `ps4.asm:44013`) for every
slot that landed. The children are sprites. The new battle agility is the
caster's MEN subtracted from the *modified* value, floored at 1, exactly as
THREAD's and DORAN's.

### WARNING (`$14`, CommndBall) is a refill

Record 20 `1E 00 2C 00 00 00 00 00` has the effect byte of the two Fission
records, and its object ends the same way: `$1CC` (`loc_1879A`,
`ps4.asm:33852`) finishes with `clr.w (a4) / movea.l $7C(a4), a1 /
jmp loc_14CBE` (`ps4.asm:33846-33850`) - the refill of the neighbour slot
`EnemyAI_EmptySpace` named, after an alarm in front of it (`$198`,
`loc_185EC`, `ps4.asm:33725`). 45 CommndBall's init is `EnemyInit_Tower`
(`ps4.asm:18243`), which clears the objects beside it like Igglanova's does, so
`CommndBall` fights alone until a WARNING calls one of its two FloatMine2 back. Unless
the next fighter is a FloatMine: the routine returns first when `$52(a4)`, the next
fighter object's enemy id, is `$2C` (`ps4.asm:18244-18246`; formation `$125` queues
all three, [`BATTLE_ORACLE_ZELAN.md`](../oracle/BATTLE_ORACLE_ZELAN.md) section 6).
The port already ran Fission's refill (`enemy_skill::resolve_fission`); the
WARNING record joins it (`EnemySkill::is_refill`) and `initialize_enemies`
covers 39 Tower and 45. The first version of this lane read WARNING as a spent
turn with an alarm and the capture refuted it: the log's first round queue holds
the CommndBall alone.

### BARRIER (`$1D`, Siren386 and Browren486)

`EnemyAttackOffs` maps enemies 47/48/49 to `EnemyAttack_Warren286`
(`ps4.asm:19254-19256`, 22515). Siren386's conditional `$08` tests magic-hit
reaction bit 1 and clears the whole reaction byte (`EnemyAI_MagicDamageReceived`,
22500-22511); Browren486's regular slot reaches the same `$1D` arm.
`loc_100A6` (`ps4.asm:22559`, compare at 22567-22570) compares **signed** battle MDEF (`$2E`) against
derived MDEF (`$2C`). If battle MDEF is greater, `loc_10016` clears the ability
and takes the physical swing. Otherwise it clears the target and loads the
`$1DC` children and `$1E0` parent (22571-22592).

The `$1E0` parent `loc_1769E` (32706) calls `GetEnemySkillEffectAndRange`
once at its handoff (32771). `AbilityEffect_MagicDefenseUp` (9220-9233)
visits living enemies in range 2 and sets each battle MDEF to its own derived
MDEF plus the caster's power stat, with 16-bit addition. It does not stack.
The decoded BARRIER record selects STR and no resistance stat, so the effect
consumes no RNG (`Effect_SetupSkillParams`, 9576-9595). The guard examines the
caster alone; lowered recipients are still reset from their derived values.

The script captures and exact generic replays are in
[BATTLE_ORACLE_X86.md](../oracle/BATTLE_ORACLE_X86.md). Unit negative controls
reject an untraced carrier or wrong effect record and pin the signed guard,
word wrap, living-slot range and second-cast fallback.

### What is deferred, and why

| pair | reason |
|---|---|
| 58 FlameNewt `$24`, 105 ShadMirage and 132 DarkForce3 `$4B` | Share a routine with a routed pair, are not on the route, were not captured. |
