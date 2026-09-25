# Battle system: retail research

Research began 2026-08-15. This document records cartridge routines and
measurements. The battle engine is implemented; current command gaps belong
in [the roadmap](../ROADMAP.md) and [playability ledger](../campaign/NATIVE_PLAYABILITY.md).
Use [runtime architecture](../RUNTIME_DESIGN.md) for settled design decisions.

Method note: every retail address below was **verified against the cartridge**
(`Phantasy Star IV (USA).md`, sha256 `511f35cc…13b6a`) by matching opcode bytes,
not by trusting the fork. Where a `if bugfixes=1` gate exists, the retail branch
was confirmed from ROM bytes and is the one transcribed here.

**Citation convention discovered while scouting, and it matters:** in
`ps4.asm` the `loc_XXXXX` *labels* are retail addresses (`loc_266C` is at
`0x00266C`, `loc_B716` at `0x00B716`, `loc_27F3BC` at `0x0027F3BC` — all
byte-verified). The auto-generated `;0x0 (0x0000....)` *inline comments* are
the fork's build addresses and drift (this is the ~0x552 discrepancy
docs/source-notes/disassembly-discrepancies.md already records near `0x8000`). Cite labels, never the comments.

---

## 1. The shape of the system

Battle is **two nested state machines plus a per-frame object system**, all
driven from `GameMode_Battle` (`ps4.asm:947`):

```
GameMode_Battle:
    Battle_DrawCommandIcons
    RunBattleRoutines        ; the LOGIC machine   -> Battle_Routine   ($FFFF4100)
    Battle_UpdatePaletteObjs
    Battle_UpdateFighters    ; 12 fighter slots, each runs its own routine
    RunBattleRoutines2       ; the UI/MESSAGE machine -> Battle_Routine_2 ($FFFF4108)
    Battle_UpdateEnemySprites
    Battle_RunObjects        ; animation objects
```

- **`BattleRoutines`** (`ps4.asm:7524`) — **18 entries**, the turn engine:
  idle/delay, setup, command collection (`Battle_ProcessCOMD`), macro
  (`Battle_ProcessMACRO`), escape (`Battle_ProcessRUN`), turn ordering
  (`Battle_OrderTurns`), per-actor execution (`loc_576A`), damage/effect
  application (`Battle_DoAttackEffect`), end-of-round cleanup.
- **`BattleRoutines2`** (`ps4.asm:1138`) — **67 entries** (values `$01`–`$43`), entirely
  windows/menus/messages: main options, per-character command, tech/skill/item
  lists and paging, target cursors, victory/defeat/run/level-up messages, macro
  editor, vehicle menus. This is the part a modern UI replaces wholesale.
- **`Battle_UpdateFighters`** (`ps4.asm:987`) — 12 slots × `$40` bytes at
  `Obj_Fighters` (`$FFFF4400`). Slots 1–5 = characters, 6–9 = enemies
  (`cmpi.w #5, Current_Actor_Index; bgt` ⇒ enemy, used everywhere). Each slot's
  `fighter_routine` indexes a per-character/per-enemy action table.

**Data-driven vs code:** the *numbers* are almost entirely data (enemy records,
the 8-byte ability records, formations, level tables, four small offset tables).
The *sequencing* is code, but shallow — the whole turn engine is 18 routines.
The genuinely bespoke code is presentation: a 154-entry per-enemy
attack-animation table over 74 distinct routines, and 574 `BattleObj_*`
animation objects.

---

## 2. The two RNGs — and the landmine

Two generators, one seed word. **This is the single biggest fidelity risk in
the whole project.**

### `UpdateRNGSeed` — retail `$04236C` (`ps4.asm:86070`)

Called **once per V-blank** from the VInt handler (`ps4.asm:617`), and directly
by a handful of consumers (encounters, paralysis recovery, field effects).

```
    move.l  (RNG_Seed).w, d1       ; RNG_Seed = $FFFFEF0C, longword
    tst.w   d1                     ; tests the LOW word ($FFFFEF0E)
    bne.s   +
    move.l  #$2A6D365B, d1         ; reseed only if the low word is zero
+   move.l  d1, d0                 ; d1 = d1 * 41:
    add.l   d1, d1                 ;   x2
    add.l   d1, d1                 ;   x4
    add.l   d0, d1                 ;   x5
    asl.l   #3, d1                 ;   x40
    add.l   d0, d1                 ;   x41
    move.w  d1, d0                 ; d0.w = lo(41x)
    swap    d1
    add.w   d1, d0                 ; d0.w = lo(41x) + hi(41x)
    move.w  d0, d1                 ; d1 = lo(41x):sum  ->
    swap    d1                     ;   d1 = sum:lo(41x)
    move.l  d1, (RNG_Seed).w
```

New seed long = `(lo(41x)+hi(41x)) << 16 | lo(41x)`. (Correction 2026-08-15,
verified during the kernel port: after `swap d1` the register holds `lo:hi`,
so the `move.w` overwrites the *hi* word with the sum and the final `swap`
leaves `lo` in the low half. An earlier revision of this doc said `| hi`.)
The routine brackets itself with `movem.l d0-d1/...(sp)+` and returns nothing
in a register — consumers read the word at `(RNG_Seed).w` = the high half =
the sum. Deterministic, portable, trivially reproducible. Reseed constant
`$2A6D365B` fires when the *low* word hits zero.

### `UpdateRNGSeed2` — retail `$04239E` (`ps4.asm:86097`)

**Every battle roll goes through this.** 43 call sites.

```
    move.w  $8(a5), d0             ; a5 = VDP_Data_Port ($C00000)
                                   ; $8(a5) = $C00008 = the VDP H/V COUNTER
    add.w   (Main_Frame_Count).w, d0
    sub.w   (RNG_Seed).w, d0       ; word at $FFFFEF0C
    ror     (RNG_Seed).w           ; rotate that word right by 1
    rts
```

`d0 = HVcounter + frame_count − seedword`, then the seed word rotates one bit.

`VDP_Counter = $C00008` (`ps4.constants.asm:1953`) and VDP register `$8004`
("HInt off, **enable HV counter read**", `ps4.asm:276`) confirm the port. `a5`
is the game's global VDP-data-port register, set at `ps4.asm:358/561` and
restored at `ps4.asm:4087` after the battle item menu borrows it.

