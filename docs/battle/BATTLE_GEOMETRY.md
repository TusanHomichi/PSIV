# Battle screen geometry

Scouted 2026-08-15 (Fable). The layout facts the battle-screen build needs:
where the background sits, where enemies and party members are placed, where
the windows and damage numbers go, and what sets the pace.

Every constant below was verified against the retail cartridge
(`Phantasy Star IV (USA).md`, sha256 `511f35cc…13b6a`) by matching opcode
bytes — see the appendix. Dimensions come from the extractor, which proves them
against the same image. The command-idle rectangles and status strip below are
also checked against `oracle/frames/frame_25000.png`, the retail 320x224 capture.
Where something is genuinely dynamic the rule is given rather than a number.

`loc_XXXXX` labels in `ps4.asm` are retail addresses (established in
`docs/battle/BATTLE_SCOUT.md`); the `;0x…` inline comments are the fork's and drift.

---

## 1. The frame

| | |
|---|---|
| screen | 320 × 224 px = **40 × 28 cells** |
| both planes | 64 × 32 cells = 512 × 256 px (VDP reg `$9001`) |
| plane A | `Plane_A_Buffer` `$FFFF8000` — **fighters and windows** |
| plane B | `Plane_B_Buffer` `$FFFF9000` — **the background** |
| plane row stride | `$80` = 128 bytes = 64 cells, one pattern-name word per cell |

**At rest both planes scroll to zero**, so the mapping is direct:

```
plane cell (col, row)  ->  screen pixel (col * 8, row * 8)     for col < 40, row < 28
plane byte offset      =   row * $80 + col * 2
row = (addr - $8000) / $80        col = ((addr - $8000) mod $80) / 2
```

Every plane-A address in this document is quoted with its `(row, col)` beside
it, computed that way.

### The background

Battle backgrounds are Enigma plane mappings decompressed **straight into
`Plane_B_Buffer`** (`Battle_SetupBackground`, `ps4.asm:10124`) — no staging
buffer, no per-cell fixups.

| | |
|---|---|
| size | **64 × 24 cells = 512 × 192 px** |
| table | `BattleBGArtPtrs` at `$006ED4`, 32 entries, **20 distinct** mappings |
| base tile | `$0000` |
| palette | CRAM line 0, indices 1–13, index 0 forced black (source notes: [formats](../source-notes/formats.md)) |

The visible 320 px is the background's left 40 of 64 columns; the right 24
columns are never shown at scroll 0. Vertically the background covers **screen
y 0–191** and stops: rows 24–27 (**y 192–223**) have no background behind them,
which is the strip the UI sits in.

### The intro sweep

`GameMode_LoadBattle` starts the two planes off-screen and walks them together:

```
$006B3C   move.w  #$FF20, (Camera_Y_Pos_FG).w    ; -224
$006B42   move.w  #$E0,   (Camera_Y_Pos_BG).w    ; +224
          move.w  #$E, ($FFFF4104).l             ; 15 steps
loc_6BA4: addi.w  #$10, (Camera_Y_Pos_FG).w
          subi.w  #$10, (Camera_Y_Pos_BG).w
loc_6B96: ... until Camera_Y_Pos_FG reads 0
```

15 steps of 16 px each, the two planes converging from opposite directions onto
scroll 0. A renderer that only wants the settled scene can skip straight to
zero; a faithful one replays the sweep.

---

## 2. Enemy placement

The formation record's second byte per enemy is the position. `Enemy_Positions`
(`$FFFF41F7`) holds two bytes per enemy — `(enemy id, position)` — copied there
by `Battle_SetupEnemyData`.

`loc_79AC` (`$0079AC`), byte-verified, run identically for all four slots:

```
    move.b  (a0)+, d0          ; the position byte
    bpl.s   +
    move.w  #$40, $1E(a1)      ; bit 7 set -> palette variant
+   andi.w  #$7F, d0
    lsl.w   #3, d0             ; * 8
    add.w   d2, d0             ; + $80
    move.w  d0, $C(a1)         ; fighter_x_pos
```

