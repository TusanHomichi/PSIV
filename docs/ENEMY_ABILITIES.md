# Enemy ability inventory

The work queue for enemy abilities: one row per distinct nonzero **regular**
ability id the cartridge can roll, with the dispatch and the effect handler read
out of the disassembly, and whether `psiv-core` implements it. Read
`docs/BATTLE_SCOUT.md` §7 for the roll and `docs/THREAD.md` for the shape of one
finished dispatch (THREAD). Nothing here changes code; it is the map for doing so
one ability at a time.

**83 distinct nonzero regular ability ids.** 15 implemented,
68 unsupported: 48 damage, 4 scripted/custom, 16 status/stat effect.

## 1. Method

### Data read

| source | field / what was taken from it |
|---|---|
| `generated/enemies.json` | 153 records. `ai.regular_ability_ids` = record bytes 36..43, the eight ids `Enemy_Attack` picks from; `ai.condition_ids` / `ai.conditional_ability_ids` = bytes 28..35; `symbol` for the enemy name. |
| `generated/enemy_skills.json` | 112 eight-byte records (`EnemySkillData`, `0x28336C`): `symbol`, `display_name`, `effect_id` (byte 0), `relevant_stat` (byte 1), `target_id` (byte 2), `power_or_hit_chance` (byte 3), `resistance_stat` (byte 4), `element` (byte 5). |
| `generated/formations.json` | 504 regular + 27 boss formations; each enemy entry's `enemy.id`. |
| `generated/formation_indexes.json` | `Battle_FormationIndexes`: 68 groups x 32 formation ids. |
| `generated/encounters.json` | `Battle_EnemyFormationIndexes`, one byte per map (group / position grid / no encounters). |
| `generated/maps.json` | map symbols per map id. |
| `reference/ps4disasm/ps4.asm` | `EnemyAttackOffs`, the `EnemyAttack_*` bodies, the `BattleObjsGroup*Ptrs` tables, `AbilityEffectsOffs`, the object tails. |

### Ability universe, and how completeness was checked

Union of `ai.regular_ability_ids` over all 153 records with the zeros removed:
**83** ids, `$02`..`$70` (2..112), all inside the 112-record skill table.
53 of the 153 enemies have an all-zero regular list and never take the ability
branch. Three independent checks were run:

1. The same set re-derived straight from `raw_hex` bytes 36..43 of every record,
   ignoring the parsed fields — identical (0 mismatches across 153 records).
2. Every id is inside `1..112` and every id has a row in §2 exactly once
   (83 rows, 83 distinct ids, set equality asserted while building this file).
3. Every id resolves to a skill record whose `symbol` is non-empty, and every
   cited `EnemyAttack_*` routine has an entry in `EnemyAttackOffs`
   (`ps4.asm:19206`, 153 entries, ids `$00-$98`).

### Dispatch (the `EnemyAttack_*` → object column)

- `EnemyAttack` (`ps4.asm:19138`) rolls one of the eight regular ids at
  `ps4.asm:19146-19154` (never the same index twice in a row), then hands the
  fighter to `EnemyAttackOffs[enemy_id]` — 153 table entries resolving to 74
  distinct routines, so each carrier family has its own body.
- Inside a routine the object is chosen from `$24(a4)` (the ability slot). The
  branch chains (`cmpi.w #$ID, $24(a4)` + `tst.w $24(a4)` with a fall-through)
  were read from the routine bodies, and the object each branch stores with
  `move.w #$ID, (a1)` / `_move.w #$ID, 0(a1)` was read from the branch itself.
- Object ids were resolved through the ten `BattleObjsGroup*Ptrs` tables
  (`ps4.asm:26745`, `30968`, `37718`, `43699`, `51949`, `59567`, `66401`,
  `70300`, `70321`, `74561`) and are cited at the **object label's definition**
  line, so the citation line is where the object starts. Unnamed objects keep
  their `loc_*` label.
- The `-------` banner comments between routines are formatting, not boundaries:
  `EnemyAttack_ForcedFly` (`ps4.asm:23567`) falls through into
  `EnemyAttack_Helex`'s body, `EnemyAttack_WorkerPod` (`ps4.asm:23182`) branches
  into `EnemyAttack_Tarantella`, and `EnemyAttack_Sweeper` (`ps4.asm:23676`)
  jumps into `EnemyAttack_Seeker` for `$04`. Those are called out in §5 and are
  the reason the table sometimes lists the same object twice for two enemies.

### Effect handler (`AbilityEffect_*` column)

Enemy skill byte 0 indexes `AbilityEffectsOffs` (`ps4.asm:9036`), dispatched
through `Ability_GetEffect` (`ps4.asm:9026`) for every record in the table,
including the `AbilityEffect_None` ones. Two things to know:

- `$13-$16` sit in an `if bugfixes=1` block (line 9056) inside `AbilityEffectsOffs` (`ps4.asm:9036`). This fork build
  takes the bugfix arm; the retail arm is `RestoreAgiAndDex` for all four. No
  regular enemy ability uses those ids, so no row moves either way.
- `$2C` is one past the end: 44 entries cover `$00-$2B` and `TRAP #2` does not
  bound-check, so dispatching it jumps to an odd address. The one carrier of
  effect `$2C` is ability 112 (§5).

### Damage versus status (the class column)

For an enemy skill the effect handler applies only the status/stat change; damage
is requested by the animation object. The class therefore comes from the object
chain, by one mechanical rule:

- An object requests a damage reaction when its code writes fighter routine 5 or
  `$C` to a target pointer — `move.w #5, $2(aX)` / `move.w #$C, $2(aX)` with `aX`
  not `a4` (an object keeps its own record in `a4`). 5 is
  `Enemy_DamageAnimation`/`Character_DamageAnimation`, `$C` is
  `Fighter_TakeDamage` (`BattleEnemyRoutinePtrs` `ps4.asm:1077`,
  `BattleCharacterRoutinePtrs` `ps4.asm:1033`). THREAD's "never requests a
  physical damage reaction" is exactly the absence of this write, and
  `BattleObj_AcidBreath`'s single guarded request is its presence.
- The same write inside the shared object tails `loc_24A6C` … `loc_24BDC`
  (`ps4.asm:48468-48584`) counts, because objects reach them with `jmp`/`bra.w`;
  `loc_24BB6` (`ps4.asm:48562`) hits all five character slots.
- An object's code region runs from its label to the next object label, which
  includes its internal `loc_*` state handlers, and objects it loads via
  `move.w #$ID, (a1)` are followed recursively — several routines keep the actual
  hit request in the child (`BattleObj_HelexFlameBolt` asks for nothing itself,
  `BattleObj_HelexFlameBolt2` asks for the hit).

Classes: `damage` — the chain requests damage; `status/stat effect` — it does not
and the effect id resolves to a status/stat handler; `scripted/custom` — the
routine ignores or rewrites the rolled id, or loads no object at all; `unknown` —
not established. This is static reading, not a hardware capture: a `damage` row
says the ability goes through the damage pipeline, not what numbers it produces.

### Formations and maps

- "Formations containing those enemies" = the `n`/504 regular formations whose
  enemy list holds any carrier of the ability. The formation group masks (bytes
  5-6) are **not** a spawn filter: `loc_7F2E` (`ps4.asm:11911`) expands every
  `(enemy id, position)` pair up to the `$FF` terminator, and their only readers,
  `loc_7B94` (`ps4.asm:11590`), use them to pick the group leader's sprite.
  Boss formations are counted separately — `Event_Battle_Index` reaches them, no
  map does.
- A map reaches a formation through `Battle_EnemyFormationIndexes`
  (`Battle_SetupEnemyData`, `ps4.asm:11813`): a `group` map selects that one
  32-entry group, a `position_grid` map (Motavia, Dezolis) every group its cells
  can pick plus its vehicle groups. Group entries are a 32-way roll, so "the map
  can meet this ability" means one of the 32 formations carries it.

## 2. Regular ability table

`†` marks an unsupported row that also has a cutscene-gated carrier routine
(§5): the class shown is the ordinary dispatch, and the listed object is what
runs when the gate is clear.

