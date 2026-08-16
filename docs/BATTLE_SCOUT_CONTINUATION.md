# Battle system: scouting notes continuation

This continuation holds the worked numerical example and the sizing/scope
notes that were split out of `BATTLE_SCOUT.md` when the enemy-art extraction
was added. Section 13 remains in the main scouting note because it is the
presentation boundary used by the current slice.

## 12. Worked example — one complete basic attack, real numbers

**Setup.** New game, Chaz alone (from `game_start.json` / `characters.json`
id 0): level 1, HP 25, STR 8, MEN 6, AGI 7, DEX 5. Equipment record bytes
`02 02 05 04` = Hunt-Knife in **both** hands, Leather Helm, Leather Cloth.

Derived by `UpdateCharModStats` (retail `$05F754`; ps4.asm:127814):
- `atk_pow` = STR 8 + Σ(item `$B` strength + item `$F` attack) = 8 + (5+5+0+0) = **18**
- `dfs_pow` = AGI 7 + Σ(item `$D` agility + item `$10` defence) = 7 + (0+0+1+2) = **10**

Opponent: **MonsterFly** (enemy id 1, in formation 0 of block 1 — the very
first record): HP 20, STR 4, AGI 12, DEX 8, attack 14, defence 0, magic-defence
0, `physical` property **2** (normal), attack element 1 (physical), attack
status effect `$1B` (poison).

Formation 0 header (`raw_hex 10 00 08 80 …`): ambush chance `$10`, run chance
`$00`, drop rate `$08`, dropped item `$80` = Antidote.

### Battle start

