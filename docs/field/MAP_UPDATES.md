# Per-frame map updates

Tracking: [Issue #41](https://github.com/TusanHomichi/PSIV/issues/41).

The retail `RunMapUpdates` dispatcher is `$05493A`; its `MapUpdateJmpTbl`
is `$054972`, 64 `bra.w` entries. Every real map carries an ordered byte
list terminated by `$FF`. A nonzero return would stop the list, but all 64
retail entries return zero. Sources: `ps4.asm:112859-112953`, confirmed against
US bytes, not the fork's assembled-address annotations.

## Scope

The pack carries every decoded update list and table; the runtime executes
every modeled routine inside the field frame and reports the inputs it does not
model yet (below); Godot draws the runtime-owned palette state.

## Frame ownership and ordering

- `FieldRoutine_Controls` and `FieldRoutine_VehicleControls` call the table
  after `Field_UpdateObjects` and `AnimateTiles`, before the four camera commits,
  tile refresh and VInt (`ps4.asm:116755-116842`). Scene, transition, battle or
  input dispatch takes an early return and skips the table on that frame.
- Ordinary dialogue, camp, settings and shops do **not** dispatch it. The fork's
  added calls are inside `bugfixes=1` or `run_map_updates_during_dialog=1`, absent
  in US retail (examples `ps4.asm:117122-117136,119379-119393,134254-134268`).
  `Main_Frame_Count` still advances: reopening a field picks up the absolute
  palette phase, while local counters remained parked.
- Scene `DoMapUpdateLoop`, `DoMainUpdatesLoop`, DMA-plane update loops and
  `DoMapUpdate_JoypadLoop` do call it before their VInt; `VInt_PrepareLoop` and
  `DoDMAPlanesLoop` do not (`ps4.asm:120957-121058`). `SceneOp::Wait` represents
  the first; `WaitFrames` represents the VInt-only wait. Counts are already
  DBF-inclusive in the existing transcriptions.
- Actor overlap/walk loops call updates after objects and camera commits
  (`ps4.asm:121117-121140,121256-121291`). `Event_MoveCamera` instead writes its
  step words, calls updates, then commits that axis and updates objects
  (`ps4.asm:121484-121551`). The current scene-camera glide abstraction does not
  expose those per-axis step-word writes or the synchronous loop's exact caller
  cadence. It dispatches updates for modeled Wait/Actor/boarding-camera blocks,
  not from a guessed parallel-glide timer. General camera-glide fidelity remains
  a pre-existing input gap, not a reason to invent a second pan model.
  Motion-loop ownership includes the final arrival/completion frame, even when
  that frame advances into dialogue or ends the runner. The runtime latches
  that ownership before stepping the runner; a core regression covers both.
- The ending's Start wait updates objects and VInt only; no map dispatcher
  (`loc_7A228`, `ps4.asm:159093-159099`).
- Ice-block helpers have extra immediate calls (`ps4.asm:91380,91506`). The
  native runtime has no retail ice-block sub-loop; no retail map uses collision
  type `$B` in the stored pack. These extra call sites are not modeled here.

The runtime separates the ordinary camera latch from commit so the map update
can change the same signed 16.16 counters. Disabled `$EC25/$EC26` driver gates
return before `clr.l`, preserving seeded counters (`ps4.asm:89548-89711`); a
regression test protects this prerequisite. Vehicle maps currently bind only
palette/no-op programs; their existing camera tick stays unchanged.

## Census

Each row cites a symbol, **retail ROM address**, and its starting line in the
local `reference/ps4disasm/ps4.asm`. Lines orient the reader; bytes establish the
rule. Slots below are zero-based CRAM slots. Palette routines write the RAM
shadow, uploaded by VInt; they do not directly issue VDP register commands.
`Palette_Line_2=$FB20` means hardware line **1**, not hardware line 2.

All entries have **no direct field-object writes** and draw **no RNG** except
`$2A`'s conditional shared `UpdateRNGSeed2`. Conveyor/layout writes can change
terrain under objects; they are not object-slot mutations. Frame is the wrapping
16-bit `Main_Frame_Count`; `ECF2`, `ECF3`, `ECFF` are wrapping bytes. Map load
clears `ECF2/ECF3` (`ps4.asm:107588-107596`); the Edge manager clears `ECFF`
(`ps4.asm:109874-109881`). Bind counts count map-list occurrences, not unique
frames. Dead entries are included, not inferred away.

`$2A` reuses the existing shared `Rng2::with_surrogate`, including the native
runtime's already-ratified replacement for the hardware H/V counter term
([RNG design](../RUNTIME_DESIGN.md#rng-design),
`rust/psiv-core/src/battle/rng.rs`). Its regression proves generator, draw count,
seed ordering and native-model reload value, not bit-identical raster entropy
against a cartridge frame. This lane adds no replacement RNG algorithm.

| ID | Routine | Retail address / source line | State, palette/VDP writes and conditions | RNG | Map bindings | Runtime |
| --- | --- | --- | --- | --- | ---: | --- |
| `$00` | `MapUpdate_NoUpdate` | `0x054A72` / 112955 | No writes; moveq #0,d0 / rts | none | 264 | implemented |
| `$01` | `MapUpdate_MotaWaterPal` | `0x054A76` / 112963 | slots 25,26,27,28; 16-frame absolute clock; (frame & $30) >> 4 | none | 1 | implemented |
| `$02` | `MapUpdate_MotaTownsWaterPal` | `0x054AD0` / 112993 | slots 26,27,28,29; 8-frame absolute clock; (frame & $18) >> 3 | none | 6 | implemented |
| `$03` | `MapUpdate_PiataBasementB2_MonstersPal` | `0x054B2C` / 113023 | slots 25,26,27; 16-frame absolute clock; (frame & $30) >> 4 | none | 1 | implemented |
| `$04` | `MapUpdate_MotaQuicksandPal` | `0x054B7A` / 113052 | slots 10,11,12,13; 8-frame absolute clock; (frame & $18) >> 3 | none | 1 | implemented |
| `$05` | `MapUpdate_ZioFortMoveUpdateRate` | `0x054BD4` / 113082 | BG X/Y longword step -= step ASR 4/4 | none | 2 | implemented |
| `$06` | `MapUpdate_ZioFortTunnels` | `0x054BEC` / 113095 | FG H-scroll at 32-byte strides; BG -X/-2X bands; BG X step ASR 2; queued VRAM H-scroll DMA | none | 2 | implemented |
| `$07` | `MapUpdate_ZioFortMoveUpdateRate2` | `0x054C66` / 113141 | BG X/Y longword step -= step ASR 4/2 | none | 4 | implemented |
| `$08` | `MapUpdate_CaveWaterPal` | `0x054C7E` / 113154 | slots 10,11,12,13; 8-frame absolute clock; (frame & $18) >> 3 | none | 2 | implemented |
| `$09` | `MapUpdate_SoldiersTempleWaterPal` | `0x054CDA` / 113184 | slots 26,27,28,29; 8-frame absolute clock; (frame & $18) >> 3 | none | 2 | implemented |
| `$0A` | `MapUpdate_ValleyMazeWaterPal` | `0x054D36` / 113214 | slots 10,11,12,13; 8-frame absolute clock; (frame & $18) >> 3 | none | 4 | implemented |
| `$0B` | `MapUpdate_StripClubLightsPal` | `0x054D92` / 113244 | slots 26; 8-frame ECF2 preincrement clock; ECF3, 6 phases | none | 1 | implemented |
| `$0C` | `MapUpdate_DezoSnowstormPal` | `0x054DE0` / 113283 | slots 19,20,21,22; 8-frame absolute clock; (frame & $18) >> 3 | none | 1 | implemented |
| `$0D` | `MapUpdate_BioPlantMoveUpdateRate` | `0x054E3A` / 113313 | BG X/Y longword step -= step ASR 2/2 | none | 24 | implemented |
| `$0E` | `MapUpdate_BioPlantPal` | `0x054E52` / 113326 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases / slots 28,29; 8-frame absolute clock; ECF3, 8 phases | none | 11 | implemented |
| `$0F` | `MapUpdate_MachCntrMoveUpdateRate` | `0x054EF0` / 113374 | BG X/Y longword step -= step ASR 3/3 | none | 8 | implemented |
| `$10` | `MapUpdate_PlateSysMoveUpdateRate` | `0x054F08` / 113387 | BG X/Y longword step -= step ASR 1/1 | none | 1 | implemented |
| `$11` | `MapUpdate_VahFortConveyorBelts` | `0x054F20` / 113400 | Conveyor layout/chunk writes + plane refresh/DMA; temp $0B/$0C; frame & 7; even-frame refresh (missing buffers) | none | 1 | missing input |
| `$12` | `MapUpdate_WpnPlntConveyorBelts` | `0x055094` / 113518 | Conveyor layout/chunk writes + plane refresh/DMA; temp $11/$12; frame & 7; even-frame refresh (missing buffers) | none | 1 | missing input |
| `$13` | `MapUpdate_WreckagePal` | `0x05521C` / 113645 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases | none | 7 | implemented |
| `$14` | `MapUpdate_WreckagePal2` | `0x055270` / 113675 | slots 28,29; 8-frame absolute clock; ECF3, 8 phases | none | 2 | implemented |
| `$15` | `MapUpdate_MachCenterPal` | `0x0552CC` / 113706 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases | none | 22 | implemented |
| `$16` | `loc_55320` | `0x055320` / 113736 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases; event $65 clear,event $61 clear | none | 0 | implemented |
| `$17` | `MapUpdate_PlateSysF1Pal` | `0x055388` / 113767 | slots 28,29; 8-frame absolute clock; ECF2, 6 phases; event $65 clear,event $61 clear; event $DA bypass; stopped literal CRAM writes | none | 4 | implemented |
| `$18` | `MapUpdate_PlateSysF1Pal2` | `0x055410` / 113814 | slots 12,13; 8-frame absolute clock; ECF3, 6 phases; event $65 clear,event $61 clear; event $DA bypass | none | 4 | implemented |
| `$19` | `loc_55484` | `0x055484` / 113853 | slots 12,13; 8-frame absolute clock; ECF3, 8 phases; event $65 clear,event $61 clear; stopped literal CRAM writes | none | 0 | implemented |
| `$1A` | `MapUpdate_ClimCenterPal` | `0x055508` / 113890 | slots 28,29,12,13; 8-frame absolute clock; ECF2, 6 phases | none | 1 | implemented |
| `$1B` | `MapUpdate_ZelanF1Pal` | `0x055580` / 113924 | slots 28,29; 8-frame absolute clock; ECF3, 8 phases | none | 3 | implemented |
| `$1C` | `MapUpdate_NurvusB1Pal` | `0x0555DC` / 113954 | slots 12,13; 8-frame absolute clock; ECF3, 8 phases; event $66 clear | none | 2 | implemented |
| `$1D` | `MapUpdate_NurvusB1Pal2` | `0x055642` / 113987 | slots 28,29; 8-frame absolute clock; ECF2, 6 phases; event $66 clear | none | 1 | implemented |
| `$1E` | `MapUpdate_AirCastlePal` | `0x0556A0` / 114020 | slots 26,28; 8-frame absolute clock; ECF3, 14 phases | none | 31 | implemented |
| `$1F` | `MapUpdate_AirCastlePal2` | `0x055714` / 114050 | slots 27,29; 8-frame absolute clock; (frame & $0) >> 3 | none | 30 | implemented |
| `$20` | `MapUpdate_WreckageEnginePal` | `0x05577E` / 114079 | slots 28,29; 8-frame absolute clock; ECF2, 6 phases; event $40 set | none | 1 | implemented |
| `$21` | `MapUpdate_KuranF2Pal` | `0x0557DE` / 114112 | slots 28,29; 8-frame absolute clock; ECF2, 6 phases | none | 1 | implemented |
| `$22` | `MapUpdate_GaruberkTowerPart4` | `0x055832` / 114142 | Pattern staging + queued tile DMA; temp $15/$17 clear; frame & 7; ECF2 ping-pong 0..2, ECF3 direction (missing buffer) | none | 4 | missing input |
| `$23` | `MapUpdate_GaruberkTowerPal` | `0x0558C8` / 114196 | slots 10,11,12; 4-frame absolute clock; (frame & $C) >> 2 | none | 3 | implemented |
| `$24` | `MapUpdate_GaruberkTowerPal2` | `0x055916` / 114224 | slots 17,18,19,20; 8-frame absolute clock; (frame & $18) >> 3 | none | 2 | implemented |
| `$25` | `loc_55972` | `0x055972` / 114253 | FG X/Y longword step -= step ASR 3/3 | none | 0 | implemented |
| `$26` | `MapUpdate_GaruberkTower` | `0x05598A` / 114265 | ECF2++; 256 H-scroll pairs: FG -X; unsigned sine >> 3; angle +$60; queued VRAM DMA | none | 1 | implemented |
| `$27` | `MapUpdate_PlateSystemPal` | `0x0559E0` / 114296 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases; event $65 clear,event $61 clear; event $DA bypass | none | 1 | implemented |
| `$28` | `MapUpdate_NurvusPal` | `0x055A54` / 114336 | slots 28,29; 8-frame absolute clock; ECF3, 8 phases; event $65 clear | none | 9 | implemented |
| `$29` | `MapUpdate_NurvusPal2` | `0x055ABA` / 114369 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases; event $65 clear | none | 9 | implemented |
| `$2A` | `MapUpdate_RykCrystalsPal` | `0x055B18` / 114402 | ECF2-- wrapping; rest/flash CRAM slots 2,18; zero countdown + frame & 3=0; ECF3 phases 0..3 then RNG2 reload | shared RNG2 (1 on reload) | 1 | implemented |
| `$2B` | `MapUpdate_ZelanKuranSpacePal` | `0x055B80` / 114446 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases | none | 2 | implemented |
| `$2C` | `MapUpdate_LeRoofRoomPal` | `0x055BD4` / 114476 | slots 13; 8-frame absolute clock; ECF2, 14 phases | none | 1 | implemented |
| `$2D` | `MapUpdate_ZelanCanceller` | `0x055C1C` / 114507 | If $F120 chest $0B set, set $F100 event $72; never clears; no inventory/position gate | none | 1 | implemented |
| `$2E` | `MapUpdate_TheEdgePalLine1` | `0x055C36` / 114520 | Clear CRAM slots 0..13 every call; on even frames rotate 5 colors over slots 1..13; ECF2 modulo 13 | none | 9 | implemented |
| `$2F` | `MapUpdate_TheEdgePalLine2` | `0x055C94` / 114567 | Every 4 frames: map palette+$60, 28 records of 14 words; write slot 0 then slots 16..29; ECF3 modulo 28 | none | 1 | implemented |
| `$30` | `MapUpdate_TheEdgePart2_PalLine2` | `0x055CDA` / 114596 | Every 4 frames: map palette+$60, 14 sliding windows at 2-byte stride; slot 0 then 16..29; ECF3 modulo 14 | none | 8 | implemented |
| `$31` | `loc_55D1E` | `0x055D1E` / 114625 | ECFF++; 256 H-scroll pairs: FG -X; signed sine ASR 4 minus $10; angle +4; queued VRAM DMA | none | 8 | implemented |
| `$32` | `loc_55D78` | `0x055D78` / 114657 | Unused: ECFF++; 20 V-scroll pairs: signed sine ASR 5/BG Y; angle +$2C; FG Y integer-word write; queued VSRAM DMA | none | 0 | implemented |
| `$33` | `MapUpdate_StrengthTwChests` | `0x055E26` / 114705 | All $F120 chest $A1,$A2,$A3 set -> $F100 event $D5 set | none | 1 | implemented |
| `$34` | `MapUpdate_CourageTwChests` | `0x055E58` / 114724 | All $F120 chest $A4,$A5 set -> $F100 event $D3 set | none | 1 | implemented |
| `$35` | `MapUpdate_VahFortPal` | `0x055E7E` / 114740 | slots 12,13; 8-frame absolute clock; ECF2, 6 phases; event $BB clear,event $B3 set | none | 4 | implemented |
| `$36` | `MapUpdate_VahFortPal2` | `0x055EEA` / 114776 | slots 28,29; 8-frame absolute clock; ECF3, 8 phases; event $BB clear,event $B3 set | none | 1 | implemented |
| `$37` | `MapUpdate_LadeaTwAirCstlMusic` | `0x055F5E` / 114812 | Write secondary map $EC2A=$FFFF and saved sound byte $ECEC=$A7; no sound-play request | none | 2 | implemented |
| `$38` | `MapUpdate_ClrChestFlag` | `0x055F6E` / 114821 | Clear $F120 chest $A9 every call (actually bound to WeaponPlant_F2) | none | 1 | implemented |
| `$39` | `MapUpdate_RandomBattlesFlag` | `0x055F7C` / 114832 | Random_Battles_Flag=1 iff Character_1 curr_y word > $160, unsigned; otherwise 0 | none | 1 | implemented |
| `$3A` | `MapUpdate_NoUpdate2` | `0x055F98` / 114848 | No writes; moveq #0,d0 / rts | none | 0 | implemented |
| `$3B` | `MapUpdate_NoUpdate3` | `0x055F9C` / 114855 | No writes; moveq #0,d0 / rts | none | 0 | implemented |
| `$3C` | `MapUpdate_NoUpdate4` | `0x055FA0` / 114862 | No writes; moveq #0,d0 / rts | none | 0 | implemented |
| `$3D` | `MapUpdate_NoUpdate5` | `0x055FA4` / 114869 | No writes; moveq #0,d0 / rts | none | 0 | implemented |
| `$3E` | `MapUpdate_NoUpdate6` | `0x055FA8` / 114876 | No writes; moveq #0,d0 / rts | none | 0 | implemented |
| `$3F` | `MapUpdate_NoUpdate7` | `0x055FAC` / 114883 | No writes; moveq #0,d0 / rts | none | 0 | implemented |

`$11` and `$12` (the Vahal Fort and Weapon Plant belt animation) stay the
missing-input programs above. The belts' *carry* (events `$1D..$20`) does not
depend on them: it tests the chunk ids `$A8..$AB` / `$AC..$AF` that the
animation cycles through, and the static layout holds ids in those ranges
([PLATFORMS_AND_BELTS](PLATFORMS_AND_BELTS.md#the-conveyor-belts)).

### Retail quirks retained

- `$1F` masks the already-zero `d0`: the fork's optional frame reload is absent,
  so this phase remains zero (`ps4.asm:114050-114077`, `$055714`).
- `$14/$19/$1B/$1C/$28/$36` read their second column at `+$0C`, even where
  there are eight phases. `$0E` uses `+$10` for its second table. These overlaps
  are resolved from instruction displacements, not repaired to match labels.
- `$24`'s last read is `+$14`, not `+$18` (`$055916`); `$2C` visits fourteen
  bytes of its fifteen-byte table (`$055BD4`). `$2E` clears fourteen words
  **every** call, including odd frames (`$055C36`).
- `$38` is labeled unreferenced in the fork but bound to map `$0C6`
  WeaponPlant_F2 in retail. `$37` writes the saved sound selector, not a request
  to play it. Flag doors are verified physically: test `$057624/$05762E/$057638`
  select `$F100/$F120/$F140`; set starts at `$057666`, clear at `$0576A8`.
  [Flag-bank evidence](MAP_EFFECTS.md#7-discrepancies-against-the-disassembly-clone)
  owns the naming correction.

### Missing modeled inputs

`$11/$12` require writable `Map_Layout_FG` bytes, conveyor chunk definitions,
plane refresh/DMA and `$ECF6` plane flags; `$22` requires the shared `RAM_Start`
pattern staging image and tile DMA. Their exact conditions and effects are in
the census. They are emitted as `unsupported` with the absent subsystem,
visible through `Runtime::map_updates()` and logged by Godot. No partial
counter/layout/tile effects are applied. This is **61 implemented entries,
three explicit missing-input entries**, not complete conveyor/tile-DMA parity.

Horizontal/vertical scroll entries emit exact sparse word writes with the DMA
destination class. Godot's composed-map renderer does not yet draw per-line or
per-column scroll staging records. Their state/record tests pass independently
of any visual claim. Scene glides and extra ice-loop calls have the caller gaps
listed above; this lane does not change their owning modules.

## Extraction and presentation contract

`psiv_tools/map_updates.py` follows the dispatcher's table pointer, validates all
64 zero-return routine boundaries, decodes gates/clocks/ROM table reads, and
binds the unchanged list order. Palette words, sine samples and Edge blob
windows exist only in the ignored pack. The manifest adds `map_updates` census;
each map adds `map_updates`, `map_update_palette` (64 raw CRAM words), and where
needed `map_update_images` (original PNG → added index PNG).

`psiv_tools/map_updates_render.py` retains indexed PNG IDAT and alpha bytes and
replaces only PLTE with an index transport. Equal RGB colors remain distinct
slots. Original PNGs are untouched. Godot maps those slots through the runtime
CRAM shadow, using the measured [RGB565 ramps](COLOR_PIPELINE.md); all phase,
flag, counter, RNG and state decisions live in Rust. The shader is installed
for base, priority-overlay, layout-variant and patch-atlas images. Sprite-sheet
recoloring is not claimed by these composed-map captures.

`psiv-data` rejects malformed recipes/table bounds and missing 64-word palette
shadows when updates are present. Legacy packs with no key retain their old
behavior. A new pack is required to enable #41.

## Verification and fixture limits

- `session_map_updates`: eleven pad-only Session cases cover Canceller's correct
  bank and negatives, Aiedo's direct-ROM eight-frame sequence, paused menu and
  dialogue frames, tower conjunctions, music/chest-clear writes, camera ordering,
  sparse scroll writes, exactly one conditional RNG2 reload, Y boundary and
  explicit missing inputs. The ordinary Canceller chest interaction sets its
  chest bit while the loot window is open, and sets `$72` only after the window
  closes and the field resumes. Tower entry events skip the dispatch frame;
  their subsequent scene update waits perform the write.
- `session_map_updates_palette`: independent counters and wrap, local strip
  duration paused by menu, shutdown/bypass flags, Edge clear/rotate/blob order,
  sliding blob plus signed scroll, and no-op side effects.
- These sessions start with `Session::start(...).new_game()` and use an explicit
  `from_save` placement fixture. After construction all advancement is
  `Session::frame(Pad)`. They prove the ordinary mode/input seam from those
  states, **not** a connected New Game-to-Zelan route or fresh-process CONTINUE.
  The existing save seam stores cell-scaled Y; fixture setup honors it while
  live rules use the corrected `PixelPos::from_cell` coordinate.
- Python controls corrupt a table branch, update index, clock opcode, PNG type,
  original pack bytes, an original collision field and a manifest JSON digest.
  They must be rejected. `psiv-data` also rejects an out-of-CRAM destination,
  an out-of-table phase mask and an absent counter slot before frame execution.
  Opt-in runtime controls use a private temporary pack: removing Zelan's
  dispatcher or rotating Aiedo's water table must make the corresponding test
  fail. Accepted packs and local owner assets are never edited.

Correction ladder: **rung 1**, one runtime dispatcher and typed program/flag-bank
representation with exhaustive execution matches; Godot consumes only the
resulting palette. **Rung 2** guards reject malformed decoded tables and
non-additive pack edits. The existing deterministic pack inventory guard now
counts declared additive index images rather than treating every extra PNG as
an overlay. Its no-overlay check still rejects an unwanted overlay/index image.