| ability | record | `EnemyAttack_*` → object | `AbilityEffect_*` | carriers | class | status |
|---|---|---|---|---|---|---|
| `$02` (2) **FlameBolt**<br>FLAME BOLT | eff `$01` · stat $01 (strength) · tgt 8 · pow 80 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_ForcedFly` (`ps4.asm:23567`)<br>→ BattleObj_HelexFlameBolt (`ps4.asm:30279`)<br>`EnemyAttack_Helex` (`ps4.asm:23574`)<br>→ BattleObj_HelexFlameBolt (`ps4.asm:30279`) | `AbilityEffect_None` (`ps4.asm:9092`) | 0 Helex, 5 ForcedFly<br>14/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for both carriers (`DAMAGE_SKILL_ROUTES`); the child object holds the one damage request |
| `$03` (3) **RailGun**<br>RAIL-GUN | eff `$01` · stat $05 (attack) · tgt 8 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_GunnerBit` (`ps4.asm:23604`)<br>→ BattleObj_GunnerBitAtk (`ps4.asm:30361`)<br>BattleObj_GunnerBitAtk2 (`ps4.asm:30450`) | `AbilityEffect_None` (`ps4.asm:9092`) | 2 GunnerBit<br>6/504 formations | damage | unsupported |
| `$04` (4) **LasrCannon**<br>LASRCANNON | eff `$01` · stat $05 (attack) · tgt 8 · pow 0 · res $06 (defense) · el `2` energy | `EnemyAttack_Loader` (`ps4.asm:22448`)<br>→ loc_16C04 (`ps4.asm:31936`)<br>`EnemyAttack_ProtectBit` (`ps4.asm:23637`)<br>→ BattleObj_ProtectBitAtk (`ps4.asm:30592`)<br>BattleObj_ProtectBitAtk2 (`ps4.asm:30706`)<br>`EnemyAttack_Seeker` (`ps4.asm:23663`)<br>→ BattleObj_SeekerAtk (`ps4.asm:30851`)<br>`EnemyAttack_Sweeper` (`ps4.asm:23676`)<br>→ BattleObj_SeekerAtk (`ps4.asm:30851`) | `AbilityEffect_None` (`ps4.asm:9092`) | 4 ProtectBit, 7 Seeker, 8 Sweeper, 51 Loader, 52 Debugger<br>30/504 formations | damage | unsupported |
| `$05` (5) **Lightning**<br>LIGHTNING | eff `$01` · stat $01 (strength) · tgt 8 · pow 40 · res $07 (magic_defense) · el `7` electric | `EnemyAttack_Sweeper` (`ps4.asm:23676`)<br>→ BattleObj_SweeperAtk (`ps4.asm:30911`) | `AbilityEffect_None` (`ps4.asm:9092`) | 8 Sweeper<br>6/504 formations | damage | unsupported |
| `$07` (7) **Fission2**<br>FISSION | eff `$1E` · stat $00 (none) · tgt 10 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_FloatMine` (`ps4.asm:22675`) — no object; the routine clears `$24(a4)` | `AbilityEffect_None` (`ps4.asm:9092`) | 50 FloatMine2<br>2/504 formations | — | implemented — two different paths, the id alone is not the dispatch: `enemy_skill::resolve_fission` for 12 Igglanova / 13 Guilgenova (§3, reached through `fission_neighbor`), and `enemy_skill::resolve_no_effect_turn` for 50 FloatMine2, whose own `EnemyAttack_FloatMine` has no `$07` arm and spends the turn at `loc_10406` (`ps4.asm:22781`, `BattleEvent::EnemyAbilityWasted`) |
| `$08` (8) **SpiralBld**<br>SPIRAL BLD | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_Locusta` (`ps4.asm:23472`)<br>→ BattleObj_LocustaAtk (`ps4.asm:29448`)<br>BattleObj_LocustaAtk2 (`ps4.asm:29506`)<br>BattleObj_LocustaAtk3 (`ps4.asm:29541`)<br>BattleObj_LocustaSpiralBld (`ps4.asm:29175`) | `AbilityEffect_None` (`ps4.asm:9092`) | 15 Fanbite<br>6/504 formations | damage | unsupported |
| `$09` (9) **MotrCannon**<br>MOTRCANNON | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_Slave` (`ps4.asm:23451`)<br>→ BattleObj_SlaveMotrCannon (`ps4.asm:28736`)<br>BattleObj_SlaveMotrCannon2 (`ps4.asm:28957`) | `AbilityEffect_None` (`ps4.asm:9092`) | 18 Servant<br>6/504 formations (+1 boss) | damage | unsupported |
| `$0B` (11) **StasisBall**<br>STASISBALL | eff `$1C` · stat $01 (strength) · tgt 8 · pow 64 · res $01 (strength) · el `13` efess | `EnemyAttack_Blauzen` (`ps4.asm:23346`)<br>→ BattleObj_BlauzenStasisBall (`ps4.asm:28127`)<br>BattleObj_BlauzenStasisBall5 (`ps4.asm:27819`)<br>BattleObj_BlauzenStasisBall4 (`ps4.asm:27928`)<br>BattleObj_BlauzenStasisBall3 (`ps4.asm:27983`)<br>BattleObj_BlauzenStasisBall2 (`ps4.asm:28057`)<br>`EnemyAttack_LifeDeletr` (`ps4.asm:23124`)<br>→ BattleObj_LifeDeletrStasisBall (`ps4.asm:26956`)<br>BattleObj_LifeDeletrStasisBall2 (`ps4.asm:26855`)<br>BattleObj_LifeDeletrStasisBall3 (`ps4.asm:26823`) | `AbilityEffect_Paralyze` (`ps4.asm:9424`) | 19 Blauzen, 21 Goldine, 26 LifeDeletr<br>11/504 formations | status/stat effect | unsupported |
| `$0E` (14) **MicroMissl**<br>MICROMISSL | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_BalDuel` (`ps4.asm:23316`)<br>→ loc_378FC (`ps4.asm:72007`)<br>`EnemyAttack_LifeDeletr` (`ps4.asm:23124`)<br>→ BattleObj_LifeDeletrMicroMissl (`ps4.asm:52229`) | `AbilityEffect_None` (`ps4.asm:9092`) | 26 LifeDeletr, 28 DragerDuel, 29 JurafaDuel<br>13/504 formations | damage | unsupported |
| `$0F` (15) **FlamLaunch**<br>FLAMLAUNCH | eff `$01` · stat $01 (strength) · tgt 8 · pow 64 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_BalDuel` (`ps4.asm:23316`)<br>→ loc_37704 (`ps4.asm:71820`) | `AbilityEffect_None` (`ps4.asm:9092`) | 29 JurafaDuel<br>4/504 formations | damage | unsupported |
| `$10` (16) **Thread**<br>THREAD | eff `$06` · stat $01 (strength) · tgt 8 · pow 64 · res $03 (agility) · el `1` physical | `EnemyAttack_Crawler` (`ps4.asm:23081`)<br>→ BattleObj_Thread (`ps4.asm:36186`) | `AbilityEffect_AgilityDown` (`ps4.asm:9139`) | 31 CarrionCr<br>3/504 formations | — | implemented — `enemy_skill::resolve_thread` |
| `$11` (17) **Poison**<br>POISON | eff `$1B` · stat $01 (strength) · tgt 8 · pow 64 · res $01 (strength) · el `13` efess | `EnemyAttack_Crawler` (`ps4.asm:23081`)<br>→ BattleObj_Poison (`ps4.asm:36279`) | `AbilityEffect_Poison` (`ps4.asm:9410`) | 32 Caterpillr<br>7/504 formations | status/stat effect | implemented — `enemy_skill::resolve_poison` ([ENEMY_POISON.md](ENEMY_POISON.md)) |
| `$13` (19) **CellSplit**<br>CELL SPLIT | eff `$01` · stat $01 (strength) · tgt 9 · pow 80 · res $06 (defense) · el `1` physical | `EnemyAttack_MetaSlug` (`ps4.asm:22931`)<br>→ BattleObj_CellSplit (`ps4.asm:35240`)<br>BattleObj_CellSplit2 (`ps4.asm:35344`)<br>BattleObj_CellSplit3 (`ps4.asm:35392`)<br>BattleObj_CellSplit4 (`ps4.asm:35445`)<br>BattleObj_CellSplit5 (`ps4.asm:35493`) | `AbilityEffect_None` (`ps4.asm:9092`) | 37 SnowSlug, 38 FractOoze, 137 FractOoze2<br>5/504 formations (+1 boss) | damage | unsupported |
| `$16` (22) **Flash**<br>FLASH | eff `$21` · stat $01 (strength) · tgt 9 · pow 64 · res $02 (mental) · el `11` psychic | `EnemyAttack_ArmDrone` (`ps4.asm:22789`)<br>→ loc_1886C (`ps4.asm:33905`) | `AbilityEffect_DexterityDown` (`ps4.asm:9457`) | 43 StarDrone<br>7/504 formations | damage | unsupported |
| `$17` (23) **Waiting**<br>WAITING | eff `$22` · stat $00 (none) · tgt 0 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_FloatMine` (`ps4.asm:22675`) — no object; the routine clears `$24(a4)` | `AbilityEffect_None` (`ps4.asm:9092`) | 44 FloatMine, 46 VopalSphre, 50 FloatMine2<br>10/504 formations | — | implemented — `enemy_skill::resolve_no_effect_turn` (`rust/psiv-core/src/battle/enemy_skill.rs`): all three carriers are `EnemyAttack_FloatMine` entries (`$2C`/`$2E`/`$32`) whose routine has no `$17` arm, so `loc_10406` (`ps4.asm:22781`) spends the turn about to happen, exactly as the name says. `rust/psiv-godot/src/battle/timeline.rs` narrates `BattleEvent::EnemyAbilityWasted` as an empty line with no beat — which is what retail shows — and its `narration` match now names every variant, so a new core event fails to compile there until it is given its line |
| `$19` (25) **Detonation**<br>DETONATION | eff `$24` · stat $05 (attack) · tgt 9 · pow 24 · res $06 (defense) · el `1` physical | `EnemyAttack_FloatMine` (`ps4.asm:22675`)<br>→ loc_17B5E (`ps4.asm:33029`)<br>loc_17DA6 (`ps4.asm:33183`)<br>loc_17E1A (`ps4.asm:33211`)<br>loc_17F6C (`ps4.asm:33295`)<br>loc_1804A (`ps4.asm:33351`)<br>loc_18144 (`ps4.asm:33412`)<br>loc_1816A (`ps4.asm:33424`)<br>loc_1824A (`ps4.asm:33483`)<br>loc_18270 (`ps4.asm:33495`)<br>loc_18344 (`ps4.asm:33550`) | `AbilityEffect_None` (`ps4.asm:9092`) | 45 CommndBall<br>2/504 formations | damage | unsupported |
| `$1C` (28) **FlareShot**<br>FLARE SHOT | eff `$01` · stat $05 (attack) · tgt 8 · pow 16 · res $06 (defense) · el `2` energy | `EnemyAttack_DarkForce1` (`ps4.asm:20030`)<br>→ loc_31F60 (`ps4.asm:64584`)<br>`EnemyAttack_Warren286` (`ps4.asm:22515`)<br>→ loc_1786E (`ps4.asm:32828`) | `AbilityEffect_None` (`ps4.asm:9092`) | 47 Warren286, 48 Siren386, 49 Browren486, 130 DarkForce1<br>13/504 formations (+1 boss) | damage † | unsupported |
| `$1D` (29) **Barrier**<br>BARRIER | eff `$0B` · stat $01 (strength) · tgt 2 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Warren286` (`ps4.asm:22515`)<br>→ loc_1769E (`ps4.asm:32706`)<br>loc_175A2 (`ps4.asm:32636`)<br>loc_174BE (`ps4.asm:32575`)<br>loc_1740E (`ps4.asm:32526`)<br>loc_17364 (`ps4.asm:32478`)<br>loc_172CA (`ps4.asm:32429`)<br>loc_171D0 (`ps4.asm:32358`) | `AbilityEffect_MagicDefenseUp` (`ps4.asm:9220`) | 49 Browren486<br>5/504 formations | status/stat effect | unsupported |
| `$1F` (31) **DblSlash**<br>DBL SLASH | eff `$01` · stat $05 (attack) · tgt 8 · pow 32 · res $06 (defense) · el `1` physical | `EnemyAttack_Loader` (`ps4.asm:22448`)<br>→ loc_16D1C (`ps4.asm:32015`)<br>loc_16E9E (`ps4.asm:32135`)<br>`EnemyAttack_SnowMole` (`ps4.asm:19620`)<br>→ BattleObj_EnemyDblSlash (`ps4.asm:68007`) | `AbilityEffect_None` (`ps4.asm:9092`) | 52 Debugger, 53 Dominator, 145 RedMole, 146 HungryMole<br>20/504 formations (+1 boss) | damage | unsupported |
| `$20` (32) **PhononMasr**<br>PHONONMASR | eff `$01` · stat $01 (strength) · tgt 9 · pow 88 · res $06 (defense) · el `1` physical | `EnemyAttack_DarkForce1` (`ps4.asm:20030`)<br>→ loc_31CDC (`ps4.asm:64401`)<br>`EnemyAttack_Loader` (`ps4.asm:22448`)<br>→ loc_169C6 (`ps4.asm:31782`) | `AbilityEffect_None` (`ps4.asm:9092`) | 53 Dominator, 130 DarkForce1<br>4/504 formations (+2 boss) | damage † | unsupported |
| `$21` (33) **FireBreath**<br>FIREBREATH | eff `$01` · stat $01 (strength) · tgt 8 · pow 32 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_GyLaguiah` (`ps4.asm:20519`)<br>→ loc_2A01A (`ps4.asm:55324`)<br>`EnemyAttack_ProfoundDarkness1` (`ps4.asm:19823`)<br>→ loc_30022 (`ps4.asm:62419`)<br>`EnemyAttack_Ripper` (`ps4.asm:21587`)<br>→ loc_23D0A (`ps4.asm:47507`)<br>`EnemyAttack_SandNewt` (`ps4.asm:22267`)<br>→ BattleObj_FireBreath (`ps4.asm:42121`)<br>`EnemyAttack_StoneHeads` (`ps4.asm:22350`)<br>→ loc_16366 (`ps4.asm:31333`)<br>loc_16406 (`ps4.asm:31380`) | `AbilityEffect_None` (`ps4.asm:9092`) | 58 FlameNewt, 59 StoneHeads, 83 Ripper, 84 BladeRight, 117 GyLaguiah, 133 ProfoundDarkness1<br>25/504 formations (+3 boss) | damage † | unsupported |
| `$22` (34) **RayBreath**<br>RAY BREATH | eff `$01` · stat $01 (strength) · tgt 8 · pow 72 · res $07 (magic_defense) · el `2` energy | `EnemyAttack_GyLaguiah` (`ps4.asm:20519`)<br>→ loc_29EEE (`ps4.asm:55243`)<br>`EnemyAttack_ProfoundDarkness1` (`ps4.asm:19823`)<br>→ loc_2FE7C (`ps4.asm:62296`)<br>`EnemyAttack_StoneHeads` (`ps4.asm:22350`)<br>→ loc_1628A (`ps4.asm:31273`)<br>loc_16406 (`ps4.asm:31380`) | `AbilityEffect_None` (`ps4.asm:9092`) | 60 CrminHeads, 61 BlindHeads, 118 LwAddmer, 133 ProfoundDarkness1<br>15/504 formations (+1 boss) | damage † | unsupported |
| `$23` (35) **SuperSonic**<br>SUPERSONIC | eff `$01` · stat $01 (strength) · tgt 9 · pow 150 · res $06 (defense) · el `1` physical | `EnemyAttack_DezoOwl` (`ps4.asm:19658`)<br>→ BattleObj_OwlSupersonic (`ps4.asm:68761`) | `AbilityEffect_None` (`ps4.asm:9092`) | 142 Skytiara<br>7/504 formations | damage | unsupported |
| `$24` (36) **PoisonMist**<br>POISONMIST | eff `$1B` · stat $01 (strength) · tgt 8 · pow 80 · res $01 (strength) · el `13` efess | `EnemyAttack_SandNewt` (`ps4.asm:22267`)<br>→ BattleObj_PoisonMist (`ps4.asm:42202`) | `AbilityEffect_Poison` (`ps4.asm:9410`) | 57 Mistralgec, 58 FlameNewt<br>6/504 formations | status/stat effect | unsupported |
| `$25` (37) **SleepGas**<br>SLEEP GAS | eff `$07` · stat $01 (strength) · tgt 9 · pow 80 · res $01 (strength) · el `11` psychic | `EnemyAttack_AbeFrog` (`ps4.asm:22246`)<br>→ BattleObj_SleepGas (`ps4.asm:41714`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 63 GerotLux<br>3/504 formations | status/stat effect | unsupported |
| `$26` (38) **Shift**<br>SHIFT | eff `$09` · stat $82 (mental) · tgt 3 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2AAEE (`ps4.asm:56081`) | `AbilityEffect_AttackUp` (`ps4.asm:9181`) | 116 Radhin<br>4/504 formations | status/stat effect | unsupported |
| `$27` (39) **Saner**<br>SANER | eff `$0C` · stat $82 (mental) · tgt 2 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A91A (`ps4.asm:55950`) | `AbilityEffect_AgilityUp` (`ps4.asm:9237`) | 116 Radhin<br>4/504 formations | status/stat effect | unsupported |
| `$28` (40) **Doran**<br>DORAN | eff `$06` · stat $82 (mental) · tgt 9 · pow 80 · res $02 (mental) · el `11` psychic | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A814 (`ps4.asm:55873`) | `AbilityEffect_AgilityDown` (`ps4.asm:9139`) | 115 Greneris<br>1/504 formations | status/stat effect | unsupported |
| `$29` (41) **Seals**<br>SEALS | eff `$08` · stat $82 (mental) · tgt 9 · pow 80 · res $02 (mental) · el `11` psychic | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A74A (`ps4.asm:55820`) | `AbilityEffect_SealTech` (`ps4.asm:9167`) | 115 Greneris, 116 Radhin<br>5/504 formations | status/stat effect | unsupported |
| `$2A` (42) **Rimit**<br>RIMIT | eff `$07` · stat $82 (mental) · tgt 9 · pow 32 · res $02 (mental) · el `11` psychic | `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`)<br>→ BattleObj_EnemyRimit (`ps4.asm:38366`)<br>BattleObj_AcidBreathChild (`ps4.asm:38622`)<br>`EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2ACD6 (`ps4.asm:56212`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 77 TechPlant, 115 Greneris<br>3/504 formations | damage | unsupported |
| `$2B` (43) **Needle**<br>NEEDLE | eff `$01` · stat $05 (attack) · tgt 8 · pow 16 · res $06 (defense) · el `1` physical | `EnemyAttack_Scorpirus` (`ps4.asm:22062`)<br>→ BattleObj_Needle (`ps4.asm:40740`)<br>`EnemyAttack_SnowMole` (`ps4.asm:19620`)<br>→ BattleObj_MoleNeedle (`ps4.asm:68370`) | `AbilityEffect_None` (`ps4.asm:9092`) | 68 Rajago, 69 BiterFly, 146 HungryMole<br>13/504 formations | damage | unsupported |
| `$2D` (45) **Deban**<br>DEBAN | eff `$0A` · stat $82 (mental) · tgt 2 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A660 (`ps4.asm:55760`) | `AbilityEffect_DefenseUp` (`ps4.asm:9203`) | 116 Radhin<br>4/504 formations | status/stat effect | unsupported |
| `$2E` (46) **Giwat**<br>GIWAT | eff `$01` · stat $82 (mental) · tgt 8 · pow 88 · res $07 (magic_defense) · el `5` water_ice | `EnemyAttack_DElmLars` (`ps4.asm:20222`)<br>→ loc_28F76 (`ps4.asm:54226`)<br>`EnemyAttack_FlattrPlnt` (`ps4.asm:21778`)<br>→ BattleObj_EnemyGiwat (`ps4.asm:38195`)<br>BattleObj_AcidBreathChild (`ps4.asm:38622`)<br>`EnemyAttack_HewGilla` (`ps4.asm:21395`)<br>→ loc_22670 (`ps4.asm:45925`)<br>`EnemyAttack_ShadowSabr` (`ps4.asm:21933`)<br>→ loc_1D5AA (`ps4.asm:39951`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_21852 (`ps4.asm:45013`) | `AbilityEffect_None` (`ps4.asm:9092`) | 71 FrostSaber, 77 TechPlant, 91 HewGilla, 101 DarkWitch, 122 DElmLars, 123 XeAThoul<br>27/504 formations (+2 boss) | — | implemented — `enemy_damage::resolve_damage_skill` for all six carriers (`DAMAGE_SKILL_ROUTES`); each chain makes one guarded `move.w #$C` against `$38` (lines 40042, 48513, 48529, 45310, 48474) |
| `$2F` (47) **Vol**<br>VOL | eff `$02` · stat $82 (mental) · tgt 8 · pow 80 · res $02 (mental) · el `10` biological | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2AE8E (`ps4.asm:56333`)<br>`EnemyAttack_ShadowSabr` (`ps4.asm:21933`)<br>→ loc_1D2F4 (`ps4.asm:39774`)<br>`EnemyAttack_TwinArms` (`ps4.asm:21455`)<br>→ loc_23216 (`ps4.asm:46736`) | `AbilityEffect_Death` (`ps4.asm:9098`) | 72 BloodSaber, 88 SoldrFiend, 115 Greneris<br>8/504 formations | damage | unsupported |
| `$30` (48) **Distortion**<br>DISTORTION | eff `$01` · stat $01 (strength) · tgt 9 · pow 128 · res $07 (magic_defense) · el `4` gravity | `EnemyAttack_ProfoundDarkness2` (`ps4.asm:19750`)<br>→ loc_2F1B2 (`ps4.asm:61394`) | `AbilityEffect_None` (`ps4.asm:9092`) | 134 ProfoundDarkness2<br>0/504 formations | damage | unsupported |
| `$31` (49) **Gra**<br>GRA | eff `$01` · stat $82 (mental) · tgt 9 · pow 32 · res $07 (magic_defense) · el `4` gravity | `EnemyAttack_DimensWorm` (`ps4.asm:21892`)<br>→ loc_1C99C (`ps4.asm:39050`)<br>loc_1C73E (`ps4.asm:38879`)<br>loc_1C658 (`ps4.asm:38807`) | `AbilityEffect_None` (`ps4.asm:9092`) | 73 DimensWorm, 74 OuterBeast<br>14/504 formations | damage | unsupported |
| `$32` (50) **Gigra**<br>GIGRA | eff `$01` · stat $82 (mental) · tgt 9 · pow 64 · res $07 (magic_defense) · el `4` gravity | `EnemyAttack_DimensWorm` (`ps4.asm:21892`)<br>→ loc_1C99C (`ps4.asm:39050`)<br>loc_1C73E (`ps4.asm:38879`)<br>loc_1C658 (`ps4.asm:38807`) | `AbilityEffect_None` (`ps4.asm:9092`) | 74 OuterBeast<br>7/504 formations | damage | unsupported |
| `$33` (51) **AcidBreath**<br>ACIDBREATH | eff `$01` · stat $01 (strength) · tgt 8 · pow 24 · res $06 (defense) · el `1` physical | `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`)<br>→ BattleObj_AcidBreath (`ps4.asm:38566`)<br>BattleObj_AcidBreathChild (`ps4.asm:38622`)<br>`EnemyAttack_Piercer` (`ps4.asm:21518`)<br>→ loc_23998 (`ps4.asm:47264`)<br>loc_24FD2 (`ps4.asm:48883`) | `AbilityEffect_None` (`ps4.asm:9092`) | 75 FlattrPlnt, 76 FlyScreamr, 85 Piercer, 86 HakenLeft<br>19/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for all four carriers (`DAMAGE_SKILL_ROUTES`); 77 TechPlant shares `EnemyAttack_FlattrPlnt` but never rolls `$33` |
| `$34` (52) **Voice**<br>VOICE | eff `$07` · stat $01 (strength) · tgt 9 · pow 128 · res $02 (mental) · el `11` psychic | `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`)<br>→ BattleObj_Voice (`ps4.asm:38465`)<br>BattleObj_AcidBreathChild (`ps4.asm:38622`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 76 FlyScreamr<br>3/504 formations | status/stat effect | unsupported |
| `$35` (53) **Gizan**<br>GIZAN | eff `$01` · stat $82 (mental) · tgt 9 · pow 48 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_DElmLars` (`ps4.asm:20222`)<br>→ loc_2912C (`ps4.asm:54326`)<br>`EnemyAttack_FlattrPlnt` (`ps4.asm:21778`)<br>→ BattleObj_EnemyGizan (`ps4.asm:38024`)<br>BattleObj_AcidBreathChild (`ps4.asm:38622`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_21176 (`ps4.asm:44547`) | `AbilityEffect_None` (`ps4.asm:9092`) | 77 TechPlant, 101 DarkWitch, 122 DElmLars, 123 XeAThoul, 124 LeFawGan, 125 GiLeFarg<br>17/504 formations (+2 boss) | damage | unsupported |
| `$36` (54) **StrngLight**<br>STRNGLIGHT | eff `$07` · stat $01 (strength) · tgt 8 · pow 48 · res $02 (mental) · el `11` psychic | `EnemyAttack_ToadStool` (`ps4.asm:21737`)<br>→ BattleObj_StrngLight (`ps4.asm:37778`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 79 Shrieker<br>8/504 formations | status/stat effect | unsupported |
| `$37` (55) **SandStorm**<br>SAND STORM | eff `$01` · stat $05 (attack) · tgt 9 · pow 96 · res $06 (defense) · el `1` physical | `EnemyAttack_SandWorm` (`ps4.asm:21658`)<br>→ BattleObj_SandStorm (`ps4.asm:48204`) | `AbilityEffect_None` (`ps4.asm:9092`) | 81 DesrtLeach<br>1/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for 81 DesrtLeach (`DAMAGE_SKILL_ROUTES`); byte 2 is 9, the all-party *nibble*, but the chain makes one request at line 48547 |
| `$38` (56) **Earthquake**<br>EARTHQUAKE | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_KingRappy` (`ps4.asm:19596`)<br>→ BattleObj_KingRappyEarthquake (`ps4.asm:67513`)<br>`EnemyAttack_SandWorm` (`ps4.asm:21658`)<br>→ BattleObj_Earthquake (`ps4.asm:47884`) | `AbilityEffect_None` (`ps4.asm:9092`) | 80 SandWorm, 149 KingRappy<br>1/504 formations (+1 boss) | damage | unsupported |
| `$39` (57) **Maelstrom**<br>MAELSTROM | eff `$01` · stat $05 (attack) · tgt 9 · pow 32 · res $06 (defense) · el `1` physical | `EnemyAttack_SandWorm` (`ps4.asm:21658`)<br>→ BattleObj_Maelstrom (`ps4.asm:47766`) | `AbilityEffect_None` (`ps4.asm:9092`) | 82 Leviathan<br>1/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for 82 Leviathan (`DAMAGE_SKILL_ROUTES`); byte 2 is 9, but the chain makes one request at line 48547 |
| `$3C` (60) **BladeShine**<br>BLADESHINE | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_TwinArms` (`ps4.asm:21455`)<br>→ loc_237AA (`ps4.asm:47129`) | `AbilityEffect_None` (`ps4.asm:9092`) | 87 TwinArms, 88 SoldrFiend<br>7/504 formations | damage | unsupported |
| `$3D` (61) **HakenBolt**<br>HAKEN BOLT | eff `$01` · stat $05 (attack) · tgt 8 · pow 36 · res $06 (defense) · el `1` physical | `EnemyAttack_TwinArms` (`ps4.asm:21455`)<br>→ loc_23544 (`ps4.asm:46959`) | `AbilityEffect_None` (`ps4.asm:9092`) | 87 TwinArms, 88 SoldrFiend<br>7/504 formations | damage | unsupported |
| `$3E` (62) **Gires**<br>GIRES | eff `$12` · stat $82 (mental) · tgt 1 · pow 64 · res $00 (none) · el `0` none | `EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_213DC (`ps4.asm:44701`) | `AbilityEffect_NormalLogic` (`ps4.asm:9282`) | 100 TechMaster<br>6/504 formations | status/stat effect | unsupported |
| `$3F` (63) **FlodBreath**<br>FLODBREATH | eff `$01` · stat $05 (attack) · tgt 8 · pow 20 · res $06 (defense) · el `1` physical | `EnemyAttack_HewGilla` (`ps4.asm:21395`)<br>→ loc_22A90 (`ps4.asm:46199`)<br>`EnemyAttack_Ismounos` (`ps4.asm:21434`)<br>→ BattleObj_FlodBreath (`ps4.asm:46319`) | `AbilityEffect_None` (`ps4.asm:9092`) | 90 Depcen, 91 HewGilla, 92 Elmelew<br>11/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for all three carriers (`DAMAGE_SKILL_ROUTES`); one request at line 48529 in `loc_24B20` |
| `$40` (64) **Wat**<br>WAT | eff `$01` · stat $82 (mental) · tgt 8 · pow 24 · res $07 (magic_defense) · el `5` water_ice | `EnemyAttack_HewGilla` (`ps4.asm:21395`)<br>→ BattleObj_EnemyWat (`ps4.asm:45978`)<br>`EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2B006 (`ps4.asm:56437`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_218D6 (`ps4.asm:45047`) | `AbilityEffect_None` (`ps4.asm:9092`) | 91 HewGilla, 92 Elmelew, 99 TechUser, 100 TechMaster, 114 Juza<br>27/504 formations (+1 boss) | — | implemented — `enemy_damage::resolve_damage_skill` for all five carriers (`DAMAGE_SKILL_ROUTES`); one request each, at line 48529 (91/92), 45310 (99/100) or 56553 (114) |
| `$42` (66) **RaySpear**<br>RAY-SPEAR | eff `$01` · stat $05 (attack) · tgt 8 · pow 16 · res $06 (defense) · el `2` energy | `EnemyAttack_Centaur` (`ps4.asm:21343`)<br>→ loc_220B8 (`ps4.asm:45533`) | `AbilityEffect_None` (`ps4.asm:9092`) | 97 KingSaber, 98 DarkRider<br>9/504 formations | damage | unsupported |
| `$43` (67) **ThrowLancr**<br>THROWLANCR | eff `$01` · stat $01 (strength) · tgt 9 · pow 136 · res $06 (defense) · el `2` energy | `EnemyAttack_Centaur` (`ps4.asm:21343`)<br>→ loc_21ED2 (`ps4.asm:45414`) | `AbilityEffect_None` (`ps4.asm:9092`) | 98 DarkRider<br>5/504 formations | damage | unsupported |
| `$44` (68) **Foi**<br>FOI | eff `$01` · stat $82 (mental) · tgt 8 · pow 20 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2B08E (`ps4.asm:56477`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_21BF0 (`ps4.asm:45229`) | `AbilityEffect_None` (`ps4.asm:9092`) | 99 TechUser, 100 TechMaster, 114 Juza<br>18/504 formations (+1 boss) | — | implemented — `enemy_damage::resolve_damage_skill` for all three carriers (`DAMAGE_SKILL_ROUTES`); one request each, at line 45310 (99/100) or 56553 (114) |
| `$47` (71) **Zan**<br>ZAN | eff `$01` · stat $82 (mental) · tgt 9 · pow 16 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2AF18 (`ps4.asm:56372`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_215D6 (`ps4.asm:44836`) | `AbilityEffect_None` (`ps4.asm:9092`) | 100 TechMaster, 114 Juza<br>6/504 formations (+1 boss) | damage | unsupported |
| `$48` (72) **Gifoi**<br>GIFOI | eff `$01` · stat $82 (mental) · tgt 8 · pow 56 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_DElmLars` (`ps4.asm:20222`)<br>→ loc_2900E (`ps4.asm:54263`)<br>`EnemyAttack_TechUser` (`ps4.asm:21156`)<br>→ loc_21960 (`ps4.asm:45087`) | `AbilityEffect_None` (`ps4.asm:9092`) | 101 DarkWitch, 124 LeFawGan<br>9/504 formations | damage | unsupported |
| `$4A` (74) **StarDust**<br>STAR DUST | eff `$01` · stat $02 (mental) · tgt 9 · pow 72 · res $07 (magic_defense) · el `2` energy | `EnemyAttack_Acacia` (`ps4.asm:21092`)<br>→ loc_20B96 (`ps4.asm:44101`) | `AbilityEffect_None` (`ps4.asm:9092`) | 105 ShadMirage<br>3/504 formations | damage | unsupported |
| `$4B` (75) **ShadowBind**<br>SHADOWBIND | eff `$06` · stat $02 (mental) · tgt 9 · pow 64 · res $02 (mental) · el `11` psychic | `EnemyAttack_Acacia` (`ps4.asm:21092`)<br>→ loc_2078E (`ps4.asm:43835`)<br>`EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2BCEA (`ps4.asm:57360`)<br>`EnemyAttack_DarkForce3` (`ps4.asm:19907`)<br>→ loc_30A38 (`ps4.asm:63083`) | `AbilityEffect_AgilityDown` (`ps4.asm:9139`) | 105 ShadMirage, 111 ChaosSorcr, 132 DarkForce3, 138 ChaosSorcr2<br>11/504 formations (+2 boss) | status/stat effect | unsupported |
| `$4C` (76) **EvilEye**<br>EVIL EYE | eff `$07` · stat $02 (mental) · tgt 8 · pow 64 · res $02 (mental) · el `11` psychic | `EnemyAttack_DarkForce2` (`ps4.asm:19970`)<br>→ loc_30DC0 (`ps4.asm:63323`)<br>`EnemyAttack_Haunt` (`ps4.asm:21005`)<br>→ loc_2CB1C (`ps4.asm:58346`)<br>`EnemyAttack_ProfoundDarkness3` (`ps4.asm:19692`)<br>→ loc_2E524 (`ps4.asm:60482`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 106 Haunt, 107 Spector, 131 DarkForce2, 135 ProfoundDarkness3<br>11/504 formations (+2 boss) | damage † | unsupported |
| `$4D` (77) **Corrsion**<br>CORRSION | eff `$01` · stat $02 (mental) · tgt 9 · pow 64 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2BED2 (`ps4.asm:57485`)<br>`EnemyAttack_DarkForce3` (`ps4.asm:19907`)<br>→ loc_308FA (`ps4.asm:62998`)<br>`EnemyAttack_Haunt` (`ps4.asm:21005`)<br>→ loc_2C98A (`ps4.asm:58230`)<br>`EnemyAttack_Zio2` (`ps4.asm:19519`) — phase counter rewrites `$24(a4)`; with the counter clear the rolled id is dispatched.<br>`$6B`/phase 0 → BattleObj_MagBarrir (`ps4.asm:67319`)<br>`$00` → loc_338E0 (`ps4.asm:66735`)<br>`$6C` → BattleObj_BlackWave2 (`ps4.asm:66922`)<br>`$4D` → loc_33646 (`ps4.asm:66537`)<br>else → loc_335A6 (`ps4.asm:66498`) | `AbilityEffect_None` (`ps4.asm:9092`) | 106 Haunt, 107 Spector, 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 132 DarkForce3, 140 Zio2<br>30/504 formations (+2 boss) | damage † | unsupported |
| `$4E` (78) **DthSpell**<br>DTHSPELL | eff `$02` · stat $02 (mental) · tgt 8 · pow 112 · res $02 (mental) · el `10` biological | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2B8C6 (`ps4.asm:57104`)<br>`EnemyAttack_Haunt` (`ps4.asm:21005`)<br>→ loc_2C854 (`ps4.asm:58148`) | `AbilityEffect_Death` (`ps4.asm:9098`) | 107 Spector, 112 Illusionst, 113 ImagioMage<br>18/504 formations (+1 boss) | status/stat effect | unsupported |
| `$4F` (79) **Hewn**<br>HEWN | eff `$01` · stat $02 (mental) · tgt 9 · pow 56 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2B6F8 (`ps4.asm:56987`)<br>`EnemyAttack_Haunt` (`ps4.asm:21005`)<br>→ loc_2C6E2 (`ps4.asm:58047`)<br>`EnemyAttack_SaLews` (`ps4.asm:20396`)<br>→ loc_299BE (`ps4.asm:54905`)<br>`EnemyAttack_Zio2` (`ps4.asm:19519`) — phase counter rewrites `$24(a4)`; with the counter clear the rolled id is dispatched.<br>`$6B`/phase 0 → BattleObj_MagBarrir (`ps4.asm:67319`)<br>`$00` → loc_338E0 (`ps4.asm:66735`)<br>`$6C` → BattleObj_BlackWave2 (`ps4.asm:66922`)<br>`$4D` → loc_33646 (`ps4.asm:66537`)<br>else → loc_335A6 (`ps4.asm:66498`) | `AbilityEffect_None` (`ps4.asm:9092`) | 108 Phantom, 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews, 138 ChaosSorcr2, 140 Zio2<br>26/504 formations (+2 boss) | damage † | unsupported |
| `$50` (80) **BadSmell**<br>BAD SMELL | eff `$07` · stat $01 (strength) · tgt 9 · pow 96 · res $01 (strength) · el `11` psychic | `EnemyAttack_Zombie` (`ps4.asm:20979`)<br>→ loc_2C42C (`ps4.asm:57853`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 110 Ghoul<br>5/504 formations | status/stat effect | unsupported |
| `$51` (81) **MindBlst**<br>MINDBLST | eff `$07` · stat $02 (mental) · tgt 9 · pow 160 · res $02 (mental) · el `11` psychic | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2C0D4 (`ps4.asm:57615`)<br>`EnemyAttack_DarkForce3` (`ps4.asm:19907`)<br>→ loc_30792 (`ps4.asm:62902`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 112 Illusionst, 113 ImagioMage, 132 DarkForce3<br>13/504 formations (+1 boss) | damage | unsupported |
| `$52` (82) **Tandle**<br>TANDLE | eff `$01` · stat $02 (mental) · tgt 9 · pow 72 · res $07 (magic_defense) · el `7` electric | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2B23C (`ps4.asm:56594`)<br>`EnemyAttack_SaLews` (`ps4.asm:20396`)<br>→ loc_29910 (`ps4.asm:54862`) | `AbilityEffect_None` (`ps4.asm:9092`) | 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews<br>21/504 formations (+1 boss) | damage | unsupported |
| `$54` (84) **BlackWave**<br>BLACK WAVE | eff `$2A` · stat $00 (none) · tgt 8 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Zio` (`ps4.asm:19483`) — phase counter rewrites `$24(a4)`.<br>phase 0 → `$6B` + BattleObj_MagBarrir (`ps4.asm:67319`)<br>phase 1 → `$53` + BattleObj_NightmareFull (`ps4.asm:66960`)<br>else → writes `$54` (BLACK WAVE) + BattleObj_BlackWave3 (`ps4.asm:66897`) | `AbilityEffect_None` (`ps4.asm:9092`) | 139 Zio<br>0/504 formations (+1 boss) | scripted/custom | unsupported |
| `$55` (85) **Legeon**<br>LEGEON | eff `$01` · stat $02 (mental) · tgt 9 · pow 96 · res $07 (magic_defense) · el `2` energy | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2B5FA (`ps4.asm:56873`) | `AbilityEffect_None` (`ps4.asm:9092`) | 113 ImagioMage<br>8/504 formations | damage | unsupported |
| `$56` (86) **ForceFlash**<br>FORCEFLASH | eff `$01` · stat $02 (mental) · tgt 9 · pow 36 · res $07 (magic_defense) · el `2` energy | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A300 (`ps4.asm:55527`) | `AbilityEffect_None` (`ps4.asm:9092`) | 114 Juza, 115 Greneris, 116 Radhin<br>5/504 formations (+1 boss) | damage | unsupported |
| `$57` (87) **Gelun**<br>GELUN | eff `$03` · stat $82 (mental) · tgt 9 · pow 64 · res $02 (mental) · el `11` psychic | `EnemyAttack_Juza` (`ps4.asm:20575`)<br>→ loc_2A814 (`ps4.asm:55873`) | `AbilityEffect_AttackDown` (`ps4.asm:9109`) | 115 Greneris<br>1/504 formations | status/stat effect | unsupported |
| `$58` (88) **LghtBreath**<br>LGHTBREATH | eff `$01` · stat $01 (strength) · tgt 8 · pow 64 · res $07 (magic_defense) · el `7` electric | `EnemyAttack_GyLaguiah` (`ps4.asm:20519`)<br>→ loc_29DC6 (`ps4.asm:55162`) | `AbilityEffect_None` (`ps4.asm:9092`) | 119 CulaBellr<br>2/504 formations | damage | unsupported |
| `$5A` (90) **Flaeli**<br>FLAELI | eff `$01` · stat $02 (mental) · tgt 8 · pow 72 · res $07 (magic_defense) · el `3` fire | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`)<br>→ loc_2BAA2 (`ps4.asm:57218`)<br>`EnemyAttack_DElmLars` (`ps4.asm:20222`)<br>→ loc_28E36 (`ps4.asm:54155`)<br>`EnemyAttack_SaLews` (`ps4.asm:20396`)<br>→ loc_29A76 (`ps4.asm:54949`) | `AbilityEffect_None` (`ps4.asm:9092`) | 111 ChaosSorcr, 112 Illusionst, 113 ImagioMage, 121 SaLews, 125 GiLeFarg, 138 ChaosSorcr2<br>24/504 formations (+2 boss) | damage | unsupported |
| `$5D` (93) **Tandil**<br>TANDIL | eff `$01` · stat $02 (mental) · tgt 9 · pow 104 · res $07 (magic_defense) · el `7` electric | `EnemyAttack_DElmLars` (`ps4.asm:20222`)<br>→ loc_29306 (`ps4.asm:54450`) | `AbilityEffect_None` (`ps4.asm:9092`) | 124 LeFawGan, 125 GiLeFarg<br>6/504 formations | damage | unsupported |
| `$5E` (94) **Megid**<br>MEGID | eff `$01` · stat $82 (mental) · tgt 9 · pow 112 · res $07 (magic_defense) · el `6` anti_evil | `EnemyAttack_ProfoundDarkness3` (`ps4.asm:19692`)<br>→ loc_2DF4A (`ps4.asm:60068`)<br>`EnemyAttack_ReFaze` (`ps4.asm:20193`)<br>→ loc_284C0 (`ps4.asm:53503`) | `AbilityEffect_None` (`ps4.asm:9092`) | 127 ReFaze, 135 ProfoundDarkness3<br>0/504 formations (+1 boss) | damage | unsupported |
| `$5F` (95) **ThndHalbrt**<br>THNDHALBRT | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `7` electric | `EnemyAttack_Lashiec` (`ps4.asm:20116`)<br>→ loc_281F6 (`ps4.asm:53309`) | `AbilityEffect_None` (`ps4.asm:9092`) | 128 Lashiec<br>0/504 formations (+1 boss) | damage | unsupported |
| `$60` (96) **Posession**<br>POSESSION | eff `$07` · stat $02 (mental) · tgt 8 · pow 32 · res $02 (mental) · el `11` psychic | `EnemyAttack_Lashiec` (`ps4.asm:20116`)<br>→ loc_2805C (`ps4.asm:53215`) | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | 128 Lashiec<br>0/504 formations (+1 boss) | damage | unsupported |
| `$61` (97) **AnothrGate**<br>ANOTHRGATE | eff `$01` · stat $02 (mental) · tgt 9 · pow 148 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_Lashiec` (`ps4.asm:20116`)<br>→ loc_27B02 (`ps4.asm:52835`)<br>`EnemyAttack_ProfoundDarkness2` (`ps4.asm:19750`)<br>→ loc_2F060 (`ps4.asm:61289`) | `AbilityEffect_None` (`ps4.asm:9092`) | 128 Lashiec, 134 ProfoundDarkness2<br>0/504 formations (+1 boss) | damage | unsupported |
| `$63` (99) **Burstroc**<br>BURSTROC | eff `$01` · stat $01 (strength) · tgt 9 · pow 72 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_DarkForce1` (`ps4.asm:20030`)<br>→ loc_31AB6 (`ps4.asm:64247`) | `AbilityEffect_None` (`ps4.asm:9092`) | 130 DarkForce1<br>0/504 formations (+1 boss) | scripted/custom | unsupported |
| `$64` (100) **ShdwBreath**<br>SHDWBREATH | eff `$01` · stat $05 (attack) · tgt 8 · pow 112 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_DarkForce2` (`ps4.asm:19970`)<br>→ loc_31378 (`ps4.asm:63744`)<br>`EnemyAttack_ProfoundDarkness1` (`ps4.asm:19823`)<br>→ loc_301B6 (`ps4.asm:62539`) | `AbilityEffect_None` (`ps4.asm:9092`) | 131 DarkForce2, 133 ProfoundDarkness1<br>0/504 formations (+2 boss) | scripted/custom | unsupported |
| `$65` (101) **LightShowr**<br>LIGHTSHOWR | eff `$01` · stat $01 (strength) · tgt 9 · pow 120 · res $07 (magic_defense) · el `7` electric | `EnemyAttack_DarkForce2` (`ps4.asm:19970`)<br>→ loc_30FD2 (`ps4.asm:63467`)<br>`EnemyAttack_ProfoundDarkness2` (`ps4.asm:19750`)<br>→ loc_2EF0E (`ps4.asm:61209`) | `AbilityEffect_None` (`ps4.asm:9092`) | 131 DarkForce2, 134 ProfoundDarkness2<br>0/504 formations (+1 boss) | damage † | unsupported |
| `$69` (105) **Canceling**<br>CANCELING | eff `$27` · stat $00 (none) · tgt 9 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_ProfoundDarkness3` (`ps4.asm:19692`)<br>→ loc_2E2DE (`ps4.asm:60310`) | `AbilityEffect_RestoreStats` (`ps4.asm:9472`) | 135 ProfoundDarkness3<br>0/504 formations | damage | unsupported |
| `$6A` (106) **WindStorm**<br>WIND STORM | eff `$01` · stat $05 (attack) · tgt 9 · pow 32 · res $06 (defense) · el `1` physical | `EnemyAttack_DezoOwl` (`ps4.asm:19658`)<br>→ loc_3526A (`ps4.asm:68568`) | `AbilityEffect_None` (`ps4.asm:9092`) | 143 Owltalon<br>2/504 formations | damage | unsupported |
| `$6C` (108) **BlackWave2**<br>BLACK WAVE | eff `$01` · stat $02 (mental) · tgt 8 · pow 112 · res $07 (magic_defense) · el `1` physical | `EnemyAttack_Zio2` (`ps4.asm:19519`) — phase counter rewrites `$24(a4)`; with the counter clear the rolled id is dispatched.<br>`$6B`/phase 0 → BattleObj_MagBarrir (`ps4.asm:67319`)<br>`$00` → loc_338E0 (`ps4.asm:66735`)<br>`$6C` → BattleObj_BlackWave2 (`ps4.asm:66922`)<br>`$4D` → loc_33646 (`ps4.asm:66537`)<br>else → loc_335A6 (`ps4.asm:66498`) | `AbilityEffect_None` (`ps4.asm:9092`) | 140 Zio2<br>0/504 formations | scripted/custom | unsupported |
| `$6D` (109) **RoundEyes**<br>ROUND EYES | eff `$01` · stat $01 (strength) · tgt 8 · pow 0 · res $06 (defense) · el `1` physical | `EnemyAttack_Rappy` (`ps4.asm:19578`)<br>→ BattleObj_RoundEyes (`ps4.asm:67760`) | `AbilityEffect_None` (`ps4.asm:9092`) | 147 Rappy<br>3/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for 147 Rappy (`DAMAGE_SKILL_ROUTES`); the attack object becomes `BattleObj_RoundEyes`, whose phase 8 jumps to `loc_24B20` (line 67775) for the one request at line 48529 |
| `$6E` (110) **LovelEyes**<br>LOVEL EYES | eff `$01` · stat $05 (attack) · tgt 8 · pow 32 · res $06 (defense) · el `1` physical | `EnemyAttack_Rappy` (`ps4.asm:19578`)<br>→ BattleObj_LovelEyes (`ps4.asm:67729`) | `AbilityEffect_None` (`ps4.asm:9092`) | 148 BlueRappy<br>2/504 formations | — | implemented — `enemy_damage::resolve_damage_skill` for 148 BlueRappy (`DAMAGE_SKILL_ROUTES`); the attack object becomes `BattleObj_LovelEyes`, whose phase 8 jumps to `loc_24B20` (line 67744) for the one request at line 48529 |
| `$70` (112) **BlackWave3**<br>BLACK WAVE | eff `$2C` · stat $00 (none) · tgt 0 · pow 0 · res $00 (none) · el `0` none | `EnemyAttack_Zio3` (`ps4.asm:19410`) — phase counter `$FFFFEE98` rewrites `$24(a4)`; it never dispatches the rolled id.<br>phase 0 → `$6B` + BattleObj_MagBarrir (`ps4.asm:67319`)<br>phase 1 → clear + BattleObj_NightmarePart1 (`ps4.asm:67253`)<br>phase 2 → turn ends, no object<br>phase 3 → `$53` + BattleObj_NightmarePart2 (`ps4.asm:67107`)<br>phase 4 → `$70` + BattleObj_BlackWave1 (`ps4.asm:67019`) | **out of range** (`$2C` > table max `$2B`) — §5 | 152 Zio3<br>0/504 formations (+1 boss) | — | implemented — scripted sequence in `engine::roll_enemy_ability` (`BattleEvent::FirstZioAction`) |

## 3. Abilities reachable only through the conditional-ability path

`Enemy_Attack` rolls a regular id and then lets up to four conditions replace it
with the matching `conditional_ability_ids` entry (`EnemyAIInstructionsOffs`,
`ps4.asm:19364`; 20 entries, ids 0-15 and 17-19). The ids below appear in no
regular list, so they are outside §2 — but they share the same routines and
objects, and `Fission` is implemented, so they belong in the same ledger.

| ability | record | carriers (conditional) | condition | class | status |
|---|---|---|---|---|---|
| `$06` (6) **Fission**<br>FISSION | eff `$1E` · stat $00 (none) · tgt 9 · pow 0 · res $00 (none) · el `0` none | 12 Igglanova | 1 EmptySpace | unknown | unsupported |
| `$0A` (10) **TwinClaw**<br>TWIN CLAW | eff `$01` · stat $05 (attack) · tgt 8 · pow 32 · res $06 (defense) · el `1` physical | 20 Silvalt, 21 Goldine | 2 HalfHPOrLower | damage | unsupported |
| `$0C` (12) **Combine**<br>COMBINE | eff `$1F` · stat $00 (none) · tgt 25 · pow 26 · res $00 (none) · el `0` none | 23 ArthroPod | 3 WiredineExists | unknown | unsupported |
| `$0D` (13) **Combine2**<br>COMBINE | eff `$1F` · stat $00 (none) · tgt 23 · pow 26 · res $00 (none) · el `0` none | 25 Wiredine | 4 ArthroPodExists | unknown | unsupported |
| `$12` (18) **Fusion**<br>FUSION | eff `$1F` · stat $00 (none) · tgt 34 · pow 36 · res $00 (none) · el `0` none | 34 ZolSlug | 6 ZolSlugs | unknown | unsupported |
| `$14` (20) **Warning**<br>WARNING | eff `$1E` · stat $00 (none) · tgt 44 · pow 0 · res $00 (none) · el `0` none | 39 Tower, 45 CommndBall | 1 EmptySpace | unknown | unsupported |
| `$15` (21) **ChargCnnon**<br>CHARGCNNON | eff `$20` · stat $01 (strength) · tgt 9 · pow 48 · res $07 (magic_defense) · el `2` energy | 40 CRayTube | 5 CRayTubeNearSatMinion | damage | unsupported |
| `$18` (24) **Explosion**<br>EXPLOSION | eff `$23` · stat $05 (attack) · tgt 8 · pow 0 · res $06 (defense) · el `1` physical | 44 FloatMine, 50 FloatMine2 | 7 PhysicalAtkReceived | damage | unsupported |
| `$1A` (26) **CyanicBomb**<br>CYANICBOMB | eff `$02` · stat $01 (strength) · tgt 8 · pow 56 · res $01 (strength) · el `10` biological | 46 VopalSphre | 7 PhysicalAtkReceived | status/stat effect | unsupported |
| `$1B` (27) **Fission3**<br>FISSION | eff `$25` · stat $00 (none) · tgt 35 · pow 4 · res $00 (none) · el `0` none | 38 FractOoze | 2 HalfHPOrLower | unknown | unsupported |
| `$1E` (30) **Spark**<br>SPARK | eff `$02` · stat $01 (strength) · tgt 8 · pow 80 · res $01 (strength) · el `12` mechanical | 49 Browren486 | 8 MagicDamageReceived | damage | unsupported |
| `$2C` (44) **Airslash**<br>AIRSLASH | eff `$01` · stat $05 (attack) · tgt 9 · pow 0 · res $06 (defense) · el `1` physical | 70 ShadowSabr, 71 FrostSaber, 72 BloodSaber | 2 HalfHPOrLower | damage | unsupported |
| `$3A` (58) **Combine3**<br>COMBINE | eff `$1F` · stat $00 (none) · tgt 86 · pow 87 · res $00 (none) · el `0` none | 84 BladeRight | 13 HakenLeftExists | unknown | unsupported |
| `$3B` (59) **Combine4**<br>COMBINE | eff `$1F` · stat $00 (none) · tgt 84 · pow 87 · res $00 (none) · el `0` none | 86 HakenLeft | 14 BladeRightExists | unknown | unsupported |
| `$41` (65) **Nothing2**<br>NOTHING | eff `$1F` · stat $00 (none) · tgt 0 · pow 80 · res $00 (none) · el `0` none | 94 InfantWorm | 9 Alone | unknown | unsupported |
| `$45` (69) **Res**<br>RES | eff `$12` · stat $82 (mental) · tgt 1 · pow 16 · res $00 (none) · el `0` none | 99 TechUser | 15 HalfHPOrLower_AllEnemies | status/stat effect | unsupported |
| `$46` (70) **Sar**<br>SAR | eff `$12` · stat $82 (mental) · tgt 2 · pow 16 · res $00 (none) · el `0` none | 100 TechMaster | 15 HalfHPOrLower_AllEnemies | status/stat effect | unsupported |
| `$49` (73) **Gisar**<br>GISAR | eff `$12` · stat $82 (mental) · tgt 2 · pow 16 · res $00 (none) · el `0` none | 101 DarkWitch, 116 Radhin | 15 HalfHPOrLower_AllEnemies | status/stat effect | unsupported |
| `$53` (83) **Nightmare**<br>NIGHTMARE | eff `$29` · stat $00 (none) · tgt 0 · pow 0 · res $00 (none) · el `0` none | 140 Zio2 | 19 Nothing | scripted/custom | unsupported |
| `$59` (89) **DisruptArm**<br>DISRUPTARM | eff `$01` · stat $01 (strength) · tgt 9 · pow 160 · res $06 (defense) · el `1` physical | 120 DeVars | 8 MagicDamageReceived | damage | unsupported |
| `$5C` (92) **ThndrBlast**<br>THNDRBLAST | eff `$01` · stat $02 (mental) · tgt 9 · pow 96 · res $07 (magic_defense) · el `2` energy | 123 XeAThoul | 18 ThreeXeAThouls | damage | unsupported |
| `$62` (98) **Reinforce**<br>REINFORCE | eff `$2B` · stat $00 (none) · tgt 3 · pow 0 · res $00 (none) · el `0` none | 128 Lashiec | 17 HP25PercentOrLower | status/stat effect | unsupported |
| `$67` (103) **Nothing3**<br>NOTHING | eff `$1F` · stat $00 (none) · tgt 0 · pow 134 · res $00 (none) · el `0` none | 133 ProfoundDarkness1 | 2 HalfHPOrLower | scripted/custom | unsupported |
| `$68` (104) **Nothing4**<br>NOTHING | eff `$1F` · stat $00 (none) · tgt 0 · pow 135 · res $00 (none) · el `0` none | 134 ProfoundDarkness2 | 2 HalfHPOrLower | unknown | unsupported |

## 4. Where the unsupported abilities appear

Map ids whose encounter table can roll a formation carrying the ability, with the
`maps.json` symbol:

| ability | class | maps | ids |
|---|---|---|---|
| `$03` RAIL-GUN | damage | 5 | 0BA PlateSystem, 0BB PlateSystem_F1, 0BC PlateSystem_F2, 0BD PlateSystem_F3, 0BE PlateSystem_F4 |
| `$04` LASRCANNON | damage | 14 | 001 Dezolis, 0BA PlateSystem, 0BB PlateSystem_F1, 0BC PlateSystem_F2, 0BD PlateSystem_F3, 0BE PlateSystem_F4, 0C0 ClimCenter, 0C1 ClimCenter_F1, 0C2 ClimCenter_F2, 0C3 ClimCenter_F3, 0C4 WeaponPlant, 0C5 WeaponPlant_F1, 0C6 WeaponPlant_F2, 0C7 WeaponPlant_F3 |
| `$05` LIGHTNING | damage | 4 | 0C4 WeaponPlant, 0C5 WeaponPlant_F1, 0C6 WeaponPlant_F2, 0C7 WeaponPlant_F3 |
| `$08` SPIRAL BLD | damage | 1 | 000 Motavia |
| `$09` MOTRCANNON | damage | 4 (+1 boss) | 0C4 WeaponPlant, 0C5 WeaponPlant_F1, 0C6 WeaponPlant_F2, 0C7 WeaponPlant_F3 |
| `$0B` STASISBALL | status/stat effect | 9 | 0C8 VahalFort, 0C9 VahalFort_F1, 0CA VahalFort_F2, 0CB VahalFort_F3, 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5 |
| `$0E` MICROMISSL | damage | 8 | 0C0 ClimCenter, 0C1 ClimCenter_F1, 0C2 ClimCenter_F2, 0C3 ClimCenter_F3, 0C8 VahalFort, 0C9 VahalFort_F1, 0CA VahalFort_F2, 0CB VahalFort_F3 |
| `$0F` FLAMLAUNCH | damage | 4 | 0C8 VahalFort, 0C9 VahalFort_F1, 0CA VahalFort_F2, 0CB VahalFort_F3 |
| `$13` CELL SPLIT | damage | 9 (+1 boss) | 001 Dezolis, 092 IslandCave, 093 IslandCave_F1, 094 IslandCave_F1_Part2, 095 IslandCave_Part2, 096 IslandCave_B1, 097 IslandCave_F2, 098 IslandCave_F3, 15F Hangar |
| `$16` FLASH | damage | 4 | 0C4 WeaponPlant, 0C5 WeaponPlant_F1, 0C6 WeaponPlant_F2, 0C7 WeaponPlant_F3 |
| `$19` DETONATION | damage | 8 | 190 Kuran, 191 Kuran_F1, 192 Kuran_F2, 193 Kuran_F1_Part2, 194 Kuran_F1_Part3, 195 Kuran_F1_Part5, 196 Kuran_F2_Part2, 197 Kuran_F1_Part4 |
| `$1C` FLARE SHOT | damage | 20 (+1 boss) | 0AE Wreckage, 0AF Wreckage_Part2, 0B0 Wreckage_Part3, 0B1 Wreckage_F1, 0B2 Wreckage_F1_Part2, 0B3 Wreckage_F2, 0B4 Wreckage_F2_Part2, 0B5 Wreckage_F2_Part3, 0C8 VahalFort, 0C9 VahalFort_F1, 0CA VahalFort_F2, 0CB VahalFort_F3, 190 Kuran, 191 Kuran_F1, 192 Kuran_F2, 193 Kuran_F1_Part2, 194 Kuran_F1_Part3, 195 Kuran_F1_Part5, 196 Kuran_F2_Part2, 197 Kuran_F1_Part4 |
| `$1D` BARRIER | status/stat effect | 4 | 0C8 VahalFort, 0C9 VahalFort_F1, 0CA VahalFort_F2, 0CB VahalFort_F3 |
| `$1F` DBL SLASH | damage | 9 (+1 boss) | 001 Dezolis, 0C2 ClimCenter_F2, 0C3 ClimCenter_F3, 0CA VahalFort_F2, 0CB VahalFort_F3, 158 MystVale, 159 MystVale_Part2, 15A MystVale_Part3, 15B MystVale_Part4 |
| `$20` PHONONMASR | damage | 2 (+2 boss) | 0CA VahalFort_F2, 0CB VahalFort_F3 |
| `$21` FIREBREATH | damage | 54 (+3 boss) | 082 ZioFort, 083 ZioFort_Part2, 084 ZioFort_F1, 085 ZioFort_F2West, 087 ZioFortJuzaRoom, 089 ZioFort_F2East, 08A ZioFort_F3, 08B ZioFort_F4, 08C LadeaTower, 08D LadeaTower_F1, 08E LadeaTower_F2, 08F LadeaTower_F3, 090 LadeaTower_F4, 091 LadeaTower_F5, 092 IslandCave, 093 IslandCave_F1, 094 IslandCave_F1_Part2, 095 IslandCave_Part2, 096 IslandCave_B1, 097 IslandCave_F2, 098 IslandCave_F3, 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F7 CourageTower, 0F8 CourageTower_F1, 0FC AngerTower, 0FD AngerTower_F1, 172 AirCastle_Part2, 173 AirCastle_Part3, 174 AirCastle_Part4, 175 AirCastle_Part5, 176 AirCastle_F1_Part9, 177 AirCastle_F1_Part5, 178 AirCastle_F1_Part2, 179 AirCastle_F1_Part10, 17A AirCastleInner, 17B AirCastle_F1_Part11, 17C AirCastle_F1_Part12, 17D AirCastle_F1_Part13, 17E AirCastle_Part8, 17F AirCastle_Part7, 180 AirCastle_F1_Part4, 181 AirCastle_F1, 182 AirCastle_F1_Part3, 183 AirCastle_F2, 184 AirCastleXeAThoulRoom, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2 |
| `$22` RAY BREATH | damage | 17 (+1 boss) | 001 Dezolis, 002 Rykros, 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5 |
| `$23` SUPERSONIC | damage | 5 | 001 Dezolis, 158 MystVale, 159 MystVale_Part2, 15A MystVale_Part3, 15B MystVale_Part4 |
| `$24` POISONMIST | status/stat effect | 8 | 001 Dezolis, 092 IslandCave, 093 IslandCave_F1, 094 IslandCave_F1_Part2, 095 IslandCave_Part2, 096 IslandCave_B1, 097 IslandCave_F2, 098 IslandCave_F3 |
| `$25` SLEEP GAS | status/stat effect | 1 | 15F Hangar |
| `$26` SHIFT | status/stat effect | 5 | 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$27` SANER | status/stat effect | 5 | 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$28` DORAN | status/stat effect | 5 | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5 |
| `$29` SEALS | status/stat effect | 10 | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$2A` RIMIT | damage | 9 | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5, 158 MystVale, 159 MystVale_Part2, 15A MystVale_Part3, 15B MystVale_Part4 |
| `$2B` NEEDLE | damage | 5 | 001 Dezolis, 158 MystVale, 159 MystVale_Part2, 15A MystVale_Part3, 15B MystVale_Part4 |
| `$2D` DEBAN | status/stat effect | 5 | 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$2F` VOL | damage | 19 | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5, 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F7 CourageTower, 0F8 CourageTower_F1, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$30` DISTORTION | damage | 0 | — |
| `$31` GRA | damage | 30 | 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 172 AirCastle_Part2, 173 AirCastle_Part3, 174 AirCastle_Part4, 175 AirCastle_Part5, 176 AirCastle_F1_Part9, 177 AirCastle_F1_Part5, 178 AirCastle_F1_Part2, 179 AirCastle_F1_Part10, 17A AirCastleInner, 17B AirCastle_F1_Part11, 17C AirCastle_F1_Part12, 17D AirCastle_F1_Part13, 17E AirCastle_Part8, 17F AirCastle_Part7, 180 AirCastle_F1_Part4, 181 AirCastle_F1, 182 AirCastle_F1_Part3, 183 AirCastle_F2, 184 AirCastleXeAThoulRoom, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5 |
| `$32` GIGRA | damage | 5 | 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5 |
| `$34` VOICE | status/stat effect | 4 | 08C LadeaTower, 08D LadeaTower_F1, 08E LadeaTower_F2, 08F LadeaTower_F3 |
| `$35` GIZAN | damage | 25 (+2 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 158 MystVale, 159 MystVale_Part2, 15A MystVale_Part3, 15B MystVale_Part4, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$36` STRNGLIGHT | status/stat effect | 7 | 092 IslandCave, 093 IslandCave_F1, 094 IslandCave_F1_Part2, 095 IslandCave_Part2, 096 IslandCave_B1, 097 IslandCave_F2, 098 IslandCave_F3 |
| `$38` EARTHQUAKE | damage | 1 (+1 boss) | 000 Motavia |
| `$3C` BLADESHINE | damage | 18 | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$3D` HAKEN BOLT | damage | 18 | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$3E` GIRES | status/stat effect | 7 | 0CC Nurvus_Part2, 0CD Nurvus_Part3, 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5 |
| `$42` RAY-SPEAR | damage | 11 | 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$43` THROWLANCR | damage | 6 | 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$47` ZAN | damage | 7 (+1 boss) | 0CC Nurvus_Part2, 0CD Nurvus_Part3, 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5 |
| `$48` GIFOI | damage | 10 | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1 |
| `$4A` STAR DUST | damage | 0 | — |
| `$4B` SHADOWBIND | status/stat effect | 10 (+2 boss) | 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$4C` EVIL EYE | damage | 29 (+2 boss) | 08E LadeaTower_F2, 08F LadeaTower_F3, 090 LadeaTower_F4, 091 LadeaTower_F5, 172 AirCastle_Part2, 173 AirCastle_Part3, 174 AirCastle_Part4, 175 AirCastle_Part5, 176 AirCastle_F1_Part9, 177 AirCastle_F1_Part5, 178 AirCastle_F1_Part2, 179 AirCastle_F1_Part10, 17A AirCastleInner, 17B AirCastle_F1_Part11, 17C AirCastle_F1_Part12, 17D AirCastle_F1_Part13, 17E AirCastle_Part8, 17F AirCastle_Part7, 180 AirCastle_F1_Part4, 181 AirCastle_F1, 182 AirCastle_F1_Part3, 183 AirCastle_F2, 184 AirCastleXeAThoulRoom, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5 |
| `$4D` CORRSION | damage | 51 (+2 boss) | 08E LadeaTower_F2, 08F LadeaTower_F3, 090 LadeaTower_F4, 091 LadeaTower_F5, 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 172 AirCastle_Part2, 173 AirCastle_Part3, 174 AirCastle_Part4, 175 AirCastle_Part5, 176 AirCastle_F1_Part9, 177 AirCastle_F1_Part5, 178 AirCastle_F1_Part2, 179 AirCastle_F1_Part10, 17A AirCastleInner, 17B AirCastle_F1_Part11, 17C AirCastle_F1_Part12, 17D AirCastle_F1_Part13, 17E AirCastle_Part8, 17F AirCastle_Part7, 180 AirCastle_F1_Part4, 181 AirCastle_F1, 182 AirCastle_F1_Part3, 183 AirCastle_F2, 184 AirCastleXeAThoulRoom, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$4E` DTHSPELL | status/stat effect | 43 (+1 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 172 AirCastle_Part2, 173 AirCastle_Part3, 174 AirCastle_Part4, 175 AirCastle_Part5, 176 AirCastle_F1_Part9, 177 AirCastle_F1_Part5, 178 AirCastle_F1_Part2, 179 AirCastle_F1_Part10, 17A AirCastleInner, 17B AirCastle_F1_Part11, 17C AirCastle_F1_Part12, 17D AirCastle_F1_Part13, 17E AirCastle_Part8, 17F AirCastle_Part7, 180 AirCastle_F1_Part4, 181 AirCastle_F1, 182 AirCastle_F1_Part3, 183 AirCastle_F2, 184 AirCastleXeAThoulRoom, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5 |
| `$4F` HEWN | damage | 30 (+2 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 15D ElsydeonCave, 15E ElsydeonCave_B1, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$50` BAD SMELL | status/stat effect | 4 | 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$51` MINDBLST | damage | 18 (+1 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$52` TANDLE | damage | 28 (+1 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$54` BLACK WAVE | scripted/custom | 0 (+1 boss) | — |
| `$55` LEGEON | damage | 8 | 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$56` FORCEFLASH | damage | 10 (+1 boss) | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4, 19D GaruberkTower_Part5, 19E GaruberkTower_Part6, 19F GaruberkTower_Part7 |
| `$57` GELUN | status/stat effect | 5 | 0CE Nurvus_B1, 0CF Nurvus_B2, 0D0 Nurvus_B3, 0D2 Nurvus_B4, 0D5 Nurvus_B5 |
| `$58` LGHTBREATH | damage | 1 | 002 Rykros |
| `$5A` FLAELI | damage | 28 (+2 boss) | 0F2 StrengthTower, 0F3 StrengthTower_F1, 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F7 CourageTower, 0F8 CourageTower_F1, 0F9 CourageTower_F2, 0FA CourageTower_F3, 0FC AngerTower, 0FD AngerTower_F1, 100 TheEdge, 101 TheEdge_Part2, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8, 185 AirCastleInner_B1, 186 AirCastleInner_B1_Part2, 188 AirCastleInner_B2, 189 AirCastleInner_B3, 18A AirCastleInner_B4, 18B AirCastleInner_B5, 199 GaruberkTower, 19A GaruberkTower_Part2, 19B GaruberkTower_Part3, 19C GaruberkTower_Part4 |
| `$5D` TANDIL | damage | 10 | 0F4 StrengthTower_F2, 0F5 StrengthTower_F3, 0F9 CourageTower_F2, 0FA CourageTower_F3, 102 TheEdge_Part3, 103 TheEdge_Part4, 104 TheEdge_Part5, 105 TheEdge_Part6, 106 TheEdge_Part7, 107 TheEdge_Part8 |
| `$5E` MEGID | damage | 0 (+1 boss) | — |
| `$5F` THNDHALBRT | damage | 0 (+1 boss) | — |
| `$60` POSESSION | damage | 0 (+1 boss) | — |
| `$61` ANOTHRGATE | damage | 0 (+1 boss) | — |
| `$63` BURSTROC | scripted/custom | 0 (+1 boss) | — |
| `$64` SHDWBREATH | scripted/custom | 0 (+2 boss) | — |
| `$65` LIGHTSHOWR | damage | 0 (+1 boss) | — |
| `$69` CANCELING | damage | 0 | — |
| `$6A` WIND STORM | damage | 1 | 001 Dezolis |
| `$6C` BLACK WAVE | scripted/custom | 0 | — |

## 5. Findings

### Counts

| | |
|---|---|
| distinct nonzero regular ability ids (§2) | 83 |
| implemented | 15 |
| unsupported | 68 |
| — `damage` | 48 |
| — `status/stat effect` | 16 |
| — `scripted/custom` | 4 (`$54` BLACK WAVE, `$63` BURSTROC, `$64` SHDWBREATH, `$6C` BLACK WAVE) |
| — `unknown` | 0 |
| conditional-only ids (§3) | 24, one of them implemented (`$06` FISSION) |

The fifteen implemented rows are exactly what `psiv-core` claims:

- `$02` FLAME BOLT → `enemy_damage::resolve_damage_skill` for both carriers
  (`DAMAGE_SKILL_ROUTES`, `rust/psiv-core/src/battle/enemy_damage.rs`). 0 Helex
  (`EnemyAttackOffs` `$00`) → `EnemyAttack_Helex` (`ps4.asm:23574`) → object
  `$48` = `BattleObj_HelexFlameBolt` (`ps4.asm:30279`); 5 ForcedFly (`$05`) →
  `EnemyAttack_ForcedFly` (`ps4.asm:23567`), which falls through into that same
  body for a nonzero ability. The parent object only animates and hands
  `$38`/`$3C` to `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`, lines
  30301-30302), whose single `move.w #$C, $2(a3)` (`ps4.asm:30342`) is guarded
  by bit 1 and handshaken through `($FFFF416C)`. No arm clears
  `Current_Target_Index`, and `Enemy_Attack` had already stored the drawn target
  in the object, so the one request lands on the chosen party member.
- `$07` Fission2 → `enemy_skill::resolve_fission` (`rust/psiv-core/src/battle/enemy_skill.rs`),
  reached from `roll_enemy_ability` through `fission_neighbor`; the same predicate
  (`is_fission`) covers `$06` Fission, which appears only in §3. That implemented
  path is the conditional one (enemies 12 Igglanova and 13 Guilgenova), dispatched
  by `EnemyAttack_Igglanova` (`ps4.asm:23505`), whose `tst.b ability+1(a4)` picks
  `BattleObj_IgglanovaFission` (`ps4.asm:29571`) + `BattleObj_IgglanovaFission2`
  (`ps4.asm:29796`) — no damage request, as the resolver assumes. §2's row for
  `$07` is the *other* carrier, enemy 50 FloatMine2, whose own routine has no
  `$07` arm at all — `enemy_skill::resolve_no_effect_turn` (below).
- `$17` Waiting → the same `enemy_skill::resolve_no_effect_turn`, on the three
  carriers whose `EnemyAttackOffs` entry is `EnemyAttack_FloatMine`
  (`ps4.asm:22675`): 44 FloatMine (`$2C`), 46 VopalSphre (`$2E`) and 50
  FloatMine2 (`$32`). `loc_10406` (`ps4.asm:22781`) loads no object and clears
  `$24(a4)`, so those turns pass with nothing happening (see the Port gaps
  bullet for the draws and the one unmodelled override).
- `$10` THREAD → `enemy_skill::resolve_thread`. Carrier 31 CarrionCr →
  `EnemyAttack_Crawler` (`ps4.asm:23081`) → `BattleObj_Thread` (`ps4.asm:36186`)
  for `$10`, `BattleObj_Poison` (`ps4.asm:36279`) otherwise — matching
  `docs/THREAD.md`.
- `$11` POISON → `enemy_skill::resolve_poison`. Carrier 32 Caterpillr →
  `EnemyAttack_Crawler` → `BattleObj_Poison`; effect `$1B` →
  `AbilityEffect_Poison`. See [`ENEMY_POISON.md`](ENEMY_POISON.md).
- `$33` ACIDBREATH → `enemy_damage::resolve_damage_skill` for all four carriers
  (`DAMAGE_SKILL_ROUTES`, `rust/psiv-core/src/battle/enemy_damage.rs`). 75
  FlattrPlnt and 76 FlyScreamr (`EnemyAttackOffs` `$4B`/`$4C`) →
  `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`) → `BattleObj_AcidBreath`
  (`ps4.asm:38566`) + `BattleObj_AcidBreathChild` (`ps4.asm:38622`); 85 Piercer
  and 86 HakenLeft (`$55`/`$56`) → `EnemyAttack_Piercer` (`ps4.asm:21518`) →
  object `$35C` = `loc_23998` (`ps4.asm:47264`) + `$360` = `loc_24FD2`
  (`ps4.asm:48883`). Both arms keep `Current_Target_Index`, make one reaction
  write (`move.w #5, $2(a3)`) and one damage request (`move.w #$C, $2(a3)`), and
  write MoleAttack `$D5` then EnemyAttack4 `$D8`. The same resolver's `$02`
  entries cover FLAME BOLT for 0 Helex and 5 ForcedFly.
- `$2E` GIWAT, `$37` SAND STORM, `$39` MAELSTROM, `$3F` FLODBREATH, `$40`
  WAT, `$44` FOI, `$6D` ROUND EYES and `$6E` LOVEL EYES → the same
  `enemy_damage::resolve_damage_skill`. Every carrier of each of the eight is
  covered, so none of those rows is partial: `$2E` for 71/77/91/101/122/123,
  `$3F` for 90/91/92, `$40` for 91/92/99/100/114, `$44` for 99/100/114, and one
  carrier each for `$37` (81), `$39` (82), `$6D` (147) and `$6E` (148). Each
  `(enemy, ability)` entry cites its `EnemyAttackOffs` line, its arm, its object
  chain and the single `move.w #$C` request; the per-pair readings are in
  [ENEMY_DAMAGE_ROUTES.md](ENEMY_DAMAGE_ROUTES.md) §3. The gate no longer tests
  record byte 2, which is why `$37` and `$39` — both tgt 9, the all-party
  *nibble* — are in the table: their chains make one request against the
  object's `$38`.
- `$70`/112 BLACK WAVE (Zio3) → the scripted sequence in
  `engine::roll_enemy_ability`, fed by `EnemyAttack_Zio3` (`ps4.asm:19410`):
  phase 0 writes `$6B` and loads `BattleObj_MagBarrir` (`ps4.asm:67319`), phase 1
  clears the ability and loads `BattleObj_NightmarePart1` (`ps4.asm:67253`),
  phase 2 ends the turn with no object, phase 3 writes `$53` and loads
  `BattleObj_NightmarePart2` (`ps4.asm:67107`), phase 4 writes `$70` and loads
  `BattleObj_BlackWave1` (`ps4.asm:67019`), which ends the turn without calling
  `GetEnemySkillEffectAndRange` — so the out-of-range effect `$2C` never reaches
  the dispatcher.

### Cartridge behaviour to know before implementing one

- **The child object holds the hit.** `BattleObj_HelexFlameBolt` (`ps4.asm:30279`)
  only animates; its child `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`) is where
  `move.w #5, $2(a3)` lives. Same for `BattleObj_MonsterflyAtk` (`ps4.asm:30205`)
  → `BattleObj_MonsterflyAtk2` (`ps4.asm:30241`). Reading only the object the
  routine loads would wrongly call FlameBolt a non-damaging ability; `$02` is
  implemented from that child (see its §2 row).