**Consequence:** battle randomness depends on the raster beam position at the
instant of the call — i.e. on how many CPU cycles have elapsed since the start
of the frame. A headless reimplementation cannot reproduce that without
cycle-accurate emulation of the whole frame. There is no second seed variable;
there is a second *generator*, and its entropy is the hardware.

There is also a **second-order effect**: `Battle_CalculateDamage` calls this 16
times in a tight loop, so the HV counter advances a few pixels between draws.
The 16 samples are not independent of each other in a way any software model
reproduces.

Design implication (for the session, not a decision): bit-exact battle RNG
against the cartridge is off the table for a native runtime. The realistic
options are (a) substitute `UpdateRNGSeed`'s LCG for battle rolls too, keeping
the *distributions* exact and the *sequences* different, or (b) keep an
HV-counter surrogate driven by a modelled cycle budget and accept "statistically
identical, not stream-identical". The oracle harness can still verify the
*formulas* by forcing seeds through RAM writes.

**Encounter rolls are safe** — they use `UpdateRNGSeed`, the portable one.

---

## 3. Turn order — `Battle_OrderTurns`, `ps4.asm:7723`

`Battle_Turn_Order` at `$FFFFEFB0`, 9 entries × 4 bytes = `[fighter id .w][agility .w]`.

1. Clear bit 7 of every fighter's `status`.
2. Zero the whole queue.
3. Enqueue: by `Battle_Priority` — preemptive ⇒ characters only (5 slots),
   ambush ⇒ enemies only (4 slots), normal ⇒ all 9. Skip anyone whose `status`
   has paralyzed/dead/asleep/asleep2/android-dead.
   Entry = `(id, agility_battle)`.
4. **Random jitter** (`ps4.asm:7790-7804`):

```
    move.w  (a1), d1              ; d1 = max agility in the queue (scan above)
    lsr.w   #1, d1                ; d1 = max_agility / 2
-   clr.w   d0
    swap    d0                    ; guarantee d0's high word is 0 for divu
    jsr     (UpdateRNGSeed2).l    ; d0.w = raw roll, d0 = 0000rrrr
    divu.w  d1, d0                ; 32/16 divide
    swap    d0                    ; d0.w = REMAINDER
    tst.w   (a1)
    beq.s   +                     ; agility 0 gets no bonus
    add.w   d0, (a1)              ; agility += rng mod (max_agility/2)
+   addq.w  #4, a1
    dbf     d7, -
```

   So the sort key is `agility + (rng mod (max_agility/2))`. Note the
   divide-by-zero hazard if every combatant has agility 0 — unreachable in
   retail data, but a port must decide what to do.
5. **Bubble sort descending**, 8 outer × 8 inner passes, comparing the agility
   word (`ps4.asm:7807-7819`). Stable-ish only by accident; a port must
   reproduce the exact swap order, not just "sort descending".
6. **Macro re-ordering** (`ps4.asm:7821-7926`): if the party chose MACR, the
   queue is permuted so party members act in macro order — a rotate-left of the
   sub-range between the two positions, not a swap.
7. **Enemy commands are chosen here**, not at execution time: four
   `Enemy_TargetCharacter` calls fill `Enemy_Command_Data` (`$FFFF411E`).

### Enemy target selection — `Enemy_TargetCharacter`, `ps4.asm:7978`

```
    jsr     (UpdateRNGSeed2).l
    ; a0 -> living-character list; ($FFFF414A) = count of living party members
    ; rate table indexed by (count - 2):
EnemyTargetRates:
  2 chars: $54
  3 chars: $80, $2C
  4 chars: $99, $4D, $1A
  5 chars: $AC, $66, $33, $12
-   cmp.b   (a1)+, d0             ; byte compare of the roll
    bcc.s   +                     ; roll >= rate  -> target this slot
    addq.w  #1, a0                ; else next candidate
    dbf     d1, -
+   rts                           ; fall-through targets the last living member
```

With 5 living members the front slot is picked when `roll ≥ $AC` (≈33%), and the
probabilities are front-loaded — the party leader is the most likely target.
The living-member list is rebuilt by `loc_56F0` (`ps4.asm:7952`), which skips
dead/android-dead only (paralyzed and asleep characters are still targetable).

---

## 4. The damage pipeline

Entry point per damaged fighter: `Fighter_TakeDamage` (`ps4.asm:3564`) runs on
the *target* object `a4`, derives the slot index `d6 = (a4 − $4400) >> 6`, checks
`Fighters_Hit_Flags[d6]` (`$FFFF4150`; `$FF` = miss/untargeted ⇒ no damage), then
calls `Figher_DamageCheckActor` (`ps4.asm:3737`, the routine immediately before
`loc_266C`) which splits actor-is-character vs actor-is-enemy.

### 4.1 The four inputs

Every damaging action funnels into `Battle_CalculateDamage` with:

| reg | meaning | where it comes from |
|---|---|---|
| `d1` | attack / mental power | actor stat, see below |
| `d2` | defence power | target stat, see below |
| `d3` | element factor (0–4) | target's element property byte |
| `d4` | flat bonus | crit bonus, or the ability's power byte |

Two tiny offset tables do all the stat selection. Both verified in ROM.

**`loc_275A` @ `$00275A`** — "which stat" table (index = record byte 1 & `$7F`,
or record byte 4 for the defence side):

```
0000 001A 001D 0020 0023 0026 002A 002E
 0    1    2    3    4    5    6    7
none str  men  agi  dex  atk  def  mdef
```
Offsets `< $26` are read with `move.b`, `>= $26` with `move.w` — that is the
whole reason for the `cmpi.w #$26` you see at four call sites.

**`loc_276A` @ `$00276A`** — "which element resistance" table (index = record
byte 5):

```
00 30 32 34 36 38 3A 3C 3E 40 42 44 46 48 4A 00
 0  1  2  3  4  5  6  7  8  9  A  B  C  D  E
   phys en fire grav wat anti elec holy bros bio psy mech efess destroy
```
Element ids `>= $10` mean "physical skill — use the weapon's element instead"
(`cmpi.b #$10, d3` at `ps4.asm:3817`). Element properties are stored as a word
whose **both bytes hold the same value** (see `Battle_FillEnemyStats`,
`ps4.asm:11968-11995`), and the code reads the high byte, so the factor is the
raw 0–4 from the data. Retail data uses only {0,1,2,3,4}: 0 = immune,
1 = resistant, 2 = normal, 3 = weak, 4 = very weak.