So the byte splits into two independent things:

| bits | meaning |
|---|---|
| 0–6 | **the plane-A column the art's right edge sits on** |
| 7 | `battle_sprite_tile_props2` (`$1E`): `$20` clear, **`$40` set** — an offset into `Palette_Table_Buffer`, i.e. which sub-palette the enemy draws with |

`fighter_x_pos = $80 + (position & $7F) * 8`.

### From `fighter_x_pos` to the blit

`loc_10ED4` (`$00010ED4`), the enemy art decoder:

```
    move.b  (a3)+, d1          ; art width  in cells
    move.b  (a3)+, d2          ; art height in cells
    ...
    add.w   d4, d3             ; d3 = w*2 + h*128
    move.w  $C(a4), d4         ; fighter_x_pos
    subi.w  #$80, d4
    lsr.w   #2, d4             ; byte offset = (x - $80) / 4
    lea     ($FFFF8780).w, a1  ; row 15, col 0
    adda.w  d4, a1
    suba.w  d3, a1             ; back up w cells and h rows
    jsr     (PlaneMapToRAM).l
```

`$FFFF8780` is **row 15, column 0**, and `(x − $80)/4` bytes is exactly
`position & $7F` cells. The art record's byte 4 is a **half-width** anchor
(`hwidth`), not the rendered width: the loader doubles it for the six/eight/
ten-cell body map, while the destination pointer backs up `hwidth` cells
(`hwidth*2` bytes) and `height` rows (`height*128` bytes).

**The position byte is the art anchor seam.** An enemy with half-width `hwidth`
and height `height` occupies:

```
columns  (position & $7F) - hwidth .. (position & $7F) + hwidth - 1
rows     15 - height               .. 14
```

The half-width and height are bytes 4 and 5 of the 20-byte per-enemy art
record at **`$0027F3BC`** (`record = $0027F3BC + enemy_id * 20`; long 0 is the
Enigma mapping, bytes 6–7 the base tile). The generated pack exposes both
`half_width_cells` and the derived `width_cells = 2 * half_width_cells`.

**Every enemy stands on the same baseline**: the bottom row is always plane row
14, i.e. **screen y 112–119**, feet at y = 120. Taller enemies grow upward.

### Spacing

There is none — no arithmetic spaces enemies apart. Each enemy's column is
whatever its formation record says, and the formations are hand-authored so the
art does not collide. A renderer must not invent spacing; it must use the bytes.

Formation 0 of block 1 (two MonsterFly) carries anchors 14 and 26, so the
centres/seams are 12 cells apart. The body span comes from each enemy's own
half-width record; the renderer must not substitute the full width for the
anchor subtraction.

The tape-07 command-idle receipt makes this distinction concrete. RAM at
`$41F0` on frame 25000 begins
`0F 05 00 00 02 03 00 0A 0E 0A 1A FF ...`: two enemy id-10 Zoran Bults at
positions `$0E` and `$1A`. Zoran's half-width is 3, so the six-cell body runs
are columns 11..16 and 23..28, with pixel origins `(88,72)` and `(184,72)`.
The party receipt is Chaz/Alys/Hahn at `25/10`, `53/40`, and `21/25` HP/TP;
the settled command frame has no debug attack timeline event.

### Frame-25000 VDP and idle overlay receipt

The complete fresh host dump is retained at
`oracle/states/battle_command_idle_vdp_25000.json`; its source command is
tape 07 at frame 25000 with `--dump-state`, `--dump-frames`, and the Grand
Cross build (`grand_cross=0`). The retail SAT region is empty at this settled
frame. The visible enemy body is therefore the Plane A/VDP tile result, not a
SAT sprite.