- **Six shared tails apply damage to the whole side or to one target**:
  `loc_24A6C` (`ps4.asm:48468`), `loc_24A9E` (`ps4.asm:48483`), `loc_24AEC`
  (`ps4.asm:48507`), `loc_24B20` (`ps4.asm:48523`), `loc_24B64` (`ps4.asm:48541`)
  and `loc_24BB6` (`ps4.asm:48562`), the last write landing in `loc_24BDC`
  (`ps4.asm:48574`). Objects jump into them with `jmp`/`bra.w`, so the damage
  request is often not in the object but in its exit path.
- **Fall-through and cross-routine branches**: `EnemyAttack_ForcedFly`
  (`ps4.asm:23567`) falls through into `EnemyAttack_Helex`'s body for a nonzero
  ability (both use FlameBolt's object), `EnemyAttack_WorkerPod` (`ps4.asm:23182`)
  branches into `EnemyAttack_Tarantella` (`ps4.asm:23204`), and
  `EnemyAttack_Sweeper` (`ps4.asm:23676`) jumps into `EnemyAttack_Seeker`
  (`ps4.asm:23663`) for `$04`. The `; ----` banners between labels are cosmetic.
- **Five routines clear `$24(a4)` and then run an ability-0 object**, so the object
  sees ability 0: `EnemyAttack_MiniWorm` (`loc_F590`, `ps4.asm:21768` → object
  `$398`, `loc_223C0`), `EnemyAttack_Ripper` (`loc_F3B6`, `ps4.asm:21620` →
  `$354`), `EnemyAttack_Piercer` (`loc_F2E4`, `ps4.asm:21549` → `$354`),
  `EnemyAttack_ArmDrone`'s `$17` branch (`loc_10468`, `ps4.asm:22806`, no object
  at all) and `EnemyAttack_FloatMine`'s fall-through (`loc_10406`,
  `ps4.asm:22781`, no object). That last one is the `$07`/`$17` "nothing happens"
  turn the port now implements (§2); the other four still need their objects.