### 4.2 Character basic attack — `Character_DamageEnemy`, retail `$00277A`

Retail (**no** shield check — that is the `bugfixes=1` branch, absent from the
cartridge; byte-verified at `$277A`):

```
    move.w  ability(a3), d0        ; nonzero -> it's a tech/skill/item, go elsewhere
    bne.s   loc_27D4
    bset    #0, $2A(a4)            ; mark target "was attacked physically" (AI hook)
    move.b  right_hand(a0), d0
    beq.s   loc_2794
    bsr.s   Fighter_GetElementProp ; $0027C2
    move.w  d0, d3
loc_2794:
    move.b  left_hand(a0), d0
    beq.s   loc_279E
    bsr.s   Fighter_GetElementProp
loc_279E:
    cmp.w   d0, d3
    bge.s   loc_27A4
    move.w  d0, d3                 ; keep the LARGER of the two hands' factors
loc_27A4:
    move.w  atk_pow_battle(a0), d1 ; d1 = attack
    move.w  dfs_pow_battle(a1), d2 ; d2 = target defence
    moveq   #0, d4
    lea     (Fighters_Hit_Flags).l, a0
    tst.b   (a0,d6.w)
    ble.s   loc_27BE
    move.b  d1, d4
    lsr.w   #2, d4                 ; CRITICAL: bonus = attack / 4
loc_27BE:
    bra.w   loc_266C
```

`Fighter_GetElementProp` (`$0027C2`) = `Battle_LoadWpnAttackElem` (item byte
`$12`) → `elem*2 + $2E` → byte read from the *target's* stats. Note the
retail bug this branch encodes: a **shield's** element is read as if it were a
weapon's, so equipping a shield can change your other hand's damage.

### 4.3 Enemy basic attack — `Enemy_DamageCharacter`, retail `$0026A0`

```
    move.w  $24(a3), d0            ; enemy's chosen ability; 0 = plain attack
    bne.s   loc_26D2
    move.b  $12(a0), d3            ; enemy curr_tp field = its attack ELEMENT
    add.w   d3, d3
    addi.w  #$2E, d3
    move.b  (a1,d3.w), d3          ; target's resistance to that element
    move.w  $26(a0), d1            ; enemy atk_pow_battle
    move.w  $2A(a1), d2            ; character dfs_pow_battle
    moveq   #0, d4
    lea     ($FFFF4150).l, a0
    tst.b   (a0,d6.w)
    ble.s   loc_26D0
    move.b  d1, d4
    lsr.w   #2, d4                 ; crit bonus = (attack & $FF) / 4: byte move into a cleared d4
loc_26D0:
    bra.s   loc_266C
```

### 4.4 Ability (skill / enemy skill / vehicle skill) — `loc_26DA`, `$0026DA`

```
    move.b  $1(a2,d0.w), d1        ; record byte 1
    andi.w  #$7F, d1               ;   low 7 bits = which ACTOR stat is the power
    add.w   d1, d1
    move.w  loc_275A(pc,d1.w), d1
    cmpi.w  #$26, d1
    bge.s   loc_26F4
    move.b  (a0,d1.w), d1          ; byte stat
    bra.s   loc_26F8
loc_26F4:
    move.w  (a0,d1.w), d1          ; word stat
loc_26F8:
    move.b  $5(a2,d0.w), d3        ; record byte 5 = element
    cmpi.b  #$10, d3
    blt.s   loc_272C               ; ordinary element -> table lookup
    ; else physical skill: take the max of the two hands' weapon elements
    ; (retail applies shields here too — same bug as 4.2)
loc_272C:
    move.b  loc_276A(pc,d3.w), d3
    move.b  (a1,d3.w), d3          ; target's resistance byte
loc_2734:
    move.b  $4(a2,d0.w), d2        ; record byte 4 = which TARGET stat resists
    add.w   d2, d2
    move.w  loc_275A(pc,d2.w), d2
    cmpi.w  #$26, d2
    bge.s   loc_274C
    move.b  (a1,d2.w), d2
    bra.s   loc_2750
loc_274C:
    move.w  (a1,d2.w), d2
loc_2750:
    moveq   #0, d4
    move.b  $3(a2,d0.w), d4        ; record byte 3 = POWER
    bra.w   loc_26D0
```

