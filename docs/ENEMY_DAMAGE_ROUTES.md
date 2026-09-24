# Enemy damage-ability routes

Which retail route each (enemy, ability) pair takes when the ability deals
damage: the `EnemyAttack_*` arm, the object chain it spawns, how many
`Fighter_TakeDamage` requests the chain makes and where, and the target those
requests land on. [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md) answers *whether* an
ability damages; this file answers *how*, so a generic single-target resolver can
be extended pair by pair without repeating this reading. Nothing here changes
code.

**146 rows**: one per (enemy id, ability id) pair for the 57 abilities that
`ENEMY_ABILITIES.md` §2 classes `damage` (142 pairs), plus the four ACID BREATH
pairs — ability `$33` is implemented, so §2 carries no class for it, but it is the
class this file is written for. Ability ids are decimal in `enemies.json`, hex in
the disassembly; both are shown.

## 1. Damage records: what the bytes mean

An enemy skill is an eight-byte record at `EnemySkillData` (`ps4.asm:319311`); id
`n` lives at `EnemySkillData + (n-1)*8`, which is why every reader does
`lea (EnemySkillData-8).l` and indexes with `id*8` (`lea (EnemySkillData-8).l` at lines 8690, 9571 and 3794).

| byte | field | read at |
|---|---|---|
| 0 | effect id → `AbilityEffectsOffs` (`ps4.asm:9036`) | `GetEnemySkillEffectAndRange` (`ps4.asm:8687`, reads it at line 8692) |
| 1 | power stat selector; **bit 7 is a flag, masked off before use** | `Effect_SetupSkillParams` (`ps4.asm:9576`; read at line 9579, masked at line 9580), `Enemy_DamageCharacter` (`ps4.asm:3775`; read at line 3797, masked at line 3798) |
| 2 | target/range byte; the **low nibble** picks the range | `GetEnemySkillEffectAndRange` (`ps4.asm:8687`; stored at line 8691, masked with `andi.b #$F` at line 8694) |
| 3 | hit chance (the "power" of a damage record) | `Effect_SetupSkillParams`, line 9602 |
| 4 | resistance stat selector | `Effect_SetupSkillParams`, line 9604 |
| 5 | element | `Effect_SetupSkillParams`, line 9597 |
| 6-7 | always zero | — |

### Byte 2: the target nibble

`Ability_GetEffectAndRange` (`ps4.asm:8886`) loads the nibble (line 8895) and
dispatches through `AbilityRangeOffs` (`ps4.asm:8903`), whose handlers are
`AbilityRange_Varied` (`ps4.asm:8920`), `AbilityRange_Single` (`ps4.asm:8938`),
`AbilityRange_Self` (`ps4.asm:8946`), `AbilityRange_MultiEnemies`
(`ps4.asm:8954`) and `AbilityRange_MultiChars` (`ps4.asm:8962`):

| nibble | handler | targets | records (of the 112) |
|---|---|---|---|
| 0 | Varied | the chosen target when `Current_Target_Index` ≥ 0; otherwise slots 6-10 (all characters) if the actor is an enemy, 1-5 (all enemies) if it is a party member | `$17`, `$41`, `$53`, `$67`, `$68`, `$6B`, `$6F`, `$70` |
| 1 | Single | the chosen target | `$3E`, `$45` |
| 2 | MultiEnemies | slots 6-9 (all enemies) | `$12`, `$1D`, `$27`, `$2D`, `$46`, `$49` |
| 3 | Self | the actor | `$01`, `$1B`, `$26`, `$62` |
| 4 | Single | the chosen target, **humans only** (filter at lines 8993-9000) | `$3B` |
| 5 | MultiChars | slots 1-5, humans only | — |
| 6 | Single | the chosen target, androids only | `$3A` |
| 7 | MultiChars | slots 1-5, androids only | `$0D` |
| 8 | Single | the chosen target, any profession — **the single-target damage nibble** | `$02`, `$03`, `$04`, `$05`, `$0A`, `$0B`, `$0F`, `$10`, `$11`, `$18`, `$1A`, `$1C`, `$1E`, `$1F`, `$21`, `$22`, `$24`, `$2B`, `$2E`, `$2F`, `$33`, `$36`, `$3D`, `$3F`, `$40`, `$42`, `$44`, `$48`, `$4C`, `$4E`, `$54`, `$58`, `$5A`, `$60`, `$64`, `$6C`, `$6D`, `$6E` |
| 9 | MultiChars | slots 1-5 — **the all-party damage nibble** | `$06`, `$08`, `$09`, `$0C`, `$0E`, `$13`, `$15`, `$16`, `$19`, `$20`, `$23`, `$25`, `$28`, `$29`, `$2A`, `$2C`, `$30`, `$31`, `$32`, `$34`, `$35`, `$37`, `$38`, `$39`, `$3C`, `$43`, `$47`, `$4A`, `$4B`, `$4D`, `$4F`, `$50`, `$51`, `$52`, `$55`, `$56`, `$57`, `$59`, `$5B`, `$5C`, `$5D`, `$5E`, `$5F`, `$61`, `$63`, `$65`, `$66`, `$69`, `$6A` |
| 10, 12 | — | **past the ten-entry table**: `AbilityRangeOffs` has entries for 0-9 only, `trap #2` does not bound-check, so the dispatch reads the word after the table and jumps nowhere useful | `$07`, `$14` |

`Ability_ProcessRange` (`ps4.asm:8975`) skips empty fighter slots (lines 8978-8979)
and dead fighters (lines 8989-8991), applies the human/android filter and calls the
effect handler once per target. Only nibbles 4-7 filter by profession.

All 58 damage abilities in this file use nibble 8 (25 of them) or nibble 9
(33); no damage record uses nibbles 0-7, and only two records of the whole table
fall outside it: `$07` FISSION2 (byte 10 — §2 notes that `EnemyAttack_FloatMine`
(`ps4.asm:22675`) clears `$24(a4)` and loads no object) and `$14` WARNING (byte 44,
a conditional-only record outside this file). Every other target byte above 15 is a
combine/fusion payload whose low nibble still lands on a real handler entry (see the
per-nibble row above).

### Byte 1 bit 7 (`$82`)

21 of the 112 records set it, all with selector 2 (mental, i.e. the raw
byte is `$82`): `$26`, `$27`, `$28`, `$29`, `$2A`, `$2D`, `$2E`, `$2F`, `$31`, `$32`, `$35`, `$3E`, `$40`, `$44`, `$45`, `$46`, `$47`, `$48`, `$49`, `$57`, `$5E`. Eighteen of them are
rows of §2 in [`ENEMY_ABILITIES.md`](ENEMY_ABILITIES.md); the other three (`$45`
RES, `$46` SAR, `$49` GISAR) are conditional-only records.

Two sites read byte 1 of an enemy-skill record, and both strip the bit before the
selector lookup:

- `Enemy_DamageCharacter` (`ps4.asm:3775`): reads at line 3797, `andi.w #$7F, d1`
  at line 3798, then indexes `AbilityStatsOffs` (`ps4.asm:9619`) for the power
  stat.
- `Effect_SetupSkillParams` (`ps4.asm:9576`): reads at line 9579, `andi.w #$7F, d1`
  at line 9580, same table.

No other code reads that byte, and nothing tests the bit itself, so it never
changes a damage number: the cartridge treats `$82` as selector 2. The
disassembly's own table header documents it as *"if set, it indicates that it
can't be used while tech sealed. This is generally applied to things with an
equivalent player tech"* (line 319266) — that is the fork's prose, and no reader
of the flag exists in this code path, so this file records the mask, not the claim.
A port must mask before selecting a stat: the extracted record keeps the raw value
(`generated/enemy_skills.json` shows `relevant_stat.id` 130 with a null name for
these 21 records, against 1..7 for the rest), and `technique::stat` maps 130 to 0.

### What counts as a damage request

`move.w #$C, $2(target)` writes `$C` to the target's `fighter_routine`
(`ps4.constants.asm:145`). `$C` is `Fighter_TakeDamage` in
`BattleCharacterRoutinePtrs` (`ps4.asm:1033`) and `BattleEnemyRoutinePtrs`
(`ps4.asm:1077`); it calls `Figher_DamageCheckActor` (`ps4.asm:3737`), and for an
enemy actor (lines 3743-3744) that reaches `Enemy_DamageCharacter`
(`ps4.asm:3775`), which computes the number from the actor's own ability slot
(`ability = $24` (`ps4.constants.asm:151`); read at line 3776) plus the skill
record, writes it to `Battle_Heal_Damage_List` and opens the damage window. That
write is the only way an enemy ability changes a party member's HP.

`move.w #5, $2(target)` writes routine 5 = `Character_DamageAnimation`
(`ps4.asm:12686`), which plays the flinch for the `$1C` timer
(`timer = $1C` (`ps4.constants.asm:96`)) and computes nothing. `ENEMY_ABILITIES.md` §1 counted both
writes as "requests damage"; this file counts only `$C`, and mentions the flinch
writes only where they are the whole route (the `other` rows). That is the one
place the two ledgers disagree: 15 rows here are `other` although that file classes
the ability `damage`, and §5 of this file lists them by name.