`loc_B62A`: `d1` = 7 (Chaz's agility, the only living party member), `d2` = `$10`,
scale 2, thresholds `$C` / `$74`.
`v = (r + 7 − 16) * 2 = (r − 9) * 2`, `r ∈ 0..63` ⇒ `v ∈ [−18, 108]`.
- ambush iff `v ≤ 12` ⇒ `r ≤ 15` — **25%**
- preemptive iff `v > 116` — needs `r > 67`, **impossible**
- normal otherwise — 75%

A level-1 party can be ambushed but can never get a preemptive strike here.

### Turn order

Two entries: Chaz `(id 1, agi 7)`, MonsterFly `(id 6, agi 12)`. Max agility 12,
so `d1 = 6`, and each gets `+ (rng mod 6)`. Chaz goes first iff
`7 + a > 12 + b` with `a, b ∈ 0..5` — 15 of 36 equally-likely pairs, ≈42%.

### Chaz attacks (command 1)

**Hit roll**, `loc_B716`: `d1 = 5` (dexterity_battle), `d2 = 12`
(agility_battle), scale 2, `$8` / `$74`.
`v = (r + 5 − 12) * 2 = (r − 7) * 2`.
- miss iff `v ≤ 8` ⇒ `r ≤ 11` — **12/64 = 18.75%**
- crit iff `v > 116` ⇒ `r > 65` — **impossible** (DEX 5 vs AGI 12)
- normal — 81.25%

**Damage**, `Character_DamageEnemy` → `Battle_CalculateDamage`:
- `d1` = `atk_pow_battle` = **18**
- `d2` = MonsterFly `dfs_pow_battle` = **0**
- `d3` = element factor: Hunt-Knife byte `$12` = element 1 (physical) →
  `1*2 + $2E = $30` = MonsterFly's `physical_prop` = **2**
- `d4` = 0 (no crit possible here; a crit would give `18 >> 2 = 4`)

```
S = 56 (mean):  (56+8)*18 = 1152 ; >>6 = 18 ; +18 = 36 ; *2 = 72 ; >>2 = 18 ; -0 = 18
S = 0  (min):   (0+8)*18  =  144 ; >>6 =  2 ; +18 = 20 ; *2 = 40 ; >>2 = 10 ; -0 = 10
S = 112(max):   (112+8)*18= 2160 ; >>6 = 33 ; +18 = 51 ; *2 =102 ; >>2 = 25 ; -0 = 25
```

**Damage range 10–25, mean 18** against 20 HP: Chaz two-shots a MonsterFly, and
one-shots it on a high roll. `FighterShowDamage_DecreaseHP` then does
`sub.w 18, curr_hp` → 20 − 18 = 2, still positive, fighter routine 4.

### MonsterFly attacks back

**Hit roll**: `d1 = 8` (its dexterity), `d2 = 7` (Chaz's agility).
`v = (r + 1) * 2` ⇒ miss iff `r ≤ 3` (**6.25%**), crit iff `r ≥ 58`
(**6/64 ≈ 9.4%**).

**Damage**, `Enemy_DamageCharacter`: `d1` = 14, `d2` = 10, `d3` = Chaz's
`physical_prop` = 2, `d4` = 0 or `14 >> 2 = 3`.

```
S = 56:  (64*14)=896  ; >>6 = 14 ; +14 = 28 ; *2 = 56 ; >>2 = 14 ; -10 =  4
S = 0:   (8*14) =112  ; >>6 =  1 ; +14 = 15 ; *2 = 30 ; >>2 =  7 ; -10 = -3 -> clamped to 1
S = 112: (120*14)=1680; >>6 = 26 ; +14 = 40 ; *2 = 80 ; >>2 = 20 ; -10 = 10
critical (S=56): 14+14 = 28 ; +2*3 = 34 ; *2 = 68 ; >>2 = 17 ; -10 = 7
```

**Damage range 1–10 normal, mean 4; a mean-roll critical deals 7**, against
25 HP. (Correction 2026-08-15: an earlier revision of the critical line
mis-ordered the pipeline and concluded 1 — the doubled bonus survives the
element multiply and is worth a real 3 damage here.)

### The poison rider

MonsterFly's record byte `$14` (`max_tp` reused as "attack status effect") is
`$1B` = poison. `Battle_DoAttackEffect` (`ps4.asm:8553`) picks it up, dispatches
`AbilityEffect_Poison`, which sets `d3 = $48` (`efess_prop`) and rolls through
`Effect_DoPhysicalAttack`/`loc_6640`: `d1` = enemy `strength_battle` 4, `d2` =
Chaz `strength_battle` 8, `d3` = Chaz's `efess_prop` = 2, `d4` = `$70`.

`v = (r + 4 − 8) * 2 = (r − 4) * 2`; poison lands iff `v > 112` ⇒ `r > 60`
⇒ `r ∈ {61,62,63}` — **3/64 ≈ 4.7%** per connecting hit.

### Escape

Formation 0's run chance is `$00`, so escape is `v = (r + 7 − 0) * 2 > $28`
⇒ `r > 13` — **78%**.

### Victory

MonsterFly gives 27 experience and 8 meseta, one living party member, so Chaz
gets all 27 and all 8. Item drop: rate 8, so `(roll & $7F) < 8` — **8/128 =
6.25%** chance of an Antidote.

## 14. Sizing

| piece | size | nature |
|---|---|---|
| turn engine (`BattleRoutines`) | 18 entries, ~700 lines | code, shallow |
| UI machine (`BattleRoutines2`) | 67 entries ($01–$43), ~6,000 lines | pure presentation, replaceable |
| damage math | 3 routines, 60 lines total | code — transcribe exactly |
| stat/element selection | 2 tables, 8 + 16 bytes | data |
| ability effects | 44-entry table, 21 distinct routines, <300 lines | code, formulaic |
| effect logic dispatch | 5 entries (`EffectLogicOffs`) | code |
| range/targeting | 10-entry table + 5 routines | data + trivial code |
| enemy AI conditions | 20 entries, ~40 lines each | code, formulaic |
| enemy attack animations | 154 entries → 74 routines | presentation |
| battle objects | 574 `BattleObj_*` | presentation |
| macro smart-targeting | 20-entry table + 7 routines | data + trivial code |

**Data already extracted and sufficient**: 153 enemies (stats, 14 element
properties, AI lists, XP/meseta), 112 enemy skills, 40 techniques, 54 skills,
15 combos, 160 items, 504 + 27 formations, 11 × 98 level records, encounter
group bindings. **Nothing new needs extracting for a battle v1.** The one gap is
the remaining 87 static body-hole cells that are not covered by a decoded
dynamic destination; the enemy overlay slice now locates and renders the
decoded portion generically.

## 15. A proposed scope ladder for v1

Offered as a starting point for the session, not a decision.

**Tier 0 — the math kernel (headless, no rendering).**
`UpdateRNGSeed`, `Battle_CalculateChances`, `Battle_CalculateDamage`,
`Battle_CalcHealing`, the two offset tables, `UpdateCharModStats`,
`Battle_FillEnemyStats`. Pure functions over `psiv-data` records, unit-tested
against hand-computed cases like §12. This is a day's work and it is the part
that must be perfect.

**Tier 1 — a battle you can win.**
Encounter roll → formation selection → priority roll → turn order → command
input (Attack / Defend / Run) → hit roll → damage → death → XP/meseta →
level-up. Enemy AI limited to the 8-regular-ability roll with no conditions.
No statuses, no techniques, no items. Reuses the existing pack data end to end.

**Tier 2 — the full command set.**
Techniques (TP), skills (uses), items, the 44-entry effect table, the 20 AI
conditions, status effects and their per-turn processing, drops, multi-target
weapons.

**Tier 3 — the rest.**
Macros with smart targeting, combos, vehicle battles, the android auto-revive
path, boss formations and `Event_Battle_Index`.

Presentation tracks separately and can stay abstract (log lines) through
Tier 1–2.

## 16. Open questions for the design session

1. **The RNG question, and it is the big one.** Battle randomness reads the VDP
   H/V counter. Bit-exact battle replay against the cartridge is not achievable
   in a headless core. Do we (a) swap in `UpdateRNGSeed`'s LCG for battle rolls
   — same distributions, different stream; (b) model a cycle budget to
   synthesise an HV counter; or (c) declare battle a "statistically faithful,
   not stream-faithful" subsystem and put the fidelity spine on formulas
   instead? This changes what the oracle harness is for.
2. **Which retail bugs do we reproduce?** The fidelity policy says "obvious
   original bugs may be fixed". The battle candidates are unusually load-bearing:
   the level-99 table desync corrupts stats, the shield-element bug changes
   damage, the cure-restores-stats quirk is arguably a *feature* players rely on,
   and Telepipe-in-battle consumption is a straightforward player-hostile bug.
   Each needs its own verdict in the ledger, not a blanket rule.
3. **Is `Battle_Speed` in scope as a setting?** It is already a cartridge option
   and it is pure presentation — likely a free win for the enhancement pack.
4. **Enemy art boundary — resolved for this slice.** Retail static bodies use
   plane mappings with Enigma data, while `Enemy_Sprites` supplies the animated
   replacement pieces. The runtime uses textured body quads and applies those
   decoded rectangles; the remaining art question is the 87 uncovered static
   hole cells, not the source format.
5. **Where does the "damage lands during the animation" timing live?** On
   hardware, HP moves inside the presentation state machine. A headless core
   wants to resolve the whole turn atomically and emit an effect timeline. That
   is almost certainly right — but it means the oracle must compare *end-of-turn*
   RAM, not per-frame RAM, for battle.
6. **`Zio3` / `BLACK WAVE`.** Do we implement effect `$2C` (as what?), reject it
   at data-load time, or reproduce the crash? Recommend: `psiv-data` validation
   rejects effect ids outside `$00–$2B` and records the one offender as a known
   census anomaly, mirroring the encounter-anomaly treatment.
7. **Verification plan.** Tier 0 can be unit-tested with no oracle at all if we
   force seeds. Tiers 1+ need the BizHawk harness with RAM watches on
   `Battle_Turn_Order` (`$FFFFEFB0`), `Battle_Heal_Damage_List` (`$FFFF415A`),
   `Fighters_Hit_Flags` (`$FFFF4150`), `Battle_Ability_Effects` (`$FFFF4170`)
   and `Enemy_Stats` (`$FFFF4200`). Worth building those watch lists while the
   event harness is fresh.

## Appendix: verified retail addresses

| symbol | retail | how proven |
|---|---|---|
| `UpdateRNGSeed` | `$04236C` | opcode match `48E7C000 2238EF0C 4A41` |
| `UpdateRNGSeed2` | `$04239E` | opcode match `302D0008 … 9078EF0C E6F8EF0C 4E75` |
| `RNG_Seed` | `$FFFFEF0C` (long) | `ps4.constants.asm:2328` + both routines |
| `Battle_CalculateChances` | `$00B5A6` | opcode match |
| `Battle_CalculateDamage` | `$00B5CA` | opcode match; also the `jsr` at `loc_266C` |
| `Battle_CalcHealing` | `$00B5FE` | opcode match |
| `loc_266C` (clamp 1..999) | `$00266C` | `4EB9 0000B5CA` |
| `Enemy_DamageCharacter` | `$0026A0` | `302B0024 662C 7600 1628 0012` |
| `loc_275A` stat table | `$00275A` | `0000 001A 001D 0020 0023 0026 002A 002E` |
| `loc_276A` element table | `$00276A` | `00 30 32 34 36 38 3A 3C 3E 40 42 44 46 48 4A 00` |
| `Character_DamageEnemy` | `$00277A` | `302B0024 6654 08EC0000002A` (retail branch) |
| `Fighter_GetElementProp` | `$0027C2` | `4EB9 0027DDD4 D040 0640002E` |
| `Ability_GetEffectAndRange` | `$0060B8` | `4A42 670000F4 47F9 FFFF4170` |
| `AbilityEffectsOffs` | `$0061BE` | 44 words dumped and matched entry-by-entry |
| `AbilityEffect_None` | `$006216` | `4E75` immediately after the table |
| `loc_6640` (enemy phys chance) | `$006640` | `16343000 7870 7200 7400 1229001A 142C001A` |
| `loc_B6A2` (hit-flag pass) | `$00B6A2` | label + `loc_B716` neighbourhood |
| `loc_B716` (hit/crit roll) | `$00B716` | `3039FFFF4142 4EB90027DDB6 7200 1228 0023` |
| `loc_B62A` (start priority) | `$00B62A` | `7602 780C 7A74` at `$00B634` |
| `Battle_ProcessRUN` roll | `$0054AC` | `7602 7828` |
| `Battle_GetFighterStatsAddr` | `$0027DDB6` | call sites |
| `Battle_GetFighterAddr` | `$0027DDC6` | call sites |
| `Battle_LoadWpnAttackElem` | `$0027DDD4` | `Fighter_GetElementProp`'s `jsr` |
| weapon post-hit effect | `InventoryData+$13` | `loc_27DE38` |
| `RunRandomBattles` | `$05784E` | `4A38ECE7`; step counter `$057894`, mask `$0578B0` |
| `loc_2D960` (enemy death award) | `$0002D960` | `426C0000 206C0004 08E8000200 16 30280060 D179FFFF41CE` |
| enemy art descriptor table | `$0027F3BC` | `0020D850 04080012 0020D77C 0020D2C8 0020D2A6` |
| `loc_10ED4` (enemy art decode) | `$00010ED4` | `205B 302C000E` |
| `TRAP #2` dispatcher | `$000216` | vector at `$88`; `D040 D0F00000 4E90 4E73` |

Battle RAM, from `ps4.constants.asm` (all `$FFFFxxxx`): `Battle_Routine` `4100`,
`Battle_Total_Comd_Input` `4106`, `Battle_Routine_2` `4108`,
`Battle_Command_Data` `410A`, `Enemy_Command_Data` `411E`,
`Current_Turn_Number` `4140`, `Current_Actor_Index` `4142`,
`Current_Target_Index` `4144`, `Current_Command` `4146`,
`Fighters_Hit_Flags` `4150`, `Battle_Heal_Damage_List` `415A`,
`Battle_Ability_Effects` `4170`, `Battle_Ability_Range` `4182`,
`Enemy_Formation_Data` `41F0`, `Enemy_Positions` `41F7`, `Enemy_Stats` `4200`,
`Obj_Fighters` `4400`, `Fighter_Enemy_1` `4540`, `Battle_Priority` `EE45`,
`Ability_Effect_Type` `EE4A`, `Enemy_Sprites` `EA00`, `Battle_Turn_Order` `EFB0`,
`Macro_Data` `F444`, `Character_Stats` `F500`, `Random_Battles_Flag` `ECE7`,
encounter step counter `ECE4`.