**Techniques take a different door** (`loc_281E`, `ps4.asm:4024`): they skip the
byte-1 lookup entirely and hard-wire `d1 = mental_battle(actor)`. Byte 1 of a
technique record is its **TP cost**, consumed separately by
`CharTech_CheckTPCost` (`ps4.asm:14197`, `lea (TechniqueData-7)` — i.e. byte 1).
Items use `loc_2842`: `d1 = item byte 1` (the item's own power).

This exactly matches the field names the extractor already emits
(`relevant_stat`, `power_or_hit_chance`, `resistance_stat`, `element`,
`tp_cost`) — no re-extraction needed.

### 4.5 `Battle_CalculateDamage` — retail **`$00B5CA`** (`ps4.asm:17374`)

Byte-verified: `48e7 7f00 7e0f 7c00 4eb9 0004239e 0240 0007 dc40 51cf fff2 5046
cdc1 ec4e dc41 d844 dc44 cdc3 e44e 9c42 3006 4cdf 00fe 4e75`.

```
    movem.l d1-d7, -(sp)
    moveq   #$F, d7
    moveq   #0, d6
-   jsr     (UpdateRNGSeed2).l
    andi.w  #7, d0
    add.w   d0, d6                 ; 16 draws of 0..7  ->  S in [0,112]
    dbf     d7, -
    addq.w  #8, d6                 ; d6 = S + 8
    muls.w  d1, d6                 ; * attack power       (16x16 -> 32)
    lsr.w   #6, d6                 ; / 64                 (LOW WORD ONLY)
    add.w   d1, d6                 ; + attack power
    add.w   d4, d4
    add.w   d4, d6                 ; + 2 * bonus
    muls.w  d3, d6                 ; * element factor
    lsr.w   #2, d6                 ; / 4                  (LOW WORD ONLY)
    sub.w   d2, d6                 ; - defence
    move.w  d6, d0
```

As integer math, with `&` meaning 16-bit truncation:

```
S      = sum of 16 uniform draws from 0..7          (0..112, mean 56)
t      = ((S + 8) * ATK) & 0xFFFF
t      = t >> 6                                      (logical, on the 16-bit word)
t      = (t + ATK + 2*BONUS) & 0xFFFF
t      = (t * ELEM) & 0xFFFF
t      = t >> 2                                      (logical)
damage = (int16)(t - DEF)
```

Then `loc_266C` (`$00266C`) clamps: **`damage < 1 → 1`, `damage > 999 → 999`**,
and stores it in `Battle_Heal_Damage_List` (`$FFFF415A`, one word per fighter).

Two things a port must not "clean up":

- The `lsr.w` after `muls.w` throws away the product's high word. Overflow
  starts at `(S+8)*ATK ≥ 65536`, i.e. `ATK > 546` at a maximum roll. Retail's
  realistic ceiling is ~235 (99 base strength + a 127-attack two-hander + head
  and body), so it is unreachable in practice — but the shifts are logical, not
  arithmetic, and that *does* matter once `t - DEF` goes negative.
- **Element factor 0 ("immune") still deals 1 damage**, because the minimum
  clamp runs after the multiply. Immunity in PSIV is 1 HP per hit, not zero.

`Battle_CalcHealing` — retail **`$00B5FE`** (`ps4.asm:17411`) is the same shape,
minus defence and with a final halving:

```
heal = ((((S + 8) * MEN) >> 6) + MEN + 2*POWER) >> 1
```

Callers at `ps4.asm:4633`; `d2` = `mental_battle` for techniques, the byte-1 stat
for skills, item byte 1 for items; `d3` = record byte 3. Result is clamped to
`max_hp`, or to `max_tp` for the single hardcoded exception: **Ataraxia**
(`cmpi.w #$32E, ($FFFF4146)` — command 3, skill `$2E` — restores TP, `ps4.asm:4639`).

### 4.6 HP subtraction

`FighterShowDamage_DecreaseHP` (`ps4.asm:3640`):

```
    move.w  (a1,d0.w), d1          ; damage from Battle_Heal_Damage_List
    sub.w   d1, curr_hp(a0)
    tst.w   curr_hp(a0)
    bgt.s   +
    move.w  #7, fighter_routine(a4)  ; => Character_Dead / Enemy_Dead
```

HP is **not clamped to zero** here; the death routine takes over. Note this is
in the *presentation* half — the number is computed a frame or more before the
HP actually moves.

---

## 5. Hit, miss, critical — `Battle_CalculateChances`, retail `$00B5A6`

```
    jsr     (UpdateRNGSeed2).l
    andi.w  #$3F, d0               ; r in 0..63
    sub.w   d2, d1                 ; actor param - target param
    add.w   d1, d0
    muls.w  d3, d0                 ; * scale
    cmp.w   d4, d0
    ble.s   -> return -1           ; MISS / failure
    cmp.w   d5, d0
    ble.s   -> return  0           ; normal
    -> return 1                    ; CRITICAL / surprise
```

`v = ((r + actor − target) * scale)` compared as a signed 16-bit word against
two thresholds. Five call sites, each with its own constants:

| what | `d1` (actor) | `d2` (target) | `d3` | `d4` (miss ≤) | `d5` (crit >) | site |
|---|---|---|---|---|---|---|
| physical hit/crit | actor `dexterity_battle` | target `agility_battle` | 2 | 8 | `$74` | `loc_B716` |
| battle-start priority | highest party agility | formation byte 0 | 2 | `$C` | `$74` | `loc_B62A` |
| escape | highest party agility | formation byte 1 | 2 | `$28` | *uninitialised* | `Battle_ProcessRUN` |
| status effect (physical) | actor `strength_battle` | target `strength_battle` | target's element prop | `$70` | effect id | `Effect_DoPhysicalAttack`/`loc_6640` |
| status effect (ability) | record byte-1 stat | record byte-4 stat | target element prop | record byte 3 | effect id | `Effect_SetupSkillParams` |

Notes:

- **The escape roll reads `d5` uninitialised.** Harmless — the caller only tests
  the sign, and both 0 and 1 are non-negative — but a faithful port should
  record why it's harmless rather than silently pick a value.
- **In the status-effect paths `d5` is the effect id**, so the "critical"
  distinction is meaningless there; only miss-vs-hit is used.
- `loc_B6A2` (`$00B6A2`) is the pass that fills `Fighters_Hit_Flags`: it presets
  all nine to `$FF`, then rolls `loc_B716` per live target. Dead/absent targets
  stay `$FF`. **Abilities skip the roll entirely** — `tst.w $24(a4); bne
  loc_B75A` sends techs/skills/items to `loc_B782`, which writes `0` (normal
  hit, no miss possible). Only basic attacks can miss.
- **Multi-target attacks cannot crit.** `smi ($FFFFEE49).w` records
  "target index was negative"; `loc_B754` demotes a rolled `1` to `0` when set.

With the physical constants the reachable band is:
`miss ⟺ r + DEX − AGI ≤ 4`, `crit ⟺ r + DEX − AGI > 58`. Since `r ≤ 63`, a
critical requires `DEX − AGI > −5`: an attacker whose dexterity is 5+ below the
target's agility **can never land a critical hit**.

---

## 6. Statuses and the effect table

Status lives in one byte, `status = $16` of the stats struct:

```
bit 0 poisoned   bit 1 paralyzed   bit 2 dead   bit 3 asleep
bit 4 tech-sealed  bit 5 asleep2   bit 6 android-dead   bit 7 (transient msg flag)
```

Effect dispatch is a **44-entry table, `AbilityEffectsOffs`, retail `$0061BE`**
(`ps4.asm:9036`), indexed by record byte 0. Verified word-for-word from the
cartridge. Retail contents:

- `$00,$01,$05,$10,$11,$17–$1A,$1E–$20,$22–$25,$28–$2A` → `AbilityEffect_None`
  (**20 of 44 slots do nothing**).
- `$02` Death · `$03` AttackDown · `$04` DefenseDown · `$06` AgilityDown ·
  `$07` SleepParalyze · `$08` SealTech · `$09` AttackUp · `$0A` DefenseUp ·
  `$0B` MagicDefenseUp · `$0C` AgilityUp · `$0D` ElementResistanceUp ·
  `$0E` WakeUp · `$0F`,`$12` NormalLogic (plain damage/heal) ·
  **`$13`,`$14`,`$15`,`$16` all → `AbilityEffect_RestoreAgiAndDex`** ·
  `$1B` Poison · `$1C` Paralyze · `$1D` AndroidDeath · `$21` DexterityDown ·
  `$26` DexterityUp · `$27` RestoreStats · `$2B` IncreaseStats.

**21 distinct retail routines**, and they are formulaic, not bespoke: each is
5–20 instructions of "check current status → roll → set a bit and/or adjust a
stat". Total effect code is under 300 lines.

The status *resistance* used is an element property of the target, selected by a
constant each routine pokes into `d3` before rolling:

- `$30` `physical_prop` — Attack/Defense/Agility/Dexterity down
- `$42` `bio_prop` — instant Death
- `$44` `psy_prop` — Sleep/Paralyze (from `SleepParalyze`), Seal Tech
- `$48` `efess_prop` — Poison, Paralyze, Android Death

Range/targeting is a separate 10-entry table, `AbilityRangeOffs`
(`ps4.asm:8903`), keyed by record byte 2's low nibble: varied / single /
all-enemies / self / all-humans / all-androids. `Ability_ProcessRange`
(`ps4.asm:8975`) walks the target set, skipping the dead — except for effects
`$15`/`$16` (revive), which *require* the target to be dead.

Per-turn status processing lives in **`Battle_RestoreStatsAtTurnEnd`**
(`ps4.asm:9792`):

- restore `physical_prop` from its shadow byte (`$31`);
- for each sleeping fighter, `UpdateRNGSeed2 & 1` — **odd wakes you up**,
  restoring `agility_battle` from `agility_mod` and queueing the wake message;
- **every enemy's paralysis is cleared unconditionally at end of turn**
  (`ps4.asm:9823-9832`) — enemies effectively cannot be paralyzed for more than
  one round, characters can.

Poison does **no** damage inside battle. It ticks in the field only
(`DoCharStatsUpdate`, `ps4.asm:116895`: 1 HP every 4th movement frame, and it
can kill).

---

## 7. Enemy AI — `Enemy_Attack`, retail ≈`$00CFD4` (`ps4.asm:19138`)

The 48-byte enemy record's AI block sits at `$50–$5F` of the in-battle stats
struct: `$50–$53` = four **condition ids**, `$54–$57` = four **conditional
ability ids**, `$58–$5F` = eight **regular ability ids**. (This is exactly what
`generated/enemies.json` already emits as `condition_ids` /
`conditional_ability_ids` / `regular_ability_ids`.)

```
loc_CFE6:
    jsr     (UpdateRNGSeed2).l
    andi.w  #7, d0
    cmp.w   ($FFFFEEA8).w, d0
    beq.s   loc_CFE6               ; REROLL if it matches the previous pick
    move.w  d0, ($FFFFEEA8).w
    movea.l stats_addr(a4), a3
    move.b  $58(a3,d0.w), ability+1(a4)   ; default = one of the 8 regular abilities
    lea     $50(a3), a0
    moveq   #3, d7
loc_D00C:
    move.b  (a0)+, d0              ; condition id
    beq.s   loc_D024               ; 0 terminates
    add.b   d0, d0                 ; BYTE doubling -> table is 128 entries max
    lea     EnemyAIInstructionsOffs(pc), a1
    adda.w  (a1,d0.w), a1
    jsr     (a1)
    tst.b   d1
    bne.s   loc_D024               ; condition fired -> stop scanning
    dbf     d7, loc_D00C
```

So: **roll one of eight regular abilities (never the same index twice in a
row), then let up to four conditions override it.** Each condition routine, when
true, writes `$3(a0)` — which after the post-increment is `$54+k`, the k-th
conditional ability — into `ability+1(a4)` and returns `d1 = 1`.

`EnemyAIInstructionsOffs` (`ps4.asm:19364`) has **20 entries**, and the data
uses ids 0–15 and 17–19:

```
0 Nothing            1 EmptySpace           2 HalfHPOrLower      3 WiredineExists
4 ArthroPodExists    5 CRayTubeNearSatMinion 6 ZolSlugs          7 PhysicalAtkReceived
8 MagicDamageReceived 9 Alone               10 TechDamageReceived 11 Ambush
12 TechSealed        13 HakenLeftExists     14 BladeRightExists  15 HalfHPOrLower_AllEnemies
16 Unknown (unused)  17 HP25PercentOrLower  18 ThreeXeAThouls    19 Nothing
```

They are short and mechanical, e.g.:

```
EnemyAI_HalfHPOrLower:               EnemyAI_PhysicalAtkReceived:
    move.w  max_hp(a3), d1               btst  #0, reaction_flags(a4)
    lsr.w   #1, d1                       bne.s +
    cmp.w   curr_hp(a3), d1              moveq #0, d1
    bcs.s   +                            rts
    move.b  $3(a0), ability+1(a4)   +    move.b $3(a0), ability+1(a4)
    moveq   #1, d1                       moveq  #1, d1
    rts                                  clr.b  reaction_flags(a4)
```

`reaction_flags` (`$2A` of the fighter object) is the AI's memory of what was
done to it last round: bit 0 physical, bit 1 magic, bit 2 technique, bit 3
ambush, bit 4 multi-target. Set by the damage routines (`bset #0, $2A(a4)` etc.).

**Enemy targeting** is the `EnemyTargetRates` table in §3, chosen at ordering
time. Multi-target enemy abilities override it to `$FFFF` (all) at execution.

---

## 8. Commands, escape, defend, macros

`Battle_Command_Data` at `$FFFF410A`, 4 bytes per fighter:
`[command][ability or item id][target index .w]`.

| # | command | selection routine |
|---|---|---|
| 1 | Attack | `Battle_AttackCommand` `ps4.asm:2235` |
| 2 | Technique | `Battle_TechCommand` `ps4.asm:2326` |
| 3 | Skill | `Battle_SkillCommand` `ps4.asm:2345` |
| 4 | Item | `Battle_ItemCommand` `ps4.asm:2364` |
| 5 | Defense | `Battle_DefenseCommand` `ps4.asm:2309` |
| 6 | (combo trigger) | `loc_684A` `ps4.asm:9852` |
| 7 | Vehicle skill | `loc_68C0` `ps4.asm:9890` |
| 8 | Combo execution | `loc_2864` `ps4.asm:4046`, reads `ComboData` + `$FFFF3830` |

`Battle_AttackCommand` reads the equipped weapon's **type byte** (`$A` of the
item record) to decide targeting: types 2 and 4 (multi-target one/two-handed —
Slasher, guns) set target `$FFFF` and skip enemy selection; type 5 is a shield
and cannot attack. With exactly one live enemy, target selection is skipped.