Zoran Bult's three enabled overlay pieces have durations `[8,8,8,8]`,
`[4,4,4]`, and `[20,4,4]`. In the JSON/decoded piece order (lower, middle,
upper), the frame-25000 VDP pixels compose as `(2,2,1)`. The command-idle
debug setup seeds the overlay clock with **19 elapsed ticks**; it does not seed
a frame index and it does not apply the pack's optional runtime-order metadata
to this receipt.

The tick-200 debug hook runs before `drive_battle_if_active`, so the pre-drive
state has 170 updates after the 19-tick seed and is `(1,2,1)`. The receipt
screenshot is now taken after that drive's one final update, yielding 171
elapsed updates and `(2,2,1)`. This is a capture-boundary correction, not a
global animation-speed adjustment or a phase-number workaround. Normal
formation setups retain phase zero.

Enemy recolouring uses the Genesis Plus GX RGB565 ramps: red/blue
`[0,32,65,98,139,172,205,238]` and green
`[0,32,68,101,137,170,206,238]`. The old bit-replication widening was not
the emulator's colour conversion and is not used by the battle path.

### VRAM sharing

Slots 2–4 compare their enemy id against the earlier slots and, on a match,
copy the earlier slot's art fields instead of calling the loader
(`ps4.asm:11497-11580`). Duplicate enemies in a formation share one VRAM
allocation. Geometry is unaffected — each still gets its own position byte —
but an art cache keyed by enemy id is the cartridge's own model.

### The enemy-name label objects

Two extra fighter objects carry the group name labels, placed by `loc_7B94`
(`$00007B94`) from the formation's two group bitmasks:

| object | `fighter_x_pos` | obj id |
|---|---|---|
| `Fighter_Enemy_Group_1` (`$FFFF4640`) | `$84` | 6 |
| `Fighter_Enemy_Group_2` (`$FFFF4680`) | `$B4` | 7 |

These are the "MONSTER-FLY ×2" labels, not bodies. Their plane rect comes from
`EnemyGroup_SetupNames`, which I did not chase — see §7.

---

## 3. Party placement

Five fixed slots. `Character_Draw` dispatches on the fighter's object id into
five routines at **`$008684`**, byte-verified in full:

| party slot | `fighter_x_pos` | plane A dest | (row, col) | pose grid |
|---|---|---|---|---|
| 1 | `$120` | `$FFFF87A2` | (15, **17**) | `$FFFF3000` |
| 2 | `$00F0` | `$FFFF8796` | (15, **11**) | `$FFFF3048` |
| 3 | `$0150` | `$FFFF87AE` | (15, **23**) | `$FFFF3090` |
| 4 | `$00C0` | `$FFFF878A` | (15, **5**) | `$FFFF30D8` |
| 5 | `$0180` | `$FFFF87BA` | (15, **29**) | `$FFFF3120` |

Then, shared:

```
loc_86D2:  move.w  d0, $C(a4)     ; fighter_x_pos
loc_86D6:  moveq   #6, d1
           moveq   #6, d2
           jmp     (PlaneMapToRAM).l
```

**Each character is 6 × 6 cells = 48 × 48 px**, drawn *downward* from row 15 —
the opposite direction from enemies, who are drawn upward from the same row.

```
columns  as tabled            rows  15 .. 20      screen y  120 .. 167
```

Left to right on screen the order is **slot 4, slot 2, slot 1, slot 3, slot 5**
— columns 5, 11, 17, 23, 29, exactly 6 cells apart with no gap. The party is
laid out centre-out, the leader in the middle.

The `fighter_x_pos` values are self-consistent with the columns through a
**different origin from the enemies'**:

```
character column = (fighter_x_pos - $98) / 8
enemy     column = (fighter_x_pos - $80) / 8
```

Both are used verbatim by the erase path (`loc_871C`, `$0000871C`) and the
damage-number placement, so the `$98`/`$80` split is real and not a
transcription slip. Check: `($C0 − $98)/8 = 5`, `($F0 − $98)/8 = 11`,
`($120 − $98)/8 = 17`, `($150 − $98)/8 = 23`, `($180 − $98)/8 = 29`.

### Erase and staging