### Which target a request lands on

- `target = $38` (`ps4.constants.asm:139`) on a battle object is a longword
  pointer to a fighter. `Enemy_Attack` (`ps4.asm:19138`) fills the attack object's
  `$38` with the fighter at `Current_Target_Index` before dispatching the carrier
  routine (line 19172, via `loc_D024` (`ps4.asm:19167`)); carrier routines copy it
  into the objects they spawn (`move.l a0, $38(a1)`, e.g. line 21796) or forward it
  with `loc_24A20` (`ps4.asm:48452`). A request whose target register is loaded
  from `$38(a4)` is aimed at **the chosen party target**.
- `loc_24A9E` (`ps4.asm:48483`) and `loc_24BB6` (`ps4.asm:48562`) run a five-slot
  loop over `Obj_Fighters` (`ps4.constants.asm:2055`), stepping by `obj_size` `$40`,
  and write `#$C` to **every party slot**; several objects inline the same loop,
  some from the raw base `movea.l #$FF4400, a1`, which is `Obj_Fighters` itself.
  Those are the all-party requests.
- A carrier routine that writes `$FFFF` to `Current_Target_Index` (for example
  `EnemyAttack_TwinArms` line 23326, `EnemyAttack_ArmDrone` line 22798) makes
  `Enemy_Attack` mark *every* live party object with routine 2 (`loc_D076` (`ps4.asm:19186`)). That is the reaction/range side and changes no HP by itself;
  the same abilities' objects still request damage through one of the two routes
  above.

## 2. Route table

`single` — exactly one `move.w #$C` request against the chosen target: this is the
class that a generic resolver can take over. `all-party` — one request per party
slot. `multi-hit` — several requests that are not a single five-slot loop (mixed
chains). `other` — no `move.w #$C` request at all, or a scripted route; the note
says what runs instead.