**Defend** just writes command 5. Its mechanical effect is elsewhere: it
overwrites `physical_prop` (this is the clash the `physical_prop_save` bugfix
exists to solve — see §11).

**Escape** — `Battle_ProcessRUN` (`ps4.asm:7672`):

1. If no character has clear status (`& $6E` — paralyzed/dead/asleep/asleep2/
   android-dead) → fail.
2. If `Battle_Priority > 0` (preemptive strike) → **escape succeeds
   automatically**.
3. If no enemy has clear status → **escape succeeds automatically**.
4. If `Enemy_Run_Chance` (formation byte 1) `>= $F0` → **cannot run, ever**.
5. Otherwise roll: highest party agility vs the formation byte, scale 2,
   threshold `$28`; success iff the result is non-negative.

**Battle start priority** — `loc_B62A` (`$00B62A`): roll highest party agility
vs formation byte 0, thresholds `$C` / `$74`. `-1` = **ambush** (enemies act
first, and every enemy gets `reaction_flags` bit 3 set), `0` = normal,
`1` = **preemptive strike** (party only in the queue). Boss battles
(`Event_Battle_Index >= 0`) force normal.

**Macros** — `Macro_Data` at `$FFFFF444`, 8 slots × 20 bytes (4 bytes per
character). Beyond replaying commands, macros carry **7 smart-targeting
routines** (`MacroSpecialActionOffs`, `ps4.asm:8079`) selected by a 20-entry
lookup table (`MacroSpecialActionTable`, `ps4.asm:8095`) keyed on
(command, ability id):