- **The Zio family rewrites the ability it rolled** (`move.w #$ID, $24(a4)`) rather
  than dispatching it: `EnemyAttack_Zio3` (`ps4.asm:19410`), `EnemyAttack_Zio`
  (`ps4.asm:19483`) and `EnemyAttack_Zio2` (`ps4.asm:19519`) drive objects from a
  phase counter (`$FFFFEE98`), and the phase-4 Zio3 object is the only user of
  ability 112. Zio3's rolled id is therefore never the executed ability.
- **Eight unsupported rows have a cutscene-gated carrier routine** (marked `†`):
  ProfoundDarkness1 for `$21`/`$22`, DarkForce1 for `$1C`/`$20`, DarkForce2 for
  `$4C`/`$65`, Zio2 for `$4D`/`$4F`. Those routines test a scripted-battle flag
  (`$FFFFEE87`) first and use one fixed object, clearing the ability; the enemy
  records for those carriers only describe the non-scripted fight.
- **Conditional ids share objects with regular ids.** 24 nonzero ids live only in
  `conditional_ability_ids` (§3). Seven of them are compared by a routine branch
  that no regular roll can reach — `$14`, `$18`, `$1A`, `$2C`, `$45`, `$49`, `$62`
  — and the other 17 land in their routine's final `else`, sharing an object with a
  regular id. An implementation of the AI block (`EnemyAIInstructionsOffs`,
  `ps4.asm:19364`) will start firing objects that no regular roll reaches.