- `loc_8704` (`$00008704`) clears a slot: 6 rows × 12 bytes of zeros — 6 × 6
  cells, matching the draw.
- `Character_Draw`'s tail copies plane A **rows 15–20, columns 0–39** (40 × 6)
  to `$FFFF28B0`, a staging buffer for the DMA queue. That rectangle is the
  whole party band, 320 × 48 px.

### Poses, for Tier 1

The pose buffer table is `loc_9A80` (`$00009A80`): five longs,
`$FFFF3000 + slot * $48`. `$48` = 72 bytes = **36 pattern-name words = the
6 × 6 grid**. One grid per slot, overwritten in place as the pose changes;
`Character_Draw` blits whatever is currently in it.

Content is Enigma-decoded into the grid by `loc_822C`, with the base tile taken
from the fighter's own VRAM allocation (`vdp_dest_addr >> 5`, plus the
`$1E` palette offset) and the art from a per-character PLC,
`CharBattleLongRangePLCOffs` (`ps4.asm:12481`).

For Tier 1 that reduces to two states:

| state | routine | `fighter_routine` |
|---|---|---|
| **idle (default)** | `Character_Draw` | 2 |
| **swing** | `Character_Attack` | 6 |

`Character_Attack` refuses and bails with `$32(a4) = $FFFF` unless a hand holds
an item of type 1–4, which is the same weapon test the engine's
`weapon_reach` implements. The turn engine drives the transition by writing
`fighter_routine`; the renderer only needs to know that 2 is standing and 6 is
swinging. The pose-index-to-frame timeline is out of scope here by instruction.

---

## 4. Windows

All battle windows are plane-A rectangles. Rows and columns below are derived
from the addresses by the §1 formula.

| what | plane A address | (row, col) | size (cells) | screen rect (px) |
|---|---|---|---|---|
| wide list window (items, techniques) | `$FFFF8814` | (16, 10) | 20 × 5 | x 80–239, y 128–167 |
| small list window | `$FFFF87BE` | (15, 31) | 7 × 5 | x 248–303, y 120–159 |
| transient combat message | `$FFFF8916` | (18, 11) | 12 × 3 | x 88–183, y 144–167 |
| transient message base | `$FFFF8900` | (18, 0) | — | the row the transient windows index from |
| upper window | `$FFFF848A` | (9, 5) | 15 × 6 | x 40–159, y 72–119 |

### Command-idle capture

The tape-07 command-idle frame pins the battle opener and the bottom status
layout. These are presentation rectangles, not guesses from the cursor tables:

| what | screen rect (px) | cells | contents |
|---|---|---|---|
| enemy group name | x 16–111, y 8–31 | 12 × 3 | first/current enemy-group name |
| main command menu | x 24–87, y 40–95 | 8 × 7 | COMD, MACR, RUN; one 8×8 bullet per row |
| status pane 0 | x 16–79, y 168–215 | 8 × 6 | HP:/TP: labels and empty `?` box |
| status panes 1–3 | x 72–135, x 128–191, x 184–247; y 168–215 | 8 × 6 each | member name, HP/TP, `?` box |
| status pane 4 | x 240–303, y 168–215 | 8 × 6 | HP:/TP: labels and empty `?` box |

Adjacent status panes overlap by one cell (56 px pitch, 64 px frame width).
The reference party order is Chaz, Alys, Hahn; the battle fighter slots remain
the independent center-out table in §3. Menu and status text use the retail
8×8 `ArtNem_Font`; dialogue's 8×16 font is a different asset. The selected
COMD bullet is red, while the unselected/MACR bullets are blue. MACR is
displayed but has no Tier-1 action.

The retail command-idle frame has no floating narration box. Combat narration
uses the 12×3 transient rectangle above; victory/results text uses the 20×5
wide list rectangle. The status strip remains visible while those windows are
active.

Sizes come from the `d1`/`d2` pair each opener passes to `Battle_SetupWindow`
/ `Battle_CreateWindow` (`d1` = width − 1 in some callers, so the table above
uses the literal `moveq` values as counts where the routine treats them as
counts; see §7).