```
Res/Gires/Nares, Wood Cane, Monomate/Dimate/Trimate,
ShortCake, Perolymate      -> Macro_TargetLargestHPDiff   (humans only)
Shift                      -> Macro_TargetSmallestHPDiff
Anti, Antidote             -> Macro_TargetPoisoned
Rimpa, CureParal           -> Macro_TargetParalyzed
Rever, Regen, MoonDew, SolDew -> Macro_TargetDead
Medice                     -> Macro_TargetLargestHPDiff2 (humans + androids)
RepairKit                  -> Macro_TargetLargestHPDiff3 (androids only)
```

This is a genuinely nice piece of design and it is pure data plus 7 tiny loops.

---

## 9. Encounter entry — `RunRandomBattles`, retail `$05784E` (`ps4.asm:116840`)

```
    tst.b   (Random_Battles_Flag).w         ; $FFFFECE7
    beq.w   NoRandomBattle
    cmpi.b  #1, (Tile_Collision_Standing).w ; on a map-change tile? no battle
    beq.w   NoRandomBattle
    cmpi.b  #2, (Tile_Collision_Standing).w ; recovery floor? no battle
    beq.w   NoRandomBattle
    ; also: no battle if ANY of the 8 surrounding collision cells is type 1
    ; (standing next to a town/dungeon entrance)
    ; also: no battle mid-step (x_step_duration / y_step_duration nonzero)
    bclr    #0, (Field_Movement_Flags).w    ; must have been moving
    beq.s   NoRandomBattle
    subq.b  #1, ($FFFFECE4).w               ; step counter, reset to 10 on map load
    bne.s   NoRandomBattle                  ;   and after every battle
    move.b  #1, ($FFFFECE4).w               ; from now on, roll EVERY step
    jsr     (UpdateRNGSeed).l               ; NOTE: the portable generator
    move.w  (RNG_Seed).w, d0
    tst.b   (Vehicle_Index).w
    bne.s   +
    andi.w  #$1F, d0                        ; 1 in 32 on foot
    bne.s   NoRandomBattle
    bra.s   ++
+   andi.w  #$7F, d0                        ; 1 in 128 in a vehicle
    bne.s   NoRandomBattle
+   move.w  #$14, (Game_Mode_Routine).w     ; FieldRoutine_Battle
```

So: **10 free steps after entering a map or finishing a battle, then a 1-in-32
roll on every subsequent step** (1-in-128 in a vehicle). Grace-period counter
`$FFFFECE4`; step decrement at `$057894`, the mask at `$0578B0`.

Formation selection — `Battle_SetupEnemyData` (`ps4.asm:11813`):

1. Boss battle (`Event_Battle_Index >= 0`)? → `Battle_BossFormationData`,
   index = the event battle index.
2. Otherwise `Battle_EnemyFormationIndexes[MapID]` (already extracted; the
   417-vs-416 overrun is already in the source notes). Group `> 1` is used directly.
3. Group 0/1 = overworld: decompress the Mota/Dezo position grid and index it by
   `(x >> 6, y >> 6)` — 64-cell granularity. A negative byte means group 0.
4. Decompress `Battle_FormationIndexes`, take the group's 64-byte block
   (`lsl.w #6`), and pick **one of 32 entries** with `UpdateRNGSeed2 & $1F`.
5. That yields a formation number; subtract `$80` repeatedly to walk
   `Battle_FormationData1..4`, then skip that many `$FF`-terminated records.
6. Copy 16 bytes to `Enemy_Formation_Data` (`$FFFF41F0`) and expand each enemy
   through `Battle_FillEnemyStats`.

Formation header (matches `generated/formations.json` exactly): byte 0 ambush
chance, byte 1 run chance, byte 2 item-drop rate, byte 3 dropped item, byte 4
enemy count, bytes 5–6 group bitmasks, byte 7+ `(enemy id, position)` pairs,
`$FF` terminator.

---

## 10. Rewards and drops

**Accumulation** — every enemy death (`loc_2D960`, retail `$0002D960`) adds the
enemy record's `$60` (experience) to `$FFFF41CE` and `$62` (meseta) to
`$FFFF41D0`, each **saturating at `$FFFF`**.

**Distribution** — `Battle_VictoryMessage` (`ps4.asm:4705`):

```
    ; d2 = number of party members not dead/android-dead
    move.w  ($FFFF41CE).l, d1
    tst.w   (Vehicle_Index).w
    beq.s   +
    lsr.w   #1, d1                 ; vehicle battles award HALF experience
+   divu.w  d2, d1
    andi.l  #$FFFF, d1             ; integer quotient, remainder discarded
```