| enemy | ability | `EnemyAttack_*` arm | object chain | damage request(s) | target selection | class |
|---|---|---|---|---|---|---|
| 0 Helex | `$02` FLAME BOLT | `EnemyAttack_Helex` (`ps4.asm:23574`) — no ability test in the carrier routine (the same object set for every nonzero id) | `$48` `BattleObj_HelexFlameBolt` (`ps4.asm:30279`) | 1 × `move.w #$C, $2(a3)` at line 30342 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 5 ForcedFly | `$02` FLAME BOLT | `EnemyAttack_ForcedFly` (`ps4.asm:23567`) — else arm, taken when the routine's tested ids do not match (test branch at line 23569) | `$48` `BattleObj_HelexFlameBolt` (`ps4.asm:30279`) | 1 × `move.w #$C, $2(a3)` at line 30342 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 2 GunnerBit | `$03` RAIL-GUN | `EnemyAttack_GunnerBit` (`ps4.asm:23604`) — no ability test in the carrier routine (the same object set for every nonzero id) | `$50` `BattleObj_GunnerBitAtk` (`ps4.asm:30361`) + `$54` `BattleObj_GunnerBitAtk2` (`ps4.asm:30450`) | 1 × `move.w #$C, $2(a3)` at line 30418 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 4 ProtectBit | `$04` LASRCANNON | `EnemyAttack_ProtectBit` (`ps4.asm:23637`) — no ability test in the carrier routine (the same object set for every nonzero id) | `$60` `BattleObj_ProtectBitAtk` (`ps4.asm:30592`) + `$64` `BattleObj_ProtectBitAtk2` (`ps4.asm:30706`) | 1 × `move.w #$C, $2(a3)` at line 30683 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 7 Seeker | `$04` LASRCANNON | `EnemyAttack_Seeker` (`ps4.asm:23663`) — no ability test in the carrier routine (the same object set for every nonzero id) | `$6C` `BattleObj_SeekerAtk` (`ps4.asm:30851`) | 1 × `move.w #$C, $2(a3)` at line 30818 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 8 Sweeper | `$04` LASRCANNON | `EnemyAttack_Sweeper` (`ps4.asm:23676`) — `beq.s EnemyAttack_Seeker` at line 23677 | `$6C` `BattleObj_SeekerAtk` (`ps4.asm:30851`) | 1 × `move.w #$C, $2(a3)` at line 30818 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 51 Loader | `$04` LASRCANNON | `loc_FF86` (`ps4.asm:22470`) — `bne.s loc_FFBE` at line 22471 | `$20C` `loc_16C04` (`ps4.asm:31936`) | 1 × `move.w #$C, $2(a3)` at line 31985 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 52 Debugger | `$04` LASRCANNON | `loc_FF86` (`ps4.asm:22470`) — `bne.s loc_FFBE` at line 22471 | `$20C` `loc_16C04` (`ps4.asm:31936`) | 1 × `move.w #$C, $2(a3)` at line 31985 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 8 Sweeper | `$05` LIGHTNING | `EnemyAttack_Sweeper` (`ps4.asm:23676`) — else arm, taken when the routine's tested ids do not match (test branch at line 23678) | `$70` `BattleObj_SweeperAtk` (`ps4.asm:30911`) | 1 × `move.w #$C, $2(a3)` at line 30818 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 15 Fanbite | `$08` SPIRAL BLD | `EnemyAttack_Locusta` (`ps4.asm:23472`) — else arm, taken when the routine's tested ids do not match (test branch at line 23487) | `$A0` `BattleObj_LocustaSpiralBld` (`ps4.asm:29175`) | 1 × `move.w #$C, $2(a1)` at line 29278 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 18 Servant | `$09` MOTRCANNON | `EnemyAttack_Slave` (`ps4.asm:23451`) — else arm, taken when the routine's tested ids do not match (test branch at line 23453) | `$AC` `BattleObj_SlaveMotrCannon` (`ps4.asm:28736`) + `$B0` `BattleObj_SlaveMotrCannon2` (`ps4.asm:28957`) | 1 × `move.w #$C, $2(a1)` at line 28921 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 26 LifeDeletr | `$0E` MICROMISSL | `.ability` (local label at line 23141) — `beq.s .micromissl` at line 23141 | `$80C` `BattleObj_LifeDeletrMicroMissl` (`ps4.asm:52229`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 28 DragerDuel | `$0E` MICROMISSL | `EnemyAttack_BalDuel` (`ps4.asm:23316`) — `beq.s loc_10AD2` at line 23317 | `$E8` `loc_378FC` (`ps4.asm:72007`) + `$E8` `loc_378FC` (`ps4.asm:72007`) | 1 × `move.w #$C, $2(a1)` at line 72120; 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | all five party slots (loop from `Obj_Fighters`); the stored target pointer `target` = `$38(a4)` (the chosen party target) | `multi-hit` — mixed chain: the object's own state 5 (`loc_37A40`, line 72112) requests one hit on each of the five party slots (`movea.l #$FF4400, a1` loop, line 72120), and the child it spawns, `BattleObj_AbeFrogAtk` (`ps4.asm:41843`), reaches the single-target tail `loc_24B20` (line 48529). Both requests use the actor's ability record. |
| 29 JurafaDuel | `$0E` MICROMISSL | `EnemyAttack_BalDuel` (`ps4.asm:23316`) — `beq.s loc_10AD2` at line 23317 | `$E8` `loc_378FC` (`ps4.asm:72007`) + `$E8` `loc_378FC` (`ps4.asm:72007`) | 1 × `move.w #$C, $2(a1)` at line 72120; 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | all five party slots (loop from `Obj_Fighters`); the stored target pointer `target` = `$38(a4)` (the chosen party target) | `multi-hit` — mixed chain: the object's own state 5 (`loc_37A40`, line 72112) requests one hit on each of the five party slots (`movea.l #$FF4400, a1` loop, line 72120), and the child it spawns, `BattleObj_AbeFrogAtk` (`ps4.asm:41843`), reaches the single-target tail `loc_24B20` (line 48529). Both requests use the actor's ability record. |
| 29 JurafaDuel | `$0F` FLAMLAUNCH | `EnemyAttack_BalDuel` (`ps4.asm:23316`) — else arm, taken when the routine's tested ids do not match (test branch at line 23319) | `$E0` `loc_374EA` (`ps4.asm:71643`) | 1 × `move.w #$C, $2(a1)` at line 71756 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 37 SnowSlug | `$13` CELL SPLIT | `loc_10644` (`ps4.asm:22959`) — `beq.s loc_106B0` at line 22960 | `$14C` `BattleObj_CellSplit` (`ps4.asm:35240`) + `$150` `BattleObj_CellSplit2` (`ps4.asm:35344`) + `$154` `BattleObj_CellSplit3` (`ps4.asm:35392`) + `$158` `BattleObj_CellSplit4` (`ps4.asm:35445`) + `$15C` `BattleObj_CellSplit5` (`ps4.asm:35493`) | 1 × `move.w #$C, $2(a3)` at line 35320 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 38 FractOoze | `$13` CELL SPLIT | `loc_10644` (`ps4.asm:22959`) — `beq.s loc_106B0` at line 22960 | `$14C` `BattleObj_CellSplit` (`ps4.asm:35240`) + `$150` `BattleObj_CellSplit2` (`ps4.asm:35344`) + `$154` `BattleObj_CellSplit3` (`ps4.asm:35392`) + `$158` `BattleObj_CellSplit4` (`ps4.asm:35445`) + `$15C` `BattleObj_CellSplit5` (`ps4.asm:35493`) | 1 × `move.w #$C, $2(a3)` at line 35320 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 137 FractOoze2 | `$13` CELL SPLIT | `loc_10644` (`ps4.asm:22959`) — `beq.s loc_106B0` at line 22960 | `$14C` `BattleObj_CellSplit` (`ps4.asm:35240`) + `$150` `BattleObj_CellSplit2` (`ps4.asm:35344`) + `$154` `BattleObj_CellSplit3` (`ps4.asm:35392`) + `$158` `BattleObj_CellSplit4` (`ps4.asm:35445`) + `$15C` `BattleObj_CellSplit5` (`ps4.asm:35493`) | 1 × `move.w #$C, $2(a3)` at line 35320 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 43 StarDrone | `$16` FLASH | `loc_10434` (`ps4.asm:22795`) — else arm, taken when the routine's tested ids do not match (test branch at line 22797) | `$194` `loc_1886C` (`ps4.asm:33905`) | none | — | `other` — no `move.w #$C` request anywhere in the chain: state 1 writes `move.w #5, $2(a3)` (line 33957) for every slot whose `Battle_Ability_Effects` entry is set, then opens the effect window (`Battle_Routine_2` = $27). Effect byte `$21` (DexterityDown) is applied by the effect handler, not by a hit. |
| 45 CommndBall | `$19` DETONATION | `loc_102CA` (`ps4.asm:22705`) — `bne.w loc_103C6` at line 22706 | `$1A0` `loc_17B5E` (`ps4.asm:33029`) + `$1A4` `loc_17DA6` (`ps4.asm:33183`) + `$1A8` `loc_17E1A` (`ps4.asm:33211`) + `$1AC` `loc_17F6C` (`ps4.asm:33295`) + `$1B0` `loc_1804A` (`ps4.asm:33351`) + `$1B4` `loc_18144` (`ps4.asm:33412`) + `$1B8` `loc_1816A` (`ps4.asm:33424`) + `$1BC` `loc_1824A` (`ps4.asm:33483`) + `$1C0` `loc_18270` (`ps4.asm:33495`) + `$1C4` `loc_18344` (`ps4.asm:33550`) | 1 × `move.w #$C, $2(a3)` at line 33124; 1 × `move.w #$C, $2(a3)` at line 33700 | all five party slots (loop from `Obj_Fighters`); the stored target pointer `target` = `$38(a4)` (the chosen party target) | `multi-hit` — ten objects are spawned; the lead object `loc_17B5E` (`ps4.asm:33029`) requests one hit on each of the five party slots (line 33124) and the last object `loc_18344` (`ps4.asm:33550`) reaches the single-target tail for one more hit (line 33700). `Current_Target_Index` is set to `$FFFF` first. |
| 47 Warren286 | `$1C` FLARE SHOT | `loc_1006E` (`ps4.asm:22545`) — `bne.s loc_100A6` at line 22546 | `$1D8` `loc_1786E` (`ps4.asm:32828`) | 1 × `move.w #$C, $2(a3)` at line 32877 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 48 Siren386 | `$1C` FLARE SHOT | `loc_1006E` (`ps4.asm:22545`) — `bne.s loc_100A6` at line 22546 | `$1D8` `loc_1786E` (`ps4.asm:32828`) | 1 × `move.w #$C, $2(a3)` at line 32877 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 49 Browren486 | `$1C` FLARE SHOT | `loc_1006E` (`ps4.asm:22545`) — `bne.s loc_100A6` at line 22546 | `$1D8` `loc_1786E` (`ps4.asm:32828`) | 1 × `move.w #$C, $2(a3)` at line 32877 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 130 DarkForce1 | `$1C` FLARE SHOT | `loc_DB88` (`ps4.asm:20038`) — `bne.s loc_DC08` at line 20049 | `$820` `loc_31F60` (`ps4.asm:64584`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 52 Debugger | `$1F` DBL SLASH | `loc_FF5E` (`ps4.asm:22459`) — `bne.s loc_FF86` at line 22460 | `$204` `loc_16D1C` (`ps4.asm:32015`) + `$208` `loc_16E9E` (`ps4.asm:32135`) | 1 × `move.w #$C, $2(a3)` at line 32125 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 53 Dominator | `$1F` DBL SLASH | `loc_FF5E` (`ps4.asm:22459`) — `bne.s loc_FF86` at line 22460 | `$204` `loc_16D1C` (`ps4.asm:32015`) + `$208` `loc_16E9E` (`ps4.asm:32135`) | 1 × `move.w #$C, $2(a3)` at line 32125 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 145 RedMole | `$1F` DBL SLASH | `loc_D59E` (`ps4.asm:19646`) — else arm, taken when the routine's tested ids do not match (test branch at line 19646) | `$8E0` `BattleObj_EnemyDblSlash` (`ps4.asm:68007`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 146 HungryMole | `$1F` DBL SLASH | `loc_D59E` (`ps4.asm:19646`) — else arm, taken when the routine's tested ids do not match (test branch at line 19646) | `$8E0` `BattleObj_EnemyDblSlash` (`ps4.asm:68007`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 53 Dominator | `$20` PHONONMASR | `loc_FFBE` (`ps4.asm:22484`) — else arm, taken when the routine's tested ids do not match (test branch at line 22484) | `$210` `loc_169C6` (`ps4.asm:31782`) | 1 × `move.w #$C, $2(a3)` at line 31881 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 130 DarkForce1 | `$20` PHONONMASR | `loc_DC08` (`ps4.asm:20065`) — `bne.s loc_DC56` at line 20066 | `$824` `loc_31CDC` (`ps4.asm:64401`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 58 FlameNewt | `$21` FIREBREATH | `loc_FD46` (`ps4.asm:22307`) — else arm, taken when the routine's tested ids do not match (test branch at line 22307) | `$248` `BattleObj_FireBreath` (`ps4.asm:42121`) | 1 × `move.w #$C, $2(a3)` at line 42097 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 59 StoneHeads | `$21` FIREBREATH | `loc_FE06` (`ps4.asm:22370`) — `bne.s loc_FE52` at line 22371 | `$228` `loc_16366` (`ps4.asm:31333`) + `$22C` `loc_16406` (`ps4.asm:31380`) | 1 × `move.w #$C, $2(a3)` at line 31453 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 83 Ripper | `$21` FIREBREATH | `loc_F372` (`ps4.asm:21603`) — `bne.s loc_F3B6` at line 21604 | `$34C` `loc_23D0A` (`ps4.asm:47507`) | 1 × `move.w #$C, $2(a3)` at line 47600 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 84 BladeRight | `$21` FIREBREATH | `loc_F372` (`ps4.asm:21603`) — `bne.s loc_F3B6` at line 21604 | `$34C` `loc_23D0A` (`ps4.asm:47507`) | 1 × `move.w #$C, $2(a3)` at line 47600 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 117 GyLaguiah | `$21` FIREBREATH | `loc_E2DC` (`ps4.asm:20527`) — `bne.s loc_E320` at line 20528 | `$780` `loc_2A01A` (`ps4.asm:55324`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 133 ProfoundDarkness1 | `$21` FIREBREATH | `loc_D8BE` (`ps4.asm:19849`) — `bne.s loc_D8FE` at line 19850 | `$870` `loc_30022` (`ps4.asm:62419`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 60 CrminHeads | `$22` RAY BREATH | `loc_FE52` (`ps4.asm:22389`) — `bne.s loc_FE9E` at line 22390 | `$230` `loc_1628A` (`ps4.asm:31273`) + `$22C` `loc_16406` (`ps4.asm:31380`) | 1 × `move.w #$C, $2(a3)` at line 31453 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 61 BlindHeads | `$22` RAY BREATH | `loc_FE52` (`ps4.asm:22389`) — `bne.s loc_FE9E` at line 22390 | `$230` `loc_1628A` (`ps4.asm:31273`) + `$22C` `loc_16406` (`ps4.asm:31380`) | 1 × `move.w #$C, $2(a3)` at line 31453 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 118 LwAddmer | `$22` RAY BREATH | `loc_E320` (`ps4.asm:20544`) — `bne.s loc_E35E` at line 20545 | `$784` `loc_29EEE` (`ps4.asm:55243`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 133 ProfoundDarkness1 | `$22` RAY BREATH | `loc_D8FE` (`ps4.asm:19866`) — `bne.s loc_D93E` at line 19867 | `$878` `loc_2FE7C` (`ps4.asm:62296`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 142 Skytiara | `$23` SUPERSONIC | `loc_D5D4` (`ps4.asm:19663`) — `bne.s loc_D61E` at line 19664 | `$8C4` `BattleObj_OwlSupersonic` (`ps4.asm:68761`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 77 TechPlant | `$2A` RIMIT | `loc_F65E` (`ps4.asm:21822`) — `bne.s loc_F6C4` at line 21823 | `$2F8` `BattleObj_EnemyRimit` (`ps4.asm:38366`) + `$2F0` `BattleObj_AcidBreathChild` (`ps4.asm:38622`) | none | — | `other` — no `move.w #$C` request in the chain; `BattleObj_EnemyRimit` (`ps4.asm:38366`) spawns one effect child (`$2AC` `BattleObj_EnemyRimitChild` (`ps4.asm:48736`)) per affected slot and never writes a fighter routine. Effect byte `$07` (SleepParalyze). |
| 115 Greneris | `$2A` RIMIT | `loc_E4C4` (`ps4.asm:20648`) — `bne.s loc_E516` at line 20649 | `$754` `loc_2ACD6` (`ps4.asm:56212`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_2ACD6` (`ps4.asm:56212`) runs the effect pipeline (`Battle_Routine_2` = $27) and spawns effect children. Effect byte `$07` (SleepParalyze). |
| 68 Rajago | `$2B` NEEDLE | `loc_FA1A` (`ps4.asm:22067`) — `bne.s loc_FA2A` at line 22068 | `$288` `BattleObj_Needle` (`ps4.asm:40740`) | 1 × `move.w #$C, $2(a3)` at line 48547 (tail `loc_24B64` (`ps4.asm:48541`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 69 BiterFly | `$2B` NEEDLE | `loc_FA1A` (`ps4.asm:22067`) — `bne.s loc_FA2A` at line 22068 | `$288` `BattleObj_Needle` (`ps4.asm:40740`) | 1 × `move.w #$C, $2(a3)` at line 48547 (tail `loc_24B64` (`ps4.asm:48541`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 146 HungryMole | `$2B` NEEDLE | `loc_D56E` (`ps4.asm:19633`) — `bne.s loc_D59E` at line 19634 | `$8D4` `BattleObj_MoleNeedle` (`ps4.asm:68370`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 71 FrostSaber | `$2E` GIWAT | `loc_F8F2` (`ps4.asm:21987`) — `bne.s loc_F93C` at line 21988 | `$2B4` `loc_1D5AA` (`ps4.asm:39951`) | 1 × `move.w #$C, $2(a3)` at line 40042 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 77 TechPlant | `$2E` GIWAT | `loc_F6C4` (`ps4.asm:21846`) — `bne.s loc_F722` at line 21847 | `$2FC` `BattleObj_EnemyGiwat` (`ps4.asm:38195`) + `$2F0` `BattleObj_AcidBreathChild` (`ps4.asm:38622`) | 1 × `move.w #$C, $2(a3)` at line 48513 (tail `loc_24AEC` (`ps4.asm:48507`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 91 HewGilla | `$2E` GIWAT | `loc_F108` (`ps4.asm:21426`) — else arm, taken when the routine's tested ids do not match (test branch at line 21426) | `$390` `loc_22670` (`ps4.asm:45925`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 101 DarkWitch | `$2E` GIWAT | `loc_EE0E` (`ps4.asm:21229`) — `bne.s loc_EE58` at line 21230 | `$3C4` `loc_21852` (`ps4.asm:45013`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 122 DElmLars | `$2E` GIWAT | `loc_DFBE` (`ps4.asm:20300`) — `bne.s loc_E008` at line 20301 | `$7B8` `loc_28F76` (`ps4.asm:54226`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 123 XeAThoul | `$2E` GIWAT | `loc_DFBE` (`ps4.asm:20300`) — `bne.s loc_E008` at line 20301 | `$7B8` `loc_28F76` (`ps4.asm:54226`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 72 BloodSaber | `$2F` VOL | `loc_F93C` (`ps4.asm:22005`) — `bne.s loc_F986` at line 22006 | `$2C0` `loc_1D2F4` (`ps4.asm:39774`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_1D2F4` (`ps4.asm:39774`) and its children apply the effect only (a `move.w #5, $2(a3)` flinch is written by a child in the same block, line 39933). Effect byte `$02` (Death). |
| 88 SoldrFiend | `$2F` VOL | `loc_F1E2` (`ps4.asm:21484`) — `bne.s loc_F228` at line 21485 | `$374` `loc_23216` (`ps4.asm:46736`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_23216` (`ps4.asm:46736`) runs `GetEnemySkillEffectAndRange` and spawns one effect child per affected slot, one of which writes `move.w #5, $2(a3)` (line 46939). Effect byte `$02` (Death). |
| 115 Greneris | `$2F` VOL | `loc_E47A` (`ps4.asm:20630`) — `bne.s loc_E4C4` at line 20631 | `$74C` `loc_2AE8E` (`ps4.asm:56333`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_2AE8E` (`ps4.asm:56333`) and its children (`loc_2AE0C`, `$2AC`) apply the effect only. Effect byte `$02` (Death) is applied by the effect handler. |
| 134 ProfoundDarkness2 | `$30` DISTORTION | `loc_D774` (`ps4.asm:19766`) — `bne.s loc_D7B8` at line 19767 | `$890` `loc_2F1B2` (`ps4.asm:61394`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 73 DimensWorm | `$31` GRA | `loc_F7BE` (`ps4.asm:21909`) — else arm, taken when the routine's tested ids do not match (test branch at line 21909) | `$2E0` `loc_1C99C` (`ps4.asm:39050`) + `$2DC` `loc_1C73E` (`ps4.asm:38879`) + `$2D8` `loc_1C658` (`ps4.asm:38807`) | 1 × `move.w #$C, $2(a3)` at line 39021 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 74 OuterBeast | `$31` GRA | `loc_F7BE` (`ps4.asm:21909`) — else arm, taken when the routine's tested ids do not match (test branch at line 21909) | `$2E0` `loc_1C99C` (`ps4.asm:39050`) + `$2DC` `loc_1C73E` (`ps4.asm:38879`) + `$2D8` `loc_1C658` (`ps4.asm:38807`) | 1 × `move.w #$C, $2(a3)` at line 39021 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 74 OuterBeast | `$32` GIGRA | `loc_F7BE` (`ps4.asm:21909`) — else arm, taken when the routine's tested ids do not match (test branch at line 21909) | `$2E0` `loc_1C99C` (`ps4.asm:39050`) + `$2DC` `loc_1C73E` (`ps4.asm:38879`) + `$2D8` `loc_1C658` (`ps4.asm:38807`) | 1 × `move.w #$C, $2(a3)` at line 39021 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 75 FlattrPlnt | `$33` ACIDBREATH | `loc_F5BE` (`ps4.asm:21783`) — `bne.s loc_F60A` at line 21784 | `$2EC` `BattleObj_AcidBreath` (`ps4.asm:38566`) + `$2F0` `BattleObj_AcidBreathChild` (`ps4.asm:38622`) | 1 × `move.w #$C, $2(a3)` at line 48513 (tail `loc_24AEC` (`ps4.asm:48507`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 76 FlyScreamr | `$33` ACIDBREATH | `loc_F5BE` (`ps4.asm:21783`) — `bne.s loc_F60A` at line 21784 | `$2EC` `BattleObj_AcidBreath` (`ps4.asm:38566`) + `$2F0` `BattleObj_AcidBreathChild` (`ps4.asm:38622`) | 1 × `move.w #$C, $2(a3)` at line 48513 (tail `loc_24AEC` (`ps4.asm:48507`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 85 Piercer | `$33` ACIDBREATH | `loc_F2A0` (`ps4.asm:21532`) — `bne.s loc_F2E4` at line 21533 | `$35C` `loc_23998` (`ps4.asm:47264`) | 1 × `move.w #$C, $2(a3)` at line 47352 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 86 HakenLeft | `$33` ACIDBREATH | `loc_F2A0` (`ps4.asm:21532`) — `bne.s loc_F2E4` at line 21533 | `$35C` `loc_23998` (`ps4.asm:47264`) | 1 × `move.w #$C, $2(a3)` at line 47352 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 77 TechPlant | `$35` GIZAN | `loc_F722` (`ps4.asm:21869`) — else arm, taken when the routine's tested ids do not match (test branch at line 21869) | `$304` `BattleObj_EnemyGizan` (`ps4.asm:38024`) + `$2F0` `BattleObj_AcidBreathChild` (`ps4.asm:38622`) | 1 × `move.w #$C, $2(a3)` at line 38142 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 101 DarkWitch | `$35` GIZAN | `loc_ED28` (`ps4.asm:21174`) — `bne.s loc_ED7A` at line 21175 | `$3DC` `loc_21176` (`ps4.asm:44547`) | 1 × `move.w #$C, $2(a3)` at line 44895 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 122 DElmLars | `$35` GIZAN | `loc_DF22` (`ps4.asm:20263`) — `bne.s loc_DF74` at line 20264 | `$7B0` `loc_2912C` (`ps4.asm:54326`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 123 XeAThoul | `$35` GIZAN | `loc_DF22` (`ps4.asm:20263`) — `bne.s loc_DF74` at line 20264 | `$7B0` `loc_2912C` (`ps4.asm:54326`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 124 LeFawGan | `$35` GIZAN | `loc_DF22` (`ps4.asm:20263`) — `bne.s loc_DF74` at line 20264 | `$7B0` `loc_2912C` (`ps4.asm:54326`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 125 GiLeFarg | `$35` GIZAN | `loc_DF22` (`ps4.asm:20263`) — `bne.s loc_DF74` at line 20264 | `$7B0` `loc_2912C` (`ps4.asm:54326`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 81 DesrtLeach | `$37` SAND STORM | `loc_F48A` (`ps4.asm:21692`) — `bne.s loc_F4D4` at line 21693 | `$328` `BattleObj_SandStorm` (`ps4.asm:48204`) | 1 × `move.w #$C, $2(a3)` at line 48547 (tail `loc_24B64` (`ps4.asm:48541`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 80 SandWorm | `$38` EARTHQUAKE | `loc_F4D4` (`ps4.asm:21710`) — `bne.s loc_F4FA` at line 21711 | `$330` `BattleObj_Earthquake` (`ps4.asm:47884`) | 1 × `move.w #$C, $2(a3)` at line 47998 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 149 KingRappy | `$38` EARTHQUAKE | `loc_D516` (`ps4.asm:19608`) — else arm, taken when the routine's tested ids do not match (test branch at line 19608) | `$904` `BattleObj_KingRappyEarthquake` (`ps4.asm:67513`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 82 Leviathan | `$39` MAELSTROM | `loc_F4FA` (`ps4.asm:21720`) — else arm, taken when the routine's tested ids do not match (test branch at line 21720) | `$338` `BattleObj_Maelstrom` (`ps4.asm:47766`) | 1 × `move.w #$C, $2(a3)` at line 48547 (tail `loc_24B64` (`ps4.asm:48541`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 87 TwinArms | `$3C` BLADESHINE | `EnemyAttack_TwinArms` (`ps4.asm:21455`) — `bne.s loc_F1A2` at line 21456 | `$364` `loc_237AA` (`ps4.asm:47129`) | 1 × `move.w #$C, $2(a3)` at line 47237 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 88 SoldrFiend | `$3C` BLADESHINE | `EnemyAttack_TwinArms` (`ps4.asm:21455`) — `bne.s loc_F1A2` at line 21456 | `$364` `loc_237AA` (`ps4.asm:47129`) | 1 × `move.w #$C, $2(a3)` at line 47237 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 87 TwinArms | `$3D` HAKEN BOLT | `loc_F1A2` (`ps4.asm:21468`) — `bne.s loc_F1E2` at line 21469 | `$36C` `loc_23544` (`ps4.asm:46959`) | 1 × `move.w #$C, $2(a3)` at line 47032 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 88 SoldrFiend | `$3D` HAKEN BOLT | `loc_F1A2` (`ps4.asm:21468`) — `bne.s loc_F1E2` at line 21469 | `$36C` `loc_23544` (`ps4.asm:46959`) | 1 × `move.w #$C, $2(a3)` at line 47032 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 90 Depcen | `$3F` FLODBREATH | `loc_F13C` (`ps4.asm:21443`) — else arm, taken when the routine's tested ids do not match (test branch at line 21443) | `$380` `BattleObj_FlodBreath` (`ps4.asm:46319`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 91 HewGilla | `$3F` FLODBREATH | `EnemyAttack_HewGilla` (`ps4.asm:21395`) — `bne.s loc_F0CC` at line 21400 | `$384` `loc_22A90` (`ps4.asm:46199`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 92 Elmelew | `$3F` FLODBREATH | `EnemyAttack_HewGilla` (`ps4.asm:21395`) — `bne.s loc_F0CC` at line 21400 | `$384` `loc_22A90` (`ps4.asm:46199`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 91 HewGilla | `$40` WAT | `loc_F0CC` (`ps4.asm:21412`) — `bne.s loc_F108` at line 21418 | `$388` `BattleObj_EnemyWat` (`ps4.asm:45978`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 92 Elmelew | `$40` WAT | `loc_F0CC` (`ps4.asm:21412`) — `bne.s loc_F108` at line 21418 | `$388` `BattleObj_EnemyWat` (`ps4.asm:45978`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 99 TechUser | `$40` WAT | `loc_EDC4` (`ps4.asm:21211`) — `bne.s loc_EE0E` at line 21212 | `$3C0` `loc_218D6` (`ps4.asm:45047`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 100 TechMaster | `$40` WAT | `loc_EDC4` (`ps4.asm:21211`) — `bne.s loc_EE0E` at line 21212 | `$3C0` `loc_218D6` (`ps4.asm:45047`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 114 Juza | `$40` WAT | `loc_E3DE` (`ps4.asm:20593`) — `bne.s loc_E428` at line 20594 | `$744` `loc_2B006` (`ps4.asm:56437`) | 1 × `move.w #$C, $2(a3)` at line 56553 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 97 KingSaber | `$42` RAY-SPEAR | `loc_EFF6` (`ps4.asm:21360`) — `bne.s loc_F03C` at line 21361 | `$3A4` `loc_220B8` (`ps4.asm:45533`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 98 DarkRider | `$42` RAY-SPEAR | `loc_EFF6` (`ps4.asm:21360`) — `bne.s loc_F03C` at line 21361 | `$3A4` `loc_220B8` (`ps4.asm:45533`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 98 DarkRider | `$43` THROWLANCR | `loc_F03C` (`ps4.asm:21377`) — else arm, taken when the routine's tested ids do not match (test branch at line 21377) | `$3A8` `loc_21ED2` (`ps4.asm:45414`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 99 TechUser | `$44` FOI | `EnemyAttack_TechUser` (`ps4.asm:21156`) — `bne.s loc_ED28` at line 21157 | `$3B0` `loc_21BF0` (`ps4.asm:45229`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 100 TechMaster | `$44` FOI | `EnemyAttack_TechUser` (`ps4.asm:21156`) — `bne.s loc_ED28` at line 21157 | `$3B0` `loc_21BF0` (`ps4.asm:45229`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 114 Juza | `$44` FOI | `EnemyAttack_Juza` (`ps4.asm:20575`) — `bne.s loc_E3DE` at line 20576 | `$740` `loc_2B08E` (`ps4.asm:56477`) | 1 × `move.w #$C, $2(a3)` at line 56553 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 100 TechMaster | `$47` ZAN | `loc_EE58` (`ps4.asm:21247`) — `bne.s loc_EEAA` at line 21248 | `$3D0` `loc_215D6` (`ps4.asm:44836`) | 1 × `move.w #$C, $2(a3)` at line 44895 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 114 Juza | `$47` ZAN | `loc_E428` (`ps4.asm:20611`) — `bne.s loc_E47A` at line 20612 | `$748` `loc_2AF18` (`ps4.asm:56372`) | 1 × `move.w #$C, $2(a3)` at line 56426 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 101 DarkWitch | `$48` GIFOI | `loc_ED7A` (`ps4.asm:21193`) — `bne.s loc_EDC4` at line 21194 | `$3B8` `loc_21960` (`ps4.asm:45087`) | 1 × `move.w #$C, $2(a3)` at line 45310 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 124 LeFawGan | `$48` GIFOI | `loc_DF74` (`ps4.asm:20282`) — `bne.s loc_DFBE` at line 20283 | `$7B4` `loc_2900E` (`ps4.asm:54263`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 105 ShadMirage | `$4A` STAR DUST | `+` (line 21109) — `bne.s +` at line 21109 | `$3EC` `loc_20B96` (`ps4.asm:44101`) | 1 × `move.w #$C, $2(a3)` at line 44267 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 106 Haunt | `$4C` EVIL EYE | `loc_EAC6` (`ps4.asm:21023`) — `bne.s loc_EB04` at line 21024 | `$700` `loc_2CB1C` (`ps4.asm:58346`) | none | — | `other` — no `move.w #$C` request in the chain; object `$700` = `loc_2CB1C` (`ps4.asm:58346`). Effect byte `$07` (SleepParalyze). |
| 107 Spector | `$4C` EVIL EYE | `loc_EAC6` (`ps4.asm:21023`) — `bne.s loc_EB04` at line 21024 | `$700` `loc_2CB1C` (`ps4.asm:58346`) | none | — | `other` — no `move.w #$C` request in the chain; object `$700` = `loc_2CB1C` (`ps4.asm:58346`). Effect byte `$07` (SleepParalyze). |
| 131 DarkForce2 | `$4C` EVIL EYE | `EnemyAttack_DarkForce2` (`ps4.asm:19970`) — ordinary dispatch; `$64` and `$65` are tested (`loc_DAA8`, `loc_DAEC`), the fall-through `loc_DB30` (`ps4.asm:20013`) is the arm `$4C` lands in | `$850` `loc_30DC0` (`ps4.asm:63323`) | none | — | `other` — no `move.w #$C` request in the chain; object `$850` = `loc_30DC0` (`ps4.asm:63323`) drives the effect pipeline and a child (`BattleObj_LifeDeletrMicroMissl2` (`ps4.asm:49043`)) writes `move.w #5, $2(a3)` (line 49057). The gated arm (`$FFFFEE87` set) clears the ability and loads `$83C` instead. |
| 135 ProfoundDarkness3 | `$4C` EVIL EYE | `loc_D660` (`ps4.asm:19699`) — `bne.s loc_D6A2` at line 19700 | `$8A4` `loc_2E524` (`ps4.asm:60482`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_2E524` (`ps4.asm:60482`) spawns per-target effect objects (`$340`, `$314`, `$2AC`). Effect byte `$07` (SleepParalyze). |
| 106 Haunt | `$4D` CORRSION | `loc_EB04` (`ps4.asm:21039`) — `bne.s loc_EB50` at line 21040 | `$704` `loc_2C98A` (`ps4.asm:58230`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 107 Spector | `$4D` CORRSION | `loc_EB04` (`ps4.asm:21039`) — `bne.s loc_EB50` at line 21040 | `$704` `loc_2C98A` (`ps4.asm:58230`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 111 ChaosSorcr | `$4D` CORRSION | `loc_E7EC` (`ps4.asm:20849`) — `bne.s loc_E83E` at line 20850 | `$720` `loc_2BED2` (`ps4.asm:57485`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 112 Illusionst | `$4D` CORRSION | `loc_E7EC` (`ps4.asm:20849`) — `bne.s loc_E83E` at line 20850 | `$720` `loc_2BED2` (`ps4.asm:57485`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 113 ImagioMage | `$4D` CORRSION | `loc_E7EC` (`ps4.asm:20849`) — `bne.s loc_E83E` at line 20850 | `$720` `loc_2BED2` (`ps4.asm:57485`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 132 DarkForce3 | `$4D` CORRSION | `loc_D9FE` (`ps4.asm:19934`) — `bne.s loc_DA48` at line 19935 | `$85C` `loc_308FA` (`ps4.asm:62998`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 140 Zio2 | `$4D` CORRSION | `loc_D458` (`ps4.asm:19556`) — `bne.s loc_D498` at line 19562 | `$928` `loc_33646` (`ps4.asm:66537`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` — object `$928` = `loc_33646` (`ps4.asm:66537`) reaches the `loc_24BB6` exit: one hit on each of the five party slots (line 48575). Only reached with the phase counter `$FFFFEE98` clear, i.e. Zio2's first action. |
| 108 Phantom | `$4F` HEWN | `loc_EB94` (`ps4.asm:21074`) — else arm, taken when the routine's tested ids do not match (test branch at line 21074) | `$70C` `loc_2C6E2` (`ps4.asm:58047`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 111 ChaosSorcr | `$4F` HEWN | `loc_E926` (`ps4.asm:20923`) — `bne.s loc_E978` at line 20924 | `$730` `loc_2B6F8` (`ps4.asm:56987`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 112 Illusionst | `$4F` HEWN | `loc_E926` (`ps4.asm:20923`) — `bne.s loc_E978` at line 20924 | `$730` `loc_2B6F8` (`ps4.asm:56987`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 113 ImagioMage | `$4F` HEWN | `loc_E926` (`ps4.asm:20923`) — `bne.s loc_E978` at line 20924 | `$730` `loc_2B6F8` (`ps4.asm:56987`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 121 SaLews | `$4F` HEWN | `loc_E136` (`ps4.asm:20414`) — `bne.s loc_E188` at line 20415 | `$798` `loc_299BE` (`ps4.asm:54905`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 138 ChaosSorcr2 | `$4F` HEWN | `loc_E926` (`ps4.asm:20923`) — `bne.s loc_E978` at line 20924 | `$730` `loc_2B6F8` (`ps4.asm:56987`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 140 Zio2 | `$4F` HEWN | `loc_D498` (`ps4.asm:19570`) — else arm, taken when the routine's tested ids do not match (test branch at line 19570) | `$930` `loc_335A6` (`ps4.asm:66498`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` — the fall-through arm (`loc_D498`, `ps4.asm:19570`; neither `$6C` nor `$4D`) loads object `$930` = `loc_335A6` (`ps4.asm:66498`), whose `loc_24BB6` exit requests one hit on each of the five party slots (line 48575). Only reached with the phase counter `$FFFFEE98` clear. |
| 112 Illusionst | `$51` MINDBLST | `loc_E79A` (`ps4.asm:20830`) — `bne.s loc_E7EC` at line 20831 | `$71C` `loc_2C0D4` (`ps4.asm:57615`) | none | — | `other` — no `move.w #$C` request reachable from `loc_2C0D4` (`ps4.asm:57615`); the object runs `GetEnemySkillEffectAndRange` and spawns one effect child per affected slot. Effect byte `$07` (SleepParalyze) is applied by the effect handler. |
| 113 ImagioMage | `$51` MINDBLST | `loc_E79A` (`ps4.asm:20830`) — `bne.s loc_E7EC` at line 20831 | `$71C` `loc_2C0D4` (`ps4.asm:57615`) | none | — | `other` — no `move.w #$C` request reachable from `loc_2C0D4` (`ps4.asm:57615`); the object runs `GetEnemySkillEffectAndRange` and spawns one effect child per affected slot. Effect byte `$07` (SleepParalyze) is applied by the effect handler. |
| 132 DarkForce3 | `$51` MINDBLST | `loc_DA48` (`ps4.asm:19952`) — else arm, taken when the routine's tested ids do not match (test branch at line 19952) | `$860` `loc_30792` (`ps4.asm:62902`) | none | — | `other` — no `move.w #$C` request in the chain; object `$860` = `loc_30792` (`ps4.asm:62902`) drives the effect pipeline only. Effect byte `$07` (SleepParalyze). |
| 111 ChaosSorcr | `$52` TANDLE | `loc_E9CC` (`ps4.asm:20961`) — else arm, taken when the routine's tested ids do not match (test branch at line 20961) | `$73C` `loc_2B23C` (`ps4.asm:56594`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 112 Illusionst | `$52` TANDLE | `loc_E9CC` (`ps4.asm:20961`) — else arm, taken when the routine's tested ids do not match (test branch at line 20961) | `$73C` `loc_2B23C` (`ps4.asm:56594`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 113 ImagioMage | `$52` TANDLE | `loc_E9CC` (`ps4.asm:20961`) — else arm, taken when the routine's tested ids do not match (test branch at line 20961) | `$73C` `loc_2B23C` (`ps4.asm:56594`) | 1 × `move.w #$C, $2(a3)` at line 57589 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 121 SaLews | `$52` TANDLE | `loc_E188` (`ps4.asm:20433`) — `bne.s loc_E1DC` at line 20434 | `$79C` `loc_29910` (`ps4.asm:54862`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 113 ImagioMage | `$55` LEGEON | `loc_E978` (`ps4.asm:20942`) — `bne.s loc_E9CC` at line 20943 | `$734` `loc_2B5FA` (`ps4.asm:56873`) | 1 × `move.w #$C, $2(a3)` at line 56959 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 114 Juza | `$56` FORCEFLASH | `loc_E6FE` (`ps4.asm:20783`) — else arm, taken when the routine's tested ids do not match (test branch at line 20783) | `$778` `loc_2A300` (`ps4.asm:55527`) | 1 × `move.w #$C, $2(a3)` at line 56426 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 115 Greneris | `$56` FORCEFLASH | `loc_E6FE` (`ps4.asm:20783`) — else arm, taken when the routine's tested ids do not match (test branch at line 20783) | `$778` `loc_2A300` (`ps4.asm:55527`) | 1 × `move.w #$C, $2(a3)` at line 56426 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 116 Radhin | `$56` FORCEFLASH | `loc_E6FE` (`ps4.asm:20783`) — else arm, taken when the routine's tested ids do not match (test branch at line 20783) | `$778` `loc_2A300` (`ps4.asm:55527`) | 1 × `move.w #$C, $2(a3)` at line 56426 | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 119 CulaBellr | `$58` LGHTBREATH | `loc_E35E` (`ps4.asm:20560`) — else arm, taken when the routine's tested ids do not match (test branch at line 20560) | `$788` `loc_29DC6` (`ps4.asm:55162`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 111 ChaosSorcr | `$5A` FLAELI | `loc_E890` (`ps4.asm:20887`) — `bne.s loc_E8DA` at line 20888 | `$728` `loc_2BAA2` (`ps4.asm:57218`) | 1 × `move.w #$C, $2(a3)` at line 57337 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 112 Illusionst | `$5A` FLAELI | `loc_E890` (`ps4.asm:20887`) — `bne.s loc_E8DA` at line 20888 | `$728` `loc_2BAA2` (`ps4.asm:57218`) | 1 × `move.w #$C, $2(a3)` at line 57337 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 113 ImagioMage | `$5A` FLAELI | `loc_E890` (`ps4.asm:20887`) — `bne.s loc_E8DA` at line 20888 | `$728` `loc_2BAA2` (`ps4.asm:57218`) | 1 × `move.w #$C, $2(a3)` at line 57337 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 121 SaLews | `$5A` FLAELI | `EnemyAttack_SaLews` (`ps4.asm:20396`) — `bne.s loc_E136` at line 20397 | `$794` `loc_29A76` (`ps4.asm:54949`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 125 GiLeFarg | `$5A` FLAELI | `loc_E008` (`ps4.asm:20318`) — `bne.s loc_E052` at line 20319 | `$7BC` `loc_28E36` (`ps4.asm:54155`) | 1 × `move.w #$C, $2(a3)` at line 48474 (tail `loc_24A6C` (`ps4.asm:48468`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 138 ChaosSorcr2 | `$5A` FLAELI | `loc_E890` (`ps4.asm:20887`) — `bne.s loc_E8DA` at line 20888 | `$728` `loc_2BAA2` (`ps4.asm:57218`) | 1 × `move.w #$C, $2(a3)` at line 57337 | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 124 LeFawGan | `$5D` TANDIL | `loc_DECE` (`ps4.asm:20240`) — `bne.s loc_DF22` at line 20241 | `$7AC` `loc_29306` (`ps4.asm:54450`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 125 GiLeFarg | `$5D` TANDIL | `loc_DECE` (`ps4.asm:20240`) — `bne.s loc_DF22` at line 20241 | `$7AC` `loc_29306` (`ps4.asm:54450`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 127 ReFaze | `$5E` MEGID | `EnemyAttack_ReFaze` (`ps4.asm:20193`) — no ability test in the carrier routine (the same object set for every nonzero id) | `$7E0` `loc_284C0` (`ps4.asm:53503`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 135 ProfoundDarkness3 | `$5E` MEGID | `loc_D6F2` (`ps4.asm:19733`) — else arm, taken when the routine's tested ids do not match (test branch at line 19733) | `$8AC` `loc_2DF4A` (`ps4.asm:60068`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 128 Lashiec | `$5F` THNDHALBRT | `EnemyAttack_Lashiec` (`ps4.asm:20116`) — `bne.s loc_DD2C` at line 20117 | `$7E4` `loc_281F6` (`ps4.asm:53309`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 128 Lashiec | `$60` POSESSION | `loc_DD2C` (`ps4.asm:20136`) — `bne.s loc_DD84` at line 20137 | `$7E8` `loc_2805C` (`ps4.asm:53215`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_2805C` (`ps4.asm:53215`) spawns children, one of which (`BattleObj_LifeDeletrMicroMissl2` (`ps4.asm:49043`)) writes `move.w #5, $2(a3)` (line 49057). Effect byte `$07` (SleepParalyze). |
| 128 Lashiec | `$61` ANOTHRGATE | `loc_DDD6` (`ps4.asm:20175`) — else arm, taken when the routine's tested ids do not match (test branch at line 20175) | `$7F4` `loc_27B02` (`ps4.asm:52835`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 134 ProfoundDarkness2 | `$61` ANOTHRGATE | `loc_D7B8` (`ps4.asm:19782`) — `bne.s loc_D808` at line 19783 | `$894` `loc_2F060` (`ps4.asm:61289`) | 1 × `move.w #$C, $2(a3)` at line 48575 (tail `loc_24BB6` (`ps4.asm:48562`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 131 DarkForce2 | `$65` LIGHTSHOWR | `loc_DAEC` (`ps4.asm:19996`) — `bne.s loc_DB30` at line 19997 | `$848` `loc_30FD2` (`ps4.asm:63467`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 134 ProfoundDarkness2 | `$65` LIGHTSHOWR | `loc_D808` (`ps4.asm:19800`) — `bne.s loc_D84C` at line 19801 | `$898` `loc_2EF0E` (`ps4.asm:61209`) | 1 × `move.w #$C, $2(a3)` at line 48496 (tail `loc_24A9E` (`ps4.asm:48483`)) | all five party slots (loop from `Obj_Fighters`) | `all-party` |
| 135 ProfoundDarkness3 | `$69` CANCELING | `loc_D6A2` (`ps4.asm:19715`) — `bne.s loc_D6F2` at line 19716 | `$8A8` `loc_2E2DE` (`ps4.asm:60310`) | none | — | `other` — no `move.w #$C` request in the chain; `loc_2E2DE` (`ps4.asm:60310`) spawns per-target effect objects. Effect byte `$27` (RestoreStats). |
| 143 Owltalon | `$6A` WIND STORM | `loc_D61E` (`ps4.asm:19680`) — else arm, taken when the routine's tested ids do not match (test branch at line 19680) | `$8CC` `loc_3526A` (`ps4.asm:68568`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 147 Rappy | `$6D` ROUND EYES | `loc_D4BE` (`ps4.asm:19583`) — `bne.s loc_D4DE` at line 19588 | `$8F8` `BattleObj_RoundEyes` (`ps4.asm:67760`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |
| 148 BlueRappy | `$6E` LOVEL EYES | `loc_D4DE` (`ps4.asm:19592`) — else arm, taken when the routine's tested ids do not match (test branch at line 19592) | `$8FC` `BattleObj_LovelEyes` (`ps4.asm:67729`) | 1 × `move.w #$C, $2(a3)` at line 48529 (tail `loc_24B20` (`ps4.asm:48523`)) | the stored target pointer `target` = `$38(a4)` (the chosen party target) | `single` |

## 3. Summary

| class | pairs |
|---|---|
| `single` | 70 |
| `all-party` | 58 |
| `multi-hit` | 3 |
| `other` | 15 |
| **total** | **146** |

`single` covers 70 of the 146 pairs across
25 of the 58 abilities, and the four ACID BREATH
pairs (`ENEMY_ABILITIES.md` §2 shows no class for `$33` because it is
implemented) are among them: `EnemyAttack_FlattrPlnt`'s `$33` arm makes one
`move.w #$C, $2(a3)` in the `loc_24AEC` tail (line 48513) and
`EnemyAttack_Piercer`'s `$33` arm makes one in `loc_23AB6` (line 47352), both
against `$38(a4)`. That matches `resolve_acid_breath`'s reading in
`rust/psiv-core/src/battle/enemy_skill.rs`.

### Map 000 Motavia (the next campaign's priority)

`docs/ENEMY_ABILITIES.md` §4 lists these damage abilities as reachable from map
`000 Motavia`; the pairs below are the rows of this table whose ability has that
map. §4 attributes maps per ability, so a listed pair is reachable from the map
only if that carrier's formation is one Motavia's groups can roll — the carrier
lists in §2 are repeated here so a later lane can narrow it per enemy when it
touches a formation.

| ability | class(es) of its Motavia pairs | pairs |
|---|---|---|
| `$02` FLAME BOLT | single | 0 Helex (single); 5 ForcedFly (single) |
| `$08` SPIRAL BLD | all-party | 15 Fanbite (all-party) |
| `$2E` GIWAT | single | 71 FrostSaber (single); 77 TechPlant (single); 91 HewGilla (single); 101 DarkWitch (single); 122 DElmLars (single); 123 XeAThoul (single) |
| `$37` SAND STORM | single | 81 DesrtLeach (single) |
| `$38` EARTHQUAKE | all-party | 80 SandWorm (all-party); 149 KingRappy (all-party) |
| `$39` MAELSTROM | single | 82 Leviathan (single) |
| `$3F` FLODBREATH | single | 90 Depcen (single); 91 HewGilla (single); 92 Elmelew (single) |
| `$40` WAT | single | 91 HewGilla (single); 92 Elmelew (single); 99 TechUser (single); 100 TechMaster (single); 114 Juza (single) |
| `$44` FOI | single | 99 TechUser (single); 100 TechMaster (single); 114 Juza (single) |
| `$6D` ROUND EYES | single | 147 Rappy (single) |
| `$6E` LOVEL EYES | single | 148 BlueRappy (single) |

26 pairs, 11 abilities.
`$11` POISON and the other `status/stat effect` abilities are outside this table;
ACIDBREATH (`$33`) is implemented and §4 lists only unsupported rows, so it does
not appear above even though FlattrPlnt is a Motavia enemy.

### Single rows re-read at the resolver gate (2026-09-24)

Every `single` pair above was read again from its own citations while the
resolver's table was built — the `EnemyAttackOffs` entry, the arm, the object
chain, exactly one guarded `move.w #$C` and the target loaded from `$38` — and
none of them disagreed with this survey: all 21 pairs are in
`DAMAGE_SKILL_ROUTES` (`rust/psiv-core/src/battle/enemy_damage.rs`), and none was
dropped. The two tgt-9 rows (`$37` SAND STORM for 81 DesrtLeach, `$39` MAELSTROM
for 82 Leviathan) are what settled the gate: the nibble picks the effect
handler's range, effect `$01` is `AbilityEffect_None` (`ps4.asm:9092`) so the
nibble multiplies nothing, and each chain still makes exactly one request in
`loc_24B64`. The resolver now requires the record's effect byte to be `$01`
instead of the old `byte 2 == 8` check; `SOURCE_NOTES.md` has the change's own
section.

Two conventions to read this file's citations with, both checked against the
file while the resolver's table was built. A note of the form "`bne.s loc_X` at
line N" names the line of the *test* (`cmpi.w #$ID, $24(a4)` or
`tst.w $24(a4)`), and the branch is the line after it. And §1's row for
`Effect_SetupSkillParams` now points byte 3 at line 9602
(`move.b $3(a0,d0.w), d4`), byte 4 at line 9604
(`move.b $4(a0,d0.w), d2`) and byte 5 at line 9597
(`move.b $5(a0,d0.w), d3`): the earlier 9597/9598/9594 named the element
lookup and the resistance test instead of those three reads. The resolver's
table cites label lines and the request lines themselves.

## 4. Implemented abilities outside the damage universe

For completeness, the other four implemented abilities' regular-list pairs; none
of them is a `damage` row in §2.

| enemy | ability | arm | object chain | damage request | target | class |
|---|---|---|---|---|---|---|
| 50 FloatMine2 | `$07` FISSION | `EnemyAttack_FloatMine` (`ps4.asm:22675`) — no arm for `$07`: the routine tests `$14`, `$18`, `$19` and `$1A`, and `$07` lands in the fall-through `loc_10406` (line 22781) | — | none | — | `other` — the fall-through clears `$24(a4)`, loads no object and ends the turn, so the regular roll of FISSION2 produces no request; `resolve_fission` is only reached through `fission_neighbor` (enemies 12/13). |
| 31 CarrionCr | `$10` THREAD | `loc_10836` (`ps4.asm:23107`) — `beq.s loc_1084A` at line 23108 | `$134` `BattleObj_Thread` (`ps4.asm:36186`) | none | — | `other` — no `move.w #$C` request and no flinch write in the chain; `resolve_thread` applies the AGI debuff through the effect pipeline. |
| 32 Caterpillr | `$11` POISON | `loc_10836` (`ps4.asm:23107`) — else arm, taken when the routine's tested ids do not match (test branch at line 23109) | `$138` `BattleObj_Poison` (`ps4.asm:36279`) | none | — | `other` — no `move.w #$C` request in the chain; `resolve_poison` sets the poisoned bit through the effect pipeline. |
| 152 Zio3 | `$70` BLACK WAVE | `EnemyAttack_Zio3` (`ps4.asm:19410`) — the phase counter `$FFFFEE98` rewrites `$24(a4)` before any dispatch; `$70` is written in phase 4 | `BattleObj_BlackWave1` (`ps4.asm:67019`) | none | — | `other` — scripted sequence: phase 0 `$6B` + `BattleObj_MagBarrir` (`ps4.asm:67319`), phase 1 clears the ability + `BattleObj_NightmarePart1`, phase 2 ends the turn with no object, phase 3 `$53` + `BattleObj_NightmarePart2`, phase 4 `$70` + `BattleObj_BlackWave1` (`ps4.asm:67019`), which ends the turn without calling `GetEnemySkillEffectAndRange` — so effect `$2C` (out of range) is never dispatched. `engine::roll_enemy_ability` implements the sequence. |

## 5. Method and re-check

### Data read

| source | what was taken from it |
|---|---|
| `generated/enemies.json` | 153 records: `ai.regular_ability_ids` (record bytes 36..43) and `symbol` |
| `generated/enemy_skills.json` | 112 eight-byte records: `effect_id`, `relevant_stat`, `target_id`, `power_or_hit_chance`, `resistance_stat`, `element` |
| `reference/ps4disasm/ps4.asm` | the arms, the objects, the tails, `EnemyAttackOffs` (`ps4.asm:19206`) and the ten `BattleObjsGroup*Ptrs` tables |
| `reference/ps4disasm/ps4.constants.asm` | object and fighter field offsets, `Obj_Fighters`, `EnemySkillID_*` values |
| `docs/ENEMY_ABILITIES.md` | §2 classes and carrier lists, §4 map lists |

### How each column was produced

- **Arm.** The dispatch of each `EnemyAttack_*` routine is a chain of
  `cmpi.w #$ID, $24(a4)` / `tst.w $24(a4)` / `tst.w ability(a4)` tests, each
  followed by a branch to the next test. The chain was followed mechanically
  (`loc_*`, `+`/`-` and `.local` labels, and `#EnemySkillID_*` values resolved
  through `ps4.constants.asm`), and each test's arm taken as the path the branch
  does *not* send away: a `bne`-style test owns the code up to its target, a `beq`
  test owns the body at its target. Abilities a routine never tests land in its
  final else arm; those rows say so.
- **Object chain.** Every `move.w #$ID, (a1)` in the arm, resolved through the ten
  object tables. Objects load children the same way; the chain follows them.
- **Damage requests.** A control-flow walk from each object of the chain —
  branches, `dbf` loops, `jsr`/`bsr`, `jmp`, jump tables reached through
  `jmp LABEL(pc,dN.w)` and `lea LABEL(pc)` + `trap #2` — collecting every
  `move.w #$C, $2(aX)` with `aX` not `a4`. Each request is cited by line; the
  chain of one `$33` pair is the exact shape `resolve_acid_breath` implements.
- **Target selection.** The write's target register is resolved backwards to the
  instruction that filled it: `$38(a4)` is the chosen party target, a loop from
  `Obj_Fighters` (or `#$FF4400`) is every party slot. A request whose register
  could not be resolved is marked `unverified` in the target column — there are
  none in this revision.
- **Class.** `single` = exactly one `#$C` request, against `$38(a4)`;
  `all-party` = every `#$C` request sits in a five-slot party loop; `multi-hit` =
  more than one request of mixed shape; `other` = no `#$C` request (the note names
  what the chain does instead). Anything the walk could not establish would be
  `unverified`.

### Completeness, checked mechanically

1. Universe: the union of `ai.regular_ability_ids` over the 153 enemies, with
   zeros removed, restricted to the ids `docs/ENEMY_ABILITIES.md` §2 classes
   `damage` — 57 ids — plus `$33` ACIDBREATH.
2. One row per (enemy id, ability id): the regular list holds eight *slots* and
   repeats an id when the enemy has only one ability (Helex lists `$02` eight
   times), so the list was deduplicated per enemy before counting. 142 rows for the
   57 `damage` ids + 4 ACID BREATH rows = **146 rows**, each (enemy, ability) key
   appearing once (checked as a set of keys).
3. Carrier counts cross-checked against §2: for each of the 57 ids, the number of
   distinct carriers derived from `enemies.json` equals the number of enemies §2
   lists, and the §2 totals match (142 rows on both sides, 0 mismatches).
4. The four ACID BREATH rows are the four carriers `resolve_acid_breath`'s
   `ACID_BREATH_CARRIERS` covers (75, 76, 85, 86), and every one of them is
   `single`.

### Citations, checked mechanically

Every `ps4.asm:N` citation in this file was checked to point at a line where the
named label begins. The check is a script: collect each `ps4.asm:N` and its
backticked label, resolve the label in the disassembly, and compare. Result: all
citations verified, no mismatches (see the run note below). Line-level references
(the request lines, the mask lines) are written as "line N" without the
`ps4.asm:` prefix, exactly as `ENEMY_ABILITIES.md` §1 does, so they are statements
about a line, not label citations.

### Run note

The checks above ran from scripts kept out of the repository (`build/lane-scratch/`;
`build/` is ignored): `report2.py` (arm/object/damage extraction), `check_pairs.py`
(completeness, carrier sets, the Acid Breath class), `check_citations.py` (label
citations) and `check_other.py` (a second, whole-region scan for every `other`
row). Results at this revision:

- pair set: 146 rows, 146 distinct keys, 0 missing and 0 extra against
  `enemies.json`;
- carrier re-derivation: 57 ids, 0 mismatches against `ENEMY_ABILITIES.md` §2;
- citations: 0 mismatches over every `ps4.asm:N` / `ps4.constants.asm:N` citation;
- `other` rows: 0 `move.w #$C, $2(aX)` writes in the whole code region of the
  objects concerned;
- Acid Breath: all four pairs `single`.

### Limits

- Static reading of the disassembly, not a hardware capture: a `single` row says
  one request is made against the chosen target, not what number it produces or
  when in the animation.
- The walk follows direct control flow. A request reached only through a function
  pointer installed in an object (`move.l #loc_*, $C(a4)` then `jsr (a0)`) would
  be missed; no `other` row in this file is explained by that pattern, and every
  `other` row was cross-read against the object's whole code region (label to the
  next object label) as a second check, which found no `#$C` write either.
- The class verdicts for the four implemented rows in §4 come from the same
  reading but are not what those resolvers implement, which is why they are kept
  out of the main table.