### The defend / transient window offsets

`loc_49AC` (`$000049AC`) opens a 12 × 3 window at `$FFFF8900 + offset`, with the
offset chosen per acting slot from `loc_4A3A` (`$00004A3A`):

```
loc_4A3A:  dc.b  $0A, $22, $16, $16, $22
```

Indexed by object id − 1, that gives columns **5, 17, 11, 11, 17** for slots
1–5. Note this does **not** track the party columns (17, 11, 23, 5, 29) — the
window snaps to three positions rather than following the actor. Reported as
the raw table; do not derive it from the actor's column.

### Damage numbers

`loc_29B4` (`$00029B4`), byte-verified — this is the position rule per target:

```
    move.w  $C(a4), d0         ; the TARGET's fighter_x_pos
    subi.w  #$98, d0
    lsr.w   #2, d0             ; byte offset = (x - $98) / 4
    lea     ($FFFF8280).w, a1  ; row 5, col 0
    adda.w  d0, a1
    move.w  a4, d0
    subi.w  #$4400, d0
    lsr.w   #6, d0             ; fighter slot 0..8
    cmpi.w  #5, d0
    bge.s   +                  ; enemy: stay on row 5
    adda.w  #$680, a1          ; character: down 13 rows -> row 18
+   rts
```

| target | plane row | screen y | column |
|---|---|---|---|
| **enemy** (slot ≥ 5) | **5** | 40 | `(fighter_x_pos − $98) / 8` |
| **character** (slot < 5) | **18** | 144 | `(fighter_x_pos − $98) / 8` |
| vehicle battle, enemy actor | fixed `$FFFF87A4` | (15, 18) | — |

The column formula uses the **character** origin `$98` for both sides. An
enemy's `fighter_x_pos` is `$80 + c*8`, so its number lands at column
**`c − 3`** — three cells left of the enemy's anchor column, which centres a
three-digit number under the body's right edge.

The digit block itself (`loc_255C`, `$0000255C`) is **5 cells × 2 rows**: a
frame at cell offsets 0 and 4, and the hundreds / tens / units at offsets 1, 2,
3. Values are split by repeated subtraction of 100 then 10, so leading digits
are suppressed by pointing at a blank-digit source rather than by shifting.

---

## 5. The pacing knob

| | |
|---|---|
| `Battle_Speed` | `$FFFFF442`, word |
| default | **2**, set by the new-game initialiser at **`$04444A`** |
| player range | **0–4** (config menu, `Win_UpdateCursorLeftRight` with `d0 = 4`); the menu edits `Battle_Speed+1`, the low byte |
| derived dwell | `$FFFFEE66` = **12 × (`Battle_Speed` + 1)** frames |

`GameMode_LoadBattle` at **`$006A42`**, byte-verified:

```
    move.w  (Battle_Speed).w, d0
    addq.w  #1, d0
    add.w   d0, d0          ; 2(s+1)
    add.w   d0, d0          ; 4(s+1)
    move.w  d0, d1
    add.w   d0, d0          ; 8(s+1)
    add.w   d1, d0          ; 12(s+1)
    move.w  d0, ($FFFFEE66).w
```

| `Battle_Speed` | frames | seconds at 60 Hz |
|---|---|---|
| 0 | 12 | 0.20 |
| 1 | 24 | 0.40 |
| **2 (default)** | **36** | **0.60** |
| 3 | 48 | 0.80 |
| 4 | 60 | 1.00 |

`$FFFFEE66` is the dwell the message and animation waits count down from — it
is copied into `$FFFFEE46` by the defend window (`loc_4A30`) and into the
damage-number object's timer by `loc_24BE`. It is the authentic cadence for
timeline playback, and it is a *player setting*, so the renderer should read it
rather than hard-code 36.

---

## 6. Tables worth emitting