Each living member gets `d1`, capped at `9999999`. Then, **only while
`EventFlag_Reunion` is unset**, every character *outside* the party who has
`gain_exp_flag` (`$7A`) set also receives the full `d1` — that flag is set the
first time a character wins a battle. Meseta: `$FFFF41D0` added to
`Current_Money` (`$FFFFF438`), capped at `9999999`.

**Level-up** — `BattleResults_PartyExp` (`ps4.asm:5993`) walks
`Current_Party_Slots`, indexes `CharLevelTablePtrs` (11 entries of
`[starting level .w][table pointer .l]`, `ps4.asm:6054`), and reads the 22-byte
record for `level − starting_level`. Retail applies **one level per battle per
character** and this is where two real bugs live (§11).

**Item drop** — `Battle_CheckItemDrop` (`ps4.asm:4831`):

```
    jsr     (UpdateRNGSeed2).l
    andi.b  #$7F, d0
    cmp.b   (Item_Drop_Rate).l, d0   ; formation byte 2
    bge.s   -> no drop
```

Drop iff `(roll & $7F) < rate`, i.e. `rate/128`. The item is formation byte 3.
If the party's inventory is full, the game offers a swap (`$FFFFF437` path).

---

## 11. Traps

### Fork contamination

Unlike the event engine, the battle code is **not** wholesale-rewritten by Grand
Cross — there are no ungated `include` swaps in the battle region. What there
*is*, is a dense field of `if bugfixes=1` gates. Every one below was checked
against cartridge bytes; **retail always takes the `else` branch.**

| `ps4.asm` | what the fork "fixes" — i.e. what retail actually does |
|---|---|
| 3254, 5788 | Item-name offset from Tornado Dagger on. Retail (`revision != 0`) uses `+1`. |
| 3800 | PC-relative vs absolute addressing of the stat table. Cosmetic. |
| 3822, 3913 | **Retail applies a *shield's* element property as if it were a weapon's**, in both the basic-attack and physical-skill paths. Verified absent at `$277A`. |
| 5926 | **Level-99 pointer bug.** Retail does `sub.w (a3)+, d0 / movea.l (a3)+, a1` *after* the level-99 early-out, so a level-99 character leaves `a3` un-advanced and **every character after them in the loop reads the wrong level table.** |
| 5975 | Retail grants **at most one level per battle** to out-of-party characters (no loop-back). |
| 6119 | Retail does **not** re-run `UpdateCharModStats` after a level-up, so equipment-derived stats lag one level behind until something else refreshes them. |
| 6368, 9835, 11287 | No `physical_prop_save` shadow in retail: **Defend and physical-resistance armour clobber each other.** |
| 9056, 9371 | `AbilityEffectsOffs[$13..$16]` all point at `AbilityEffect_RestoreAgiAndDex` — verified in ROM at `$0061BE`. Cure Poison / Cure Paralysis / Revive / Full Revive share one routine that **restores agility and dexterity unconditionally, whether or not you were afflicted.** |
| 16109 | Retail **consumes Telepipe and Escapipe** if you use them in battle. |

`if revision=0` gates in the battle region (`1869, 1879, 2004, 4670, 4786, 5243,
5448, 6154, 6189, 6224, 6270, 6420, 12779`) are Japanese-build variants; retail
US is `revision != 0`, consistent with the Enigma finding already in
docs/source-notes/disassembly-discrepancies.md.

### New retail cartridge findings

1. **`Battle_ProcessRUN` calls `Battle_CalculateChances` with `d5`
   uninitialised** (`ps4.asm:7704-7712`). Dormant: only the sign of the result is
   tested and both possible values are non-negative.