- **Twelve unsupported abilities have no map access at all** — they can only be met
  in boss battles, or nowhere: `$30` DISTORTION, `$4A` STAR DUST, `$54` BLACK WAVE,
  `$5E` MEGID, `$5F` THNDHALBRT, `$60` POSESSION, `$61` ANOTHRGATE, `$63` BURSTROC,
  `$64` SHDWBREATH, `$65` LIGHTSHOWR, `$69` CANCELING and `$6C` BLACK WAVE. `$54`,
  `$5E`, `$5F`, `$60`, `$61`, `$63`, `$64` and `$65` do appear in boss formations;
  `$30`, `$4A`, `$69` and `$6C` appear in none, because their owners are placed by
  a scene writing `Enemy_Positions` directly (`EnemyID_ProfoundDarkness3` at line
  61120, `EnemyID_ProfoundDarkness2` at line 61917, `EnemyID_Zio2` at line 79211)
  — or, for `$4A`'s owner ShadMirage (enemy 105), in no US encounter at all.

### Data and disassembly contradictions

- **Enemy skill byte 1 bit 7 (`$82`).** 18 of the 83 rows carry `$82` in
  `relevant_stat` (`$26`, `$27`, `$28`, `$29`, `$2A`, `$2D`, `$2E`, `$2F`, `$31`,
  `$32`, `$35`, `$3E`, `$40`, `$44`, `$47`, `$48`, `$57`, `$5E`). The cartridge
  masks the bit before using the selector — `andi.w #$7F, d1` in `loc_26DA`
  (`Enemy_DamageCharacter`, `ps4.asm:3775`; the store is at line 3798) and in
  `Effect_SetupSkillParams` (`ps4.asm:9576`; line 9579) — so the power stat is
  selector 2, mental battle. The extracted record keeps the raw `$82`
  (`generated/enemy_skills.json` and the pack's `abilities.json`), and
  `technique::stat` (`rust/psiv-core/src/battle/technique.rs`) maps 130 to 0. Any
  implementation of those abilities must mask first, or it will compute from a zero
  stat.
- **Effect `$2C` is out of range** (already in `SOURCE_NOTES.md`): Black Wave 112 is
  the only carrier, and Zio3's phase-4 object never dispatches it. The main table
  records the handler accordingly.
- **Five of the 112 skill records are referenced by no enemy AI block**: `$01`
  NOTHING, `$5B` BINDWA, `$66` DESTROCRAY, `$6B` MAGBARRIR, `$6F` CASTING. Two of
  them are still written by code — `EnemyAttack_ProfoundDarkness1` compares `$66`
  and the Zio routines write `$6B` — so "unreferenced" is not "unreachable".
- **One id is dispatched by a routine but listed by no enemy**: `$66` DESTROCRAY,
  which `EnemyAttack_ProfoundDarkness1` still compares (`loc_D93E`,
  `ps4.asm:19883`). Every other branch value in §2 belongs to a regular or
  conditional ability list of at least one enemy — the branches that look dead
  (`$49` Gisar, `$45` Res, `$14` Waitng, `$26`-`$29`, `$2C`/`$2D`, …) are exactly
  the ones the conditional path reaches.
- **Enemy 30 Crawler carries an all-zero list**, yet the THREAD family's routine
  `EnemyAttack_Crawler` is shared by enemies 30-32: ability `$10` belongs to 31
  CarrionCr and `$11` to 32 Caterpillr. `docs/THREAD.md`'s "crawler family" is about
  the shared routine, not about all three enemies rolling THREAD.
- **The formation group masks are not a spawn filter.** Bytes 5-6 of a formation
  (`group_1_mask`/`group_2_mask`) are read only by `loc_7B94` (`ps4.asm:11590`) to
  choose the group leader's sprite; the spawner `loc_7F2E` (`ps4.asm:11911`) walks
  the `(enemy id, position)` pairs to the `$FF` terminator. Counting formations by
  their enemy list — as §2 and §4 do — is therefore the right count.
- **The fork build gates `Battle_FormationIndexes` (`ps4.asm:320064`) itself** (the
  `if (revision=0)||(restored_enemy=1)` switch at line 320066). This ledger uses the
  ROM-extracted tables (`generated/formation_indexes.json`), not the fork's chosen
  arm.
- `generated/enemies.json` and `generated/enemy_skills.json` agree with the
  disassembly's own id labels for every id used here; no id in any enemy record
  exceeds the 112-record skill table, and no record places a nonzero conditional
  ability after a zero condition id (which would make the entry unreachable).

### Port gaps

- `engine::roll_enemy_ability` (`rust/psiv-core/src/battle/engine.rs`) handles
  Fission/Fission2, the record-driven damage skills (`enemy_damage`: `$33`,
  `$02` and the eight Motavia single-target abilities), THREAD, Zio3 and the
  FloatMine carriers' `$07`/`$17` no-effect turn. Every other nonzero id emits
  `BattleEvent::UnsupportedAbility` and then takes the ordinary attack path
  (the fallback at the end of `roll_enemy_ability` in `engine.rs`), so those enemies still *hit* — they hit with a physical
  swing instead of their ability. Any bug report about enemy damage in a fight
  listed in §4 is this fallback.
- **Acid Breath covers every carrier.** The `$33` `DAMAGE_SKILL_ROUTES` entries
  (`enemy_damage.rs`) hold the four enemies whose `EnemyAttackOffs` entry is one
  of the two traced `$33` routines: 75 FlattrPlnt and 76 FlyScreamr share
  `EnemyAttack_FlattrPlnt`
  (`ps4.asm:21778`), 85 Piercer and 86 HakenLeft share `EnemyAttack_Piercer`
  (`ps4.asm:21518`), whose `$33` arm loads object `$35C` = `loc_23998`
  (`ps4.asm:47264`) + `$360` = `loc_24FD2` (`ps4.asm:48883`). Both arms keep
  `Current_Target_Index`, make one reaction write and one `move.w #$C, $2(a3)`
  damage request, and write `$D5` then `$D8`, so both resolve through the same
  formula. 77 TechPlant shares `EnemyAttack_FlattrPlnt` but rolls only `$2A` and
  `$2E`, so it is deliberately outside the gate; an enemy outside it still falls
  back (the core tests `an_unproven_carrier_still_reports_acid_breath_as_unsupported`
  and `an_unproven_carrier_still_reports_flame_bolt_as_unsupported`). FLAME BOLT
  is the same shape: 0 Helex and 5 ForcedFly are the only enemies whose regular
  list holds `$02`, and both `EnemyAttackOffs` entries (`$00`, `$05`) run the
  traced object chain. The gate is the `(enemy, ability)` pair plus the
  record's effect byte: `$01` (`AbilityEffect_None`, `ps4.asm:9092`) means the
  one damage request is the record's whole effect, and a record with any other
  effect stays on the `UnsupportedAbility` path. Record byte 2 is deliberately
  *not* a gate — it picks the `Ability_ProcessRange` (`ps4.asm:8975`) handler for
  that effect — and the record's stat byte is masked with `$7F` as
  `Effect_SetupSkillParams` (`ps4.asm:9576`) masks it at line 9580.
- **Fission2's regular roll is a spent turn, not a physical attack.** The port
  reaches `resolve_fission` only through `fission_neighbor`, which returns `None`
  unless the record is enemy 12 or 13, and `resolve_fission` runs only when that
  returns a target. Enemy 50 FloatMine2 rolls `$07` as a regular ability, and the
  cartridge's `EnemyAttack_FloatMine` fall-through (`loc_10406`, `ps4.asm:22781`)
  clears `$24(a4)` and loads no object, so the turn passes with nothing happening
  — no damage, no sound, no message, no status — and the actor still counts as
  having acted. `enemy_skill::resolve_no_effect_turn` now emits
  `BattleEvent::EnemyAbilityWasted` for that and for `$17` Waiting instead of
  `UnsupportedAbility` + a swing. Two limits are deliberate: the port emits one
  event for the turn and models no per-frame timing (retail's `loc_6672` sets the
  `$FFFF418A` wait to `$F` because the ability slot is empty, so the turn advances
  sixteen frames later than an attack's — a pacing detail of the unimplemented
  object-timing layer), and `EnemyAI_PhysicalAtkReceived` (instruction 7, the
  only `condition_ids` entry these carriers have) is not modelled, so a FloatMine2
  that was hit physically since its last action would in retail have its `$07`
  replaced by the `$18` Explosion conditional; the port spends the turn instead of
  firing an untraced object. The draw accounting is exact: 9 ordering rolls, 4
  enemy-target rolls and the ability index, then nothing — `loc_B6A2` runs with
  `Current_Target_Index` 0, whose phantom slot (`$FFFF43C0`, wiped by the battle
  start's `trap #0` at `ps4.asm:9989`) is empty, so no chance roll is taken and
  all nine `Fighters_Hit_Flags` stay `$FF`. `BattleEvent::EnemyAbilityWasted` is
  narrated as an empty line and no beat (see the `$17` row); `BattleEvent` is no
  longer `non_exhaustive`, so the renderer's exhaustive narration match fails to
  compile when a new event appears instead of showing an "unhandled event" line
  at runtime.
- **The AI condition block is unimplemented.** Only `fission_neighbor` (condition 1
  for enemies 12/13) reads `condition_ids` / `conditional_abilities`; the other 19
  `EnemyAIInstructionsOffs` entries are not modelled, so all 24 §3 ids and every
  conditional override are unreachable in the port regardless of what §2
  implements.
- The two `scripted/custom` bosses that are *not* implemented (`$54` Zio, `$6C`
  Zio2) share Zio3's structure — a phase counter driving `move.w #$ID, $24(a4)` —
  so the `FirstZioAction` shape should carry over.

## Appendix — how to re-check this file

- **Completeness.** §2 holds one row per id in the union of
  `ai.regular_ability_ids` over the 153 records of `generated/enemies.json`
  (83 ids, each exactly once); the same set re-derived from `raw_hex` bytes 36..43
  is identical. §4 holds the §2 rows whose status is `unsupported` (68 rows).
- **Citations.** Every `ps4.asm:NNNN` citation in this file points at a label that
  begins on that line, and every backticked label next to such a citation is the
  label found there. Statement-level references are written as "line NNNN" and
  deliberately carry no `ps4.asm:` prefix.
- **Classes.** The verdicts come from the object-chain rule in §1. Three of them can
  be cross-read against existing documents and agree: THREAD (`no damage`,
  `docs/THREAD.md`), Acid Breath (`damage`, `SOURCE_NOTES.md`) and Fission
  (`no damage`, the resolver's comments).
- Nothing here was produced by a build run: it is static reading of the disassembly
  plus the ROM-extracted tables. No emulator or hardware capture backs a class
  verdict, and the objects' own timings remain a separate unimplemented layer
  (`docs/BATTLE_ANIMATIONS.md`).