Small enough that a hand-written constant table is defensible, but these are
the genuinely tabular ones if the pack is to carry them. Routing is the lead's
call.

**Party slot placement** (5 rows) — from `$008684`:

| slot | fighter_x_pos | plane_col | plane_row | pose_buffer |
|---|---|---|---|---|
| 1 | 288 | 17 | 15 | `$FFFF3000` |
| 2 | 240 | 11 | 15 | `$FFFF3048` |
| 3 | 336 | 23 | 15 | `$FFFF3090` |
| 4 | 192 | 5 | 15 | `$FFFF30D8` |
| 5 | 384 | 29 | 15 | `$FFFF3120` |

**Enemy art sizes** (153 rows) — bytes 4/5 of each 20-byte record at
`$0027F3BC`. Needed to turn a position byte into a rect, since the anchor is
the bottom-right corner. This one genuinely wants emitting: it is per-enemy and
the renderer cannot compute it.

**Battle background index** (32 entries → 20 distinct) — already in
`generated/planes.json` as `battle_backgrounds`; a runtime consumer needs the
per-map and per-event-battle selectors from `Battle_BackgroundIndexes`
(`ps4.asm:10219`) and `EventBattleBGIndexes` (`ps4.asm:10184`).

**Battle speed** (5 rows) — trivial; a constant in the renderer is fine.

---

## 7. What I did not pin

Named rather than guessed, per house standard.

1. **Window size conventions.** Several openers pass `moveq #N, d1` where other
   call sites clearly mean "N cells" and some appear to mean "N − 1". I have
   quoted the literal values; the command-idle capture settles the actual
   screen rectangles even where an opener's argument convention is ambiguous.
   Dynamic list contents and animation-time window lifetimes still need their
   own tape captures.

Also deliberately out of scope, by instruction: the pose-index-to-frame
animation timeline. §3 gives the idle default and the swing trigger only.

---

## Appendix: verified retail addresses

| what | retail | how proven |
|---|---|---|
| enemy position → `fighter_x_pos` | `$0079AC` | `0240 007F / E748 / D042 / 3340 000C` |
| enemy art blit anchor | `$00010ED4` | `205B 302C 000E` (`docs/battle/BATTLE_SCOUT.md`) |
| enemy art record table | `$0027F3BC` | `0020D850 04080012 …` (BATTLE_SCOUT §13) |
| party slot placement | `$008684` | full 86-byte block matched |
| party pose buffer table | `$009A80` | `loc_9A6C` reads it by slot |
| party slot erase | `$00008704` | 6 rows × 12 bytes |
| damage-number placement | `$0029B4` | full 58-byte block matched |
| damage digit renderer | `$0000255C` | `loc_255C` |
| defend/transient window | `$000049AC`, table `$00004A3A` | `dc.b $0A,$22,$16,$16,$22` |
| battle camera init | `$006B3C`, `$006B42` | `31FC FF20`, `31FC 00E0` |
| `Battle_Speed` → `$FFFFEE66` | `$006A42` | `3038 F442 5240 D040 D040 3200 D040 D041 31C0 EE66` |
| `Battle_Speed` default 2 | `$04444A` | `31FC 0002 F442` |
| background art table | `$006ED4` | extractor (`generated/planes.json`) |

RAM, from `ps4.constants.asm`: `Plane_A_Buffer` `$FFFF8000`,
`Plane_B_Buffer` `$FFFF9000`, `Enemy_Positions` `$FFFF41F7`,
`Obj_Fighters` `$FFFF4400`, `Fighter_Enemy_1` `$FFFF4540`,
`Fighter_Enemy_Group_1` `$FFFF4640`, `Fighter_Enemy_Group_2` `$FFFF4680`,
`Battle_Speed` `$FFFFF442`, `Camera_Y_Pos_FG`/`_BG`, dwell `$FFFFEE66`.

Object offsets: `fighter_x_pos` `$C`, `vdp_dest_addr` `$E`,
`battle_sprite_tile_props2` `$1E`, `action_routine` `$32`.