2. **Effect id `$2C` is one past the end of `AbilityEffectsOffs`.** The table at
   `$0061BE` has exactly 44 entries (`$00–$2B`, last word `$0330`, immediately
   followed by `AbilityEffect_None`'s `4E75`). Enemy skill 112, `BLACK WAVE`
   (`raw_hex 2c00000000000000`), declares effect `$2C`. The `TRAP #2` dispatcher
   at `$000216` is `add.w d0,d0 / adda.w (a0,d0.w),a0 / jsr (a0)` — **no bounds
   check** — so the jump would land on odd address `$00B033` and raise an
   address error. **CORRECTED 2026-08-15**: `BLACK WAVE` belongs to enemy 152 `Zio3`
   (HP 16383, agility 255, defence 255, magic-defence 255) — and Zio3 IS
   fielded, in boss formation `event_battle_index` 4, the scripted Zio
   fight (the original claim searched only the 504 normal formations).
   Whether retail can reach the crash there is unsettled; see
   the source notes.
3. **`AbilityEffect_Death`, `Poison`, `Paralyze`, `SealTech` and the stat-down
   effects resist off *element properties*, not a saving throw** — `bio_prop`,
   `efess_prop`, `psy_prop`, `physical_prop`. Not a bug; just a design fact that
   is easy to get wrong from documentation, because those property slots are
   also used as damage multipliers.
4. **Enemy paralysis is cleared unconditionally every turn end**
   (`ps4.asm:9823-9832`), while character paralysis persists. Asymmetric by
   construction, not by data.
5. **`AbilityEffect_None` occupies 20 of the 44 effect slots.** Records that
   declare those ids simply do nothing on the status side (they still deal
   damage through the separate pipeline).

---

## 12. Worked example — one complete basic attack, real numbers

The worked numerical example is preserved in
[`BATTLE_SCOUT_CONTINUATION.md`](BATTLE_SCOUT_CONTINUATION.md). The current
slice keeps the presentation boundary below as the main cross-lane contract.

---

## 13. Presentation boundary

Clean, and it splits almost exactly where you'd want it to.

**Pure presentation** (renderer's problem, zero rules):

- All 67 `BattleRoutines2` entries — windows, cursors, lists, paging, messages.
- `Battle_RunObjects` / `Battle_BuildSprites` / `Battle_FillSpriteAttributes` /
  `Battle_AnimateSprite` / `Battle_UpdatePaletteObjs` and the **574**
  `BattleObj_*` animation objects.
- The **154-entry `EnemyAttackOffs`** table (`ps4.asm:19206`), one entry per
  enemy id, collapsing to **74 distinct routines**. Each spawns an animation
  object and pokes palette entries. `EnemyAttack_Helex` is representative:
  `move.w #$48, (a1)` (= `BattleObj_HelexFlameBolt`), two palette words, load a PLC.
- `Battle_SetupBackground` / `Battle_BackgroundIndexes`, weapon palettes and PLCs
  (`Battle_WeaponIndex` `ps4.asm:13082`, `Battle_WeaponPaletteTable`,
  `Battle_LoadWeaponPLC`).
- `Battle_Speed` (`$FFFFF442`) feeds only a frame-count multiplier at
  `$FFFFEE66` (`ps4.asm:9993`) — a pacing knob, not a rules knob.
- The damage-number rendering path (`loc_255C`, `Fighter_OpenDamageWindow`,
  `Fighter_ShowDamage`).

**Leaks to watch** — presentation routines that touch state:

- `EnemyAttack_Slave` and several others write
  `Current_Target_Index = $FFFF` (all targets) from inside the animation
  routine.
- `EnemyAttack_Tower` reads `fighter_id` to pick a palette — harmless, but it
  proves these routines see game state.
- `FighterShowDamage_DecreaseHP` — the actual HP subtraction — is reached from
  the animation state machine, so "when damage lands" is a presentation-timed
  event on hardware.
- `Battle_DoAttackEffect`'s tail (`ps4.asm:8630-8670`) checks the effects array
  for Death/Sleep and spawns `BattleObj_DeathEffect` / `BattleObj_SleepEffect`,
  stalling the logic machine (`clr.w Battle_Routine`) until the animation ends.

**Where enemy battle art lives** (the base record and its decoded overlay lane):

`Enemy_Init` (`ps4.asm:17699`) does
`lea (loc_27F3BC).l, a3` and indexes it by `fighter_id * 20`. Retail
**`$0027F3BC`**, verified: five longs per enemy id.

```
$0027F3BC:  00 20 D8 50   04 08 00 12   00 20 D7 7C   00 20 D2 C8   00 20 D2 A6
            ^mapping ptr  ^w ^h ^base   ^ptr          ^ptr          ^ptr
$0027F3D0:  00 20 D8 50   04 08 00 1E   00 20 DC E6   00 20 D8 AC   00 20 D8 70
```

(Correction 2026-08-15, from the extraction — `psiv_tools/battle_art.py` is
now the authoritative decode: the records table is `loc_27F3AE` @ `0x27F3AE`,
stride 20, 153 entries, ending exactly where the 22-byte palette table
`loc_27FFA2` begins; the `+$00` word is a VRAM *reservation* advanced by
`lsl.w #5` at `loc_7BFA`, not a mirror of art #1's header — that claim was
wrong for 104 of 153; the width byte is a HALF-width, doubled by `loc_10ED4`
before drawing; the mapping addresses the three art blobs concatenated in
order #3, #1, #2; and two mappings — ProfoundDarkness2/3 — are raw word
arrays, not Enigma.)

Long 0 is an **Enigma-compressed plane mapping**, decompressed by `loc_10ED4`
(`$00010ED4`) into RAM and stamped into the plane buffer with `PlaneMapToRAM` —
the static body is therefore a background-plane graphic. Bytes 4/5 are the
mapping's width and height in cells; bytes 6–7 the base tile. The remaining
three longs are the art/animation pointers, all in the **`$20D000–$20E000`**
neighbourhood. `EnemySpriteMappingsOffs` and `Battle_EnemySpritesJmpTbl`
(`ps4.asm:98555`, only 5 entries) handle the small animated overlay pieces via
`Enemy_Sprites` at `$FFFFEA00` (16 slots); the decoded renderer applies those
pieces as dynamic tile replacements over the body.

### Enemy overlay decode (2026-08-15)

The retail table is `EnemySpriteMappingsOffs` at **`0x010FCE`**, 153
self-relative words. Its data starts at `0x011100`. Each selected block is:

```text
dc.w descriptor_count - 1
repeat descriptor_count:
    dc.b destination_tile, tile_count
    dc.l sequence_pointer
```

`loc_7C56` decompresses Art #3 into its staging bank, then copies
`tile_count` 32-byte tiles from the enemy's Art #2 RAM buffer at
`source_pattern * 32` to `destination_tile * 32`. The high bit of the pointer
is a flag: `loc_10F90` leaves that piece's `$FFFFEA00` slot disabled, while
`loc_7C56` still decodes the descriptor bytes. Active source ranges and
destination ranges are checked against the retail blobs; no guessed padding is
accepted.

The sequence pointer is a long pointer wrapper. `EnemySprites_Animate` at
**`0x040F66`** has two formats:

```text
ordinary: count, cadence, source_pattern[count]
compact:  0x80 | count, duration[count], source_pattern[count]
```

The startup source is `source_pattern[0]`. Each update decrements the stored
timer; on expiry it advances the frame, wraps to zero, and loads the next
duration. A stored duration `n` therefore holds a state for `n + 1` update
calls. The emitter records storage order, runtime order (`1..n-1, 0`), raw
cadence/durations, source Art #2 offsets, and every body placement including
the mirrored H-flipped occurrence.

This is a dynamic **tile replacement** path, not a second alpha image to lay
over the old body. Colour zero in a replacement tile must clear the previous
body pixel. `psiv_tools/battle_enemy_overlays.py` emits one full-body-sized
indexed PNG for each active piece state; `battle/art/enemy_overlays.json`
records the destination rectangles, offsets relative to the body, size,
palette line choice, flips, cadence, and ROM provenance. The Rust renderer
blits only those rectangles into a fresh body image each time a decoded piece
advances.

The USA ROM decodes to **66 unique blocks, 257 descriptors, and 239 enabled
pieces**. The static body census remains **84 complete / 69 holed / 303 hole
cells**. Dynamic destinations fully cover the holes of **59** of those 69
enemies (216 cells); 39 enemies keep a nonblank tile in every decoded frame.
Ten enabled descriptors have destination tiles absent from their body plane,
so they are retained as explicit `not_in_body_mapping` metadata rather than
given invented coordinates: ids **44–46, 50, 70–72, 139–140, and 152**.

---

## Implementation scope

The original v1 sizing, proposed scope ladder and design-session questions
are retired. Current work is tracked in [ROADMAP.md](../ROADMAP.md). The worked
example and address tables remain in [the continuation](BATTLE_SCOUT_CONTINUATION.md).

## Appendix: verified retail addresses

The full address table and battle RAM notes continue in
[`BATTLE_SCOUT_CONTINUATION.md`](BATTLE_SCOUT_CONTINUATION.md).
