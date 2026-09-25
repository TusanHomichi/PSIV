# Retail scene transcriptions

**Research coverage is not connected-play coverage.** These records extend
through the ending, while the verified connected campaign is still in the
BioPlant. Use [the roadmap](../ROADMAP.md) and [playability ledger](../campaign/NATIVE_PLAYABILITY.md)
for current progress; an individual scene fixture does not close a campaign milestone.

Opening-act material was transcribed 2026-08-14 from the retail cartridge;
the next-arc records below were added 2026-08-16, with the Dezo campaign
continuation added in the same wave. These documents are the
retail research behind the implemented event interpreter: every scene in the
opening act, disassembled from cartridge bytes, expressed as an ordered
`SceneOp` sequence, with every RAM address, event flag and dialogue reference
resolved to a name.

**Opening-act scope**: power-on through leaving Piata with the basement quest
done. This names a transcription group, not the current project boundary.

**Next-arc scope**: the scene set from Piata's post-gate Professor Holt beat
through Zema, the Tonoe road, the BioPlant escape, the Rika hand-off, Zio's
fall, the Zelan spaceship route, Dezolis/Raja arrival and the first Kuran/Dark
Force handoff. The Dezo campaign continuation runs from Le Roof through the
Reunion gate and the Profound Darkness battle request. The trigger/map census is
[12_ArcTriggerCensus](12_ArcTriggerCensus.md); the per-scene records are
[13_ProfHolt](13_ProfHolt.md) through [50_JuzaDefeated](50_JuzaDefeated.md).
The native registry and headless proof live beside those documents in
`psiv-core` and `psiv-runtime`.

## Next-arc registry

| Scene | Event | Doc | Retail bytes |
|---|---:|---|---|
| `Cutscene_ProfHolt` | `$8002` | [13](13_ProfHolt.md) | `$073F22..$073F9B` |
| `Cutscene_MeetingRune` | `$8003` | [14](14_MeetingRune.md) | `$073F9C..$074033` |
| `Event_MeetingDorin` | `$32` | [15](15_MeetingDorin.md) | `$06EEBE..$06F1A3` |
| `Cutscene_Dorin` | `$8004` | [16](16_Dorin.md) | `$074034..$0741E5` |
| `Event_RuneFlaeli` | `$27` | [17](17_RuneFlaeli.md) | `$06D7C2..$06DBD7` |
| `Event_AlshlineFound` | `$28` | [18](18_AlshlineFound.md) | `$06DBD8..$06DBE7` |
| `Cutscene_Alshline` | `$8005` | [19](19_Alshline.md) | `$0741E6..$074555` |
| `Cutscene_ZemaIgglanovaDefeated` | `$8006` | [20](20_ZemaIgglanovaDefeated.md) | `$074556..$0745DD` |
| `Event_ZemaServantBattle` | `$8A` | [21](21_ZemaServantBattle.md) | `$07311E..$07313D` |
| `Event_ZemaOldMan` | `$8B` | [22](22_ZemaOldMan.md) | `$07313E..$073165` |
| `Event_ZemaOldManAfterMission` | `$8C` | [23](23_ZemaOldManAfterMission.md) | `$073166..$07318D` |
| `Event_MeetingSaya` | `$0D` | [24](24_MeetingSaya.md) | `$06BC6A..$06BE77` |
| `Event_TonoeBasementDoor` | `$33` | [25](25_TonoeBasementDoor.md) | `$06F1A4..$06F2E9` |
| `Event_BioPlantAlarm` | `$12` | [26](26_BioPlantAlarm.md) | `$06C27C..$06C2BB` |
| `Event_GirlsSneakingOut` | `$23` | [27](27_GirlsSneakingOut.md) | `$06D4A6..$06D615` |
| `Event_ChazHouse` | `$3B` | [28](28_ChazHouse.md) | `$06FA34..$06FACB` |
| `Event_LeavingChazHouse` | `$3C` | [29](29_LeavingChazHouse.md) | `$06FACC..$06FAD3` |
| `Cutscene_MeetingRika` | `$8007` | [30](30_MeetingRika.md) | `$0745DE..$074A7D` |
| `Cutscene_DemiRescue` | `$8008` | [31](31_DemiRescue.md) | `$074A7E..$074B71` |
| `Cutscene_AlysWounded` | `$8009` | [32](32_AlysWounded.md) | `$074B72..$0751FF` |
| `Event_GettingLandRover` | `$2B` | [33](33_GettingLandRover.md) | `$06DEBE..$06E0E9` |
| `Event_MachineCenterAppearing` | `$06` | [34](34_MachineCenterAppearing.md) | `$06B4B2..$06B6F3` |
| `Event_RuneLadaeTower` | `$2E` | [35](35_RuneLadeaTower.md) | `$06E930..$06EA15` |
| `Event_PsycoWandChest` | `$2F` | [36](36_PsycoWandChest.md) | `$06EA16..$06EC61` |
| `Cutscene_PsycoWand` | `$800A` | [37](37_PsycoWand.md) | `$075200..$075A11` |
| `Event_ZioNurvus` | `$34` | [38](38_ZioNurvus.md) | `$06F2EA..$06F439` |
| `Cutscene_ZioDefeated` | `$800B` | [39](39_ZioDefeated.md) | `$075A12..$075FC7` |
| `Cutscene_MeetingWren` | `$800C` | [40](40_MeetingWren.md) | `$075FC8..$07606B` |
| `Cutscene_InsideSpaceship` | `$800D` | [41](41_InsideSpaceship.md) | `$07606C..$07607D` |
| `Cutscene_SpaceshipSabotage` | `$800E` | [42](42_SpaceshipSabotage.md) | `$07607E..$076589` |
| `Cutscene_CrashLaanding` | `$800F` | [43](43_CrashLanding.md) | `$07658A..$07714F` |
| `Cutscene_Landale` | `$8010` | [44](44_Landale.md) | `$077150..$0771D1` |
| `Event_KuranArrival` | `$3D` | [45](45_KuranArrival.md) | `$06FAD4..$06FAE5` |
| `Event_NearDarkForce1` | `$3E` | [46](46_NearDarkForce1.md) | `$06FAE6..$06FAF7` |
| `Event_DarkForce1` | `$3F` | [47](47_DarkForce1.md) | `$06FAF8..$06FB1D` |
| `Cutscene_DarkForce1Defeated` | `$8011` | [48](48_DarkForce1Defeated.md) | `$0771D2..$07734B` |
| `Event_Juza` | `$40` | [49](49_Juza.md) | `$06FB1E..$06FB5D` |
| `Event_JuzaDefeated` | `$41` | [50](50_JuzaDefeated.md) | `$06FB5E..$06FBED` |

## Dezo campaign registry

Wave 6 continues from the Dark Force 1 handoff in retail dispatch order. The
records below are the `grand_cross=0` bodies selected by the retail pointer
tables; the clone's hack-only bodies are not evidence.

| Scene | Event | Doc | Retail bytes |
|---|---:|---|---|
| `Event_MeetingLeRoof` | `$48` | [51](51_MeetingLeRoof.md) | `$070482..$07069D` |
| `Cutscene_LeRoofAgain` | `$801C` | [52](52_LeRoofAgain.md) | `$078346..$0784B5` |
| `Event_CarnivorousTrees` | `$4C` | [53](53_CarnivorousTrees.md) | `$070774..$070855` |
| `Event_SavingKyra` | `$4D` | [54](54_SavingKyra.md) | `$070856..$070975` |
| `Cutscene_MeetingKyra` | `$8013` | [55](55_MeetingKyra.md) | `$077788..$077895` |
| `Event_EclipseTorchUsed` | `$47` | [56](56_EclipseTorchUsed.md) | `$07018A..$070481` |
| `Event_DarkForce2` | `$4E` | [57](57_DarkForce2.md) | `$070976..$0709A1` |
| `Cutscene_LutzRevelation` | `$8014` | [58](58_LutzRevelation.md) | `$077896..$077A2D` |
| `Cutscene_DarkForce2Defeated` | `$8017` | [59](59_DarkForce2Defeated.md) | `$077BDA..$077DC5` |
| `Cutscene_MeetingSeth` | `$8019` | [60](60_MeetingSeth.md) | `$077EAC..$077F2D` |
| `Cutscene_AeroPrism` | `$801A` | [61](61_AeroPrism.md) | `$077F2E..$07818D` |
| `Event_DarkForce3Defeated` | `$50` | [62](62_DarkForce3Defeated.md) | `$070A2A..$070A4D` |
| `Event_ReshelBattle` | `$53` | [63](63_ReshelBattle.md) | `$070A9E..$070ABB` |
| `Event_ClmCenterForcedBattle` | `$54` | [64](64_ClmCenterForcedBattle.md) | `$070ABC..$070AD9` |
| `Event_ClmCenterAfterBattle` | `$55` | [65](65_ClmCenterAfterBattle.md) | `$070ADA..$070AEB` |
| `Event_DElmLars` | `$56` | [66](66_DElmLars.md) | `$070AEC..$070B0B` |
| `Event_AfterDElmLarsBattle` | `$57` | [67](67_AfterDElmLarsBattle.md) | `$070B0C..$070B1D` |
| `Cutscene_FindingAirCastle` | `$8015` | [68](68_FindingAirCastle.md) | `$077A2E..$077A67` |
| `Event_AirCastleArrival` | `$58` | [69](69_AirCastleArrival.md) | `$070B1E..$070B2F` |
| `Event_XeAThoulBeforeBattle` | `$59` | [70](70_XeAThoulBeforeBattle.md) | `$070B30..$070B55` |
| `Event_AirCastleFakeChest` | `$5A` | [71](71_AirCastleFakeChest.md) | `$070B56..$070C2F` |
| `Event_LashiecAppearance` | `$5D` | [72](72_LashiecAppearance.md) | `$070CB4..$070E4D` |
| `Cutscene_LashiecDefeated` | `$8016` | [73](73_LashiecDefeated.md) | `$077A68..$077BD9` |
| `Cutscene_GumbiousBishop` | `$8018` | [74](74_GumbiousBishop.md) | `$077DC6..$077EAB` |
| `Event_StrengthTowerTop` | `$5E` | [75](75_StrengthTowerTop.md) | `$070E4E..$071101` |
| `Event_CourageTowerTop` | `$5F` | [76](76_CourageTowerTop.md) | `$071102..$071449` |
| `Event_DeVars` | `$60` | [77](77_DeVars.md) | `$07144A..$071469` |
| `Event_SaLews` | `$61` | [78](78_SaLews.md) | `$07146A..$071489` |
| `Event_ReFaze` | `$63` | [79](79_ReFaze.md) | `$07157A..$071759` |
| `Event_DeVarsDefeated` | `$64` | [80](80_DeVarsDefeated.md) | `$07175A..$071821` |
| `Event_SaLewsDefeated` | `$65` | [81](81_SaLewsDefeated.md) | `$071822..$0718E5` |
| `Cutscene_BeforeElsydeonCave` | `$801D` | [82](82_BeforeElsydeonCave.md) | `$0784B6..$078583` |
| `Cutscene_Elsydeon` | `$801E` | [83](83_Elsydeon.md) | `$078584..$078A7B` |
| `Cutscene_Reunion` | `$801F` | [84](84_Reunion.md) | `$078A7C..$078D2F` |
| `Event_AngerTowerTop` | `$69` | [85](85_AngerTowerTop.md) | `$0721CC..$072261` |
| `Event_AngerTowerExitTop` | `$6A` | [86](86_AngerTowerExitTop.md) | `$072262..$0722D1` |
| `Cutscene_ProfoundDarkness` | `$8020` | [87](87_ProfoundDarkness.md) | `$078D30..$078F3D` |

## Terminal and recorded boundary surfaces

| Scene | Event | Doc | Retail bytes |
|---|---:|---|---|
| `Cutscene_RajaSick` | `$8012` | [88](88_RetailBoundaries.md) | `$07734C..$077787` |
| `Cutscene_Rykros` | `$801B` | [88](88_RetailBoundaries.md) | `$07818E..$078345` |
| `Cutscene_Ending` | `$8021` | [89](89_Ending.md) | `$078F3E..$07A811` |

The complete retail table census, including the input-driven surfaces that
remain precise why-not boundaries, is in
[12_ArcTriggerCensus](12_ArcTriggerCensus.md). The persistent headless proof
is `rust/psiv-runtime/tests/next_arc.rs`: it runs title through the `$8020`
battle request and the `$8021` ending, asserting the relevant flags and party
roster at each handoff. `rust/psiv-runtime/tests/retail_boundaries.rs` covers
the deterministic Raja Sick and Rykros records independently.

## Why these were disassembled and not read

`reference/ps4disasm` is the fallible map everywhere; for scene routines it is
worse than fallible. `ps4.asm` carries **17** `include "script/scenes/<Name>/event.asm"`
lines, and **`script/scenes/` does not exist in the clone at all**. The clone as
checked in cannot assemble (`script/eventptrs.asm`, `script/cutsceneptrs.asm`
and `script/runeventsjmptbl.asm` are missing too), so "build it and diff" was
never an option either.

> **Correction to `docs/scenes/EVENT_ENGINE_SCOUT.md`.** The scout records "17 scene
> routines include `script/scenes/*/event.asm` **ungated**". The count of 17 is
> right; the *ungated* part is not. Walking the conditional nesting, only
> **7 of the 17 are ungated**:
>
> `BasementContainers`, `MeetingSaya`, `SuspicionOnPrincipal`, `AfterIgglanova`,
> `gamestart`, `PiataChazAlone`, `AlysFound`.
>
> Eight (`BioPlantAlarm`, `GirlsSneakingOut`, `AlshlineFound`, `ChazHouse`,
> `ZemaIgglanovaDefeated`, `RuneFlaeli`, `Alshline`, `MeetingRika`) sit in an
> `if grand_cross=1 … else … endif`, so their **retail source survives in the
> `grand_cross=0` branch**. `FortuneTeller` and `AfterFortuneTeller` are
> different: their labels and Grand Cross includes are inside `if grand_cross=1`,
> with no retail `else` body and no retail pointer slots. They cannot be
> transcribed from retail bytes without inventing a scene.

For the ungated seven the clone does not contain a fork *rewrite* of retail
behaviour; it contains *no* behaviour — an entry label, an `include` of an
absent file, and an `rts`. Degrees of damage vary:

- **Nothing survives**: `PiataChazAlone`, `SuspicionOnPrincipal`,
  `BasementContainers`.
- **Prologue and epilogue survive, the middle is gone — and the survivors are
  themselves edited**: `gamestart` (three Grand Cross edits to the start
  position and facing in its epilogue — see [01](01_GameStart.md)) and
  `AlysFound` (two behavioural edits — see [03](03_AlysFound.md)). Both cases
  are the dangerous shape: code that *looks* like retail and is not.
- **Body survives as a `;`-commented block**: `AfterIgglanova` — dead code, but
  it matched the cartridge line for line and served as an independent check.

Two more opening-act scenes are fork-damaged in other ways:
`Event_PrincipalConfession` is commented out to a bare `rts`, and
`Event_PiataGuardsReprimand` is rewritten in place (and the rewrite reads the
wrong dialogue tree). See the per-scene "Fork status" sections.

## Method

Everything below was derived from the cartridge; the clone contributed only
*names* (RAM equates, struct field offsets, event-flag numbers) and, where its
code was proven byte-identical to retail, corroboration.

1. **Anchors from cartridge data, not from the clone.** `EventPtrs` (161 `dc.l`)
   and `CutscenePtrs` (34 `dc.l`) are retail *data*; entry N's pointer is
   authoritative for whatever the clone calls entry N, no matter what the clone
   did to that routine's body. `EventPtrs` was located at **`$5A2B4`** by
   hand-decoding `FieldRoutine_Event`/`loc_5A27A` and confirming the table
   reads as in-range ROM pointers; `CutscenePtrs` at **`$5A580`** by scanning
   forward for a 34-entry table whose entry 0 equals `EventPtrs[0]`.
   `RunEventsJmpTbl` was located at **`$560E8`** by scanning for the unique run
   of exactly 128 `bra.w` opcodes (`$6000`) at a 4-byte stride.
2. **Disassembly.** Capstone `CS_ARCH_M68K` / `CS_MODE_M68K_000` over the ROM,
   with a small annotator that resolves `$xxxx.w` operands against the clone's
   `ramaddr()` equates, `$xx(aN)` displacements against its object-struct
   offsets, and absolute-long call targets against a symbol map built as below.
3. **Symbol map, bootstrapped and self-checked.** Routine addresses were learned
   by aligning retail instructions against clone source lines *only where the
   mnemonic stream agrees*, so a rewritten body cannot inject a bogus name. A
   walker that assigns addresses to clone labels self-checks against every
   `loc_XXXXXX:` label (those literally encode ROM addresses) and refuses to
   record anything from a region whose anchors disagree — which is how the drift
   around `Event_RunDialogue` was caught rather than propagated.
4. **Semantics from bytes.** Where a name was uncertain or absent, the routine
   itself was disassembled and read (`trap #0`/`#1` handlers via the vector
   table at `$80`, `DoMapUpdateLoop`, `Event_MoveCamera`, `Event_GetCharacter`,
   the NPC movement-command table). Names that the clone's *commented-out*
   retail bodies later confirmed are noted as such.

The scratch tooling lives outside the repo (session scratchpad):
`psdis.py` (annotated disassembler), `align.py` (retail↔clone aligner),
`walk.py` (self-checking label walker), `bootstrap_syms.py`, `namehunt.py`.

## Facts derived here that the interpreter needs

- **`trap #0`** clears `d7+1` **longwords** at `(a0)+`; **`trap #1`** copies
  `d7+1` **words** from `(a0)+` to `(a1)+`. Decoded from the handlers at
  `$202`/`$20C` (vectors at `$80`/`$84`). One field-object struct is `$40`
  bytes, so `#$F,trap #0` clears exactly one object and `#$1F,trap #1` copies
  exactly one.
- **Map NPC index N lives at field-object RAM `$FFFFC300 + N*$40`.** Proven
  twice: `Hahn_Near_Basement = $FFFFC300` is NPC index 0 of map `$12`, and
  `Alys_Piata = $FFFFC4C0` is `$C300 + 7*$40`, NPC index 7 of map `$13`.
  The clone's `Field_Obj_Secondary` is the same address as
  `Hahn_Near_Basement` — it is simply "NPC slot 0 of the current map".
- **`Map_Start_X_Pos`/`Map_Start_Y_Pos` are in 8-pixel units**; the loader does
  `lsl.w #3` before storing to `curr_x_pos`/`curr_y_pos` (`loc_535D4`).
  Trigger position tests, by contrast, compare `curr_*_pos` in **pixels**.
- **A dialogue id is an entry index into the *current map's* tree.**
  `GetDialogueByID` (`$59164`) is `lea $FFFF3000.l,a0` then "skip d0 `$FF`
  terminators"; the map's bound tree is what was decompressed to `$FFFF3000`.
  A scene that wants a different tree must call `DialogueTreesToRAM` (`$53F00`)
  first — `Event_GameStart` and `Event_PiataGuardsReprimand` both do.
- **`RunText` always records where it stopped** in `Saved_Dialogue_Addr`
  (`$FFFFECF0`, a word, sign-extended to `$FFFFxxxx`), at `$6A12C` and
  `$6AD82`. That is the whole mechanism behind the clone's `popdlg` macro and
  behind `RunDialogueResume` below.

## SceneOp vocabulary

Base vocabulary from `docs/RUNTIME_DESIGN.md`, plus the extensions the opening
and next acts genuinely need. **The interpreter lane implements exactly this
union.**

### Base (used as designed)

| Op | Retail primitive | Notes |
|---|---|---|
| `Face{who, dir}` | `Event_UpdateObjFacing $5A936` | `dir`: 0 down, 4 up, 8 right, `$C` left |
| `RunDialogue{tree, entry}` | `Event_GetAndRunDialogue $5AC66` | `tree` is the map's bound tree unless a `SetDialogueTree` precedes |
| `SetFlag{flag}` | `EventFlags_Set $57666` | 174-flag space |
| `JoinParty{char, slot}` | `Event_AddMacro $63B82` + `Current_Party_Slot_*` | |
| `DespawnNpc{who}` | `clr.w (npc)` + `trap #0` | |
| `MoveCamera{x, y, speed}` | `Event_MoveCamera $5AAEE` | pixels; routine subtracts `$98`/`$58` for the half-screen offset |
| `Wait{ticks}` | `DoMapUpdateLoop $5A71E` | **`ticks = d0 + 1`** (`dbra`) |
| `BranchFlag{flag, …}` | `EventFlags_Test $57624` | only `Event_IgglanovaBattle` uses one in this act |
| `MoveActor{who, cmd, wait}` | NPC field routine + spin | see extension note below |

`BranchChoice` is **not used by any opening-act scene**. Every branch in this
act is either a flag test in a trigger, or a `$FA` flag chain inside the
dialogue tree (which the dialogue system owns, not the scene interpreter).

### Extensions (new — all required)

| Op | Retail primitive | Used by |
|---|---|---|
| `PlaySound{id}` | `move.b #id, (Sound_Index).l` | almost every scene; music, SFX and `$FB` stop-music share one byte |
| `SetSavedMusic{id}` | `move.b #id, (Saved_Sound_Index).w` | GuardsReprimand, GameStart |
| `FadeIn{}` / `FadeOut{}` | `Pal_FadeIn $421D4` / `PalFadeOut_ClrSpriteTbl $4223E` | GameStart, PiataPrincipal, GuardsReprimand |
| `LoadMap{map, prev_map, start_x, start_y, facing, align}` | `Field_Map_Index`/`Map_Start_*`/`Map_Load_Flags` + `RefreshMap $5AE98` | GameStart (×4), GuardsReprimand |
| `SetDialogueTree{rom_addr}` | `DialogueTreesToRAM $53F00` | GameStart, GuardsReprimand |
| `RunDialogueResume{}` | `popdlg` + `Event_RunDialogue $5AC6C` | AfterIgglanova (×2) |
| `MoveActorTo{who, x, y}` | `Event_MoveCharacters $5AA84` (party, follows) / `Event_MoveSingleObject $5A9FC` (one object) | GameStart, AfterIgglanova |
| `GetCharacter{char_id} -> actor` | `Event_GetCharacter $5A6D6` | AfterIgglanova; resolves a **character id** to its field object via the party slots, so ops can name `Chaz`/`Alys`/`Hahn` instead of a slot |
| `OverlapCharacters{}` | `Event_OverlapCharacters $5A87A` | AfterIgglanova; collapses followers onto the leader |
| `SetFollowMode{bits}` | `Char_Move_Flags $ECFE` bits 0/1/2 | GameStart |
| `SetStepOffset{v}` | `FieldObj_Step_Offset $ECE0` | AfterIgglanova |
| `PromoteNpcToChar{npc, char_id, slot, art_tile, facing}` | `trap #1` struct copies + field-object construction | AlysFound, MeetingHahn |
| `SetPartySlots{value}` | `Current_Party_Slots $F40A` (word or long) | AlysFound, MeetingHahn, GameStart |
| `AddMoney{amount}` | `addi.l #n, (Current_Money).w` | MeetingHahn (+100), PrincipalConfession (+300) |
| `StartBattle{event_battle_index}` | `Event_Battle_Index $ECFC` + `bset #3,(Routine_Exit_Flags)` | IgglanovaBattle |
| `SetRenderSpritesInCutscene{b}` | `Render_Sprites_In_Cutscenes $ECFD` | PiataPrincipal |

### Next-arc extensions

| Op | Retail primitive | Used by |
|---|---|---|
| `MoveActorToActor{who, target}` | copy a live object's position before `Event_MoveObjectStatic` | Tonoe basement door |
| `PanelCreate{id}` / `PanelDestroy{id}` / `DmaPlanes` | `Panel_Create`, `Panel_Destroy`, `DMAPlanes_VInt` | Alshline |
| `LoadArt{rom_addr, tile}` | `LoadVRAMAddressFromTileNumber` + `NemDecomp` | RuneFlaeli, Alshline |
| `ObjectAnimation{slot, object_id, art_tile, frames}` | temporary field-object construction and keyframe loops | MeetingDorin, RuneFlaeli, Alshline, Saya |
| `RemoveItem{item}` | `GetItem` + clear slot + `ReorderInventory` | Alshline |
| `SetMapLoadFlags{set, clear}` | raw `Map_Load_Flags` writes around `RefreshMap` | Dorin, Alshline, servant battle |
| `RecoverStats{}` | `RecoverStats` | Alshline, PsychoWand |
| `MoveActorToActorAxis{who, target, axis}` | copy one live coordinate before `Event_MoveSingleObject` | MeetingRika |
| `MoveActorOffset{who, dx, dy}` | read a character's current position, add offsets, move | MeetingRika |
| `Presentation{op}` | typed VDP/window/palette/temp-object record | BioPlantAlarm, GirlsSneakingOut, ChazHouse, MeetingRika |

Post-Rika state extensions are deliberately small and state-bearing:
`SetVehicleIndex`, `AddItem`, `ConfigureCharacter`, `RestorePartyHp`,
`RemovePartyMember`, `ClearCharacterStatus` and `ReviveIfDead`. They are used
by the new records rather than hidden in runtime-specific scene names.

### Dezo campaign extensions

| Op | Retail primitive | Used by |
|---|---|---|
| `BranchIfVehicle{if_mounted, if_on_foot}` | `Vehicle_Index` test around mounted event bodies | Carnivorous Trees, Saving Kyra, Eclipse Torch |
| `SetCharacterEquipment{who, slots}` | retail partial equipment write | Elsydeon |
| `SavePartySlots` / `RestorePartySlots` | transient `Saved_Char_ID_Mem_1/_5` bridge | Before Elsydeon, Elsydeon, Anger Tower |

`PanelCreate` and `PanelDestroy` carry the full retail panel word, not a byte;
Dezo panels such as `$10D`, `$13F` and `$18F` are the proof cases. The saved
party bridge is runtime-only and is deliberately absent from `StateSnapshot`
and save serialization. The headless arc test asserts the party, flags,
inventory, vehicles and equipment at each state-bearing edge.

`LoadMap` is now a blocking op: the runtime responds with `MapLoaded` only
after it has rebuilt and recast the new map. NPC `ActorMoveStarted` and
`ActorArrived` edges update `FieldMap`, which is the authoritative landing
state used by the comparator.

### Intro-only extensions

`Event_GameStart` also drives a bespoke title/prologue presentation that no
other opening-act scene uses. These are listed for completeness and are
arguably renderer-owned rather than interpreter-owned; see
[01_GameStart.md](01_GameStart.md) for the argument.

`LoadPalette{rom_addr}`, `SetCameraPos{x,y}`, `DrawTextToPlane{entry, plane_addr, vram_addr}`,
`WaitFrames{n}` (`VInt_PrepareLoop $5A7AC` — a *different* wait from `Wait`:
no map update runs), `IntroTextFadeUp{}` / `IntroTextFadeDown{}` (the
`$222`-per-3-frames colour ramp at `$73E86`/`$73EA4`).

## Documents

Cross-checked against the new-game extraction lane where they overlap:
`psiv_tools/newgame.py` and `runtime-pack/game_start.json` own the title
handoff and `Event_GameStart`'s epilogue, and this set cites them rather than
re-deriving. We agree on the routine start, on `rom_end = $073ECE`, on the
mid-scene party `[Alys, Chaz]` at `$073A24`, and on the three fork edits in the
epilogue. One framing correction is recorded in [01](01_GameStart.md): map `$11`
is the map *resident when the scene starts*, not a map the scene plays on.

| # | Scene | Event id | Retail | Fork status |
|---|---|---|---|---|
| [00](00_triggers.md) | Trigger table | — | `$560E8` | clean, spot-verified |
| [01](01_GameStart.md) | `Event_GameStart` | `$9F` | `$73946`–`$73ECE` | **body deleted, epilogue edited** |
| [02](02_PiataChazAlone.md) | `Event_PiataChazAlone` | `$A0` | `$73ECE`–`$73EE0` | **body deleted** |
| [03](03_AlysFound.md) | `Event_AlysFound` | `$03` | `$6B2D8`–`$6B3BA` | **partly deleted + 2 edits** |
| [04](04_PiataPrincipal.md) | `Cutscene_PiataPrincipal` | `$8001` | `$73EE0`–`$73F22` | clean |
| [05](05_SuspicionOnPrincipal.md) | `Event_SuspicionOnPrincipal` | `$0F` | `$6BEE8`–`$6BEF8` | **body deleted** |
| [06](06_MeetingHahn.md) | `Event_MeetingHahn` | `$04` | `$6B3BA`–`$6B4B0` | clean |
| [07](07_BasementContainers.md) | `Event_BasementContainers` | `$0C` | `$6BC36`–`$6BC6A` | **body deleted** |
| [08](08_IgglanovaBattle.md) | `Event_IgglanovaBattle` | `$6B` | `$722D2`–`$722F6` | clean |
| [09](09_AfterIgglanova.md) | `Event_AfterIgglanova` | `$25` | `$6D6C8`–`$6D784` | **body deleted** (commented copy survives) |
| [10](10_PrincipalConfession.md) | `Event_PrincipalConfession` | `$26` | `$6D784`–`$6D7C2` | **commented out to `rts`** |
| [11](11_PiataGuardsReprimand.md) | `Event_PiataGuardsReprimand` | `$9E` | `$738D2`–`$73946` | **rewritten in place** |

## Story order

```
power-on
  ── title screen
       loc_4335C   $04335C  Field_Map_Index = $11 (pre-scene default only)
       loc_44414   $044414  new-game init  (see psiv_tools/newgame.py)
       Title_StartOption $043788  Game_Mode_Routine = $C, Event_Index = $9F
  └─ Event_GameStart ($9F)              sets EventFlag_PiataFirstTime ($07)
       plays on maps $5E ChazHouse -> $54 Aiedo -> $00 Motavia -> title image
       lands on map $13 PiataAcademy_F1
  └─ RunEvent_PiataChazAlone ($7C)      flag $15 clear
       └─ Event_PiataChazAlone ($A0)    sets EventFlag_PiataChazControl ($15)
  └─ RunEvent_FindingAlys ($03)         flag $08 clear, at x=$260 y>=$F0
       └─ Event_AlysFound ($03)         Alys joins and LEADS; sets flag $08
  ── talk to principal, map $14         tree 33 entry $00 -> flag $08 -> entry $15
       └─ Cutscene_PiataPrincipal       sets EventFlag_PrincipalMeeting ($09)
  └─ RunEvent_SuspicionOnPrincipal ($0A) flag $09 set, flag $0E clear
       └─ Event_SuspicionOnPrincipal    sets EventFlag_PrincipalSuspicious ($0E)
  ── talk to Hahn, map $12              tree 33 entry $10 -> flag $09 -> entry $14
       └─ Event_MeetingHahn ($04)       Hahn joins slot 3, +100; sets flag $0A
  └─ RunEvent_BasementContainers ($08)  map $17, flag $0D clear
       └─ Event_BasementContainers      sets EventFlag_BasementContainers ($0D)
  ── step on interaction area (30,18)   Interaction_GetEvent param $0B
       └─ Event_IgglanovaBattle ($6B)   sets EventFlag_Igglanova ($0B), battle
  └─ RunEvent_AfterIgglanova ($13)      map $17
       └─ Event_AfterIgglanova ($25)    sets EventFlag_AfterIgglanova ($0F)
  ── talk to principal again            tree 33 entry $00 -> flag $0B -> entry $16
       └─ Event_PrincipalConfession     +300; sets EventFlag_PrincipalConfession ($0C)
  ── leave Piata onto the overworld
       RunEvent_ReenterPiata ($5B)      fires while flag $0C is CLEAR
       └─ Event_PiataGuardsReprimand    bounces you back into Piata  <-- act boundary
```

## Story continuation and boundary

The newly covered continuation is:

```
Zema aftermath
  ├─ BioPlant Part2 `$A3`, trigger `$0C` → Event_BioPlantAlarm `$12`
  │    temp `$08` set; player-controlled BioPlant traversal follows
  ├─ Aiedo Supermarket selector `$06` → Event_GirlsSneakingOut `$23`
  │    event `$46` set; this is a direct shop call, not a map trigger
  ├─ ChazHouse `$5E`, trigger `$26` → Event_ChazHouse `$3B`
  │    temp `$18` set; Aiedo exit trigger `$27` → Event_LeavingChazHouse `$3C`
  └─ BioPlant B4 Part2 `$AC`, trigger `$1A` → Cutscene_MeetingRika `$8007`
       event `$34` set, Rika joins slot 5, event `$35` set, Motavia `$00`
  └─ player-controlled retail traversal
       ├─ Zio Fort F4 `$8B`, `$1C` → Cutscene_DemiRescue `$8008`
       │    Zio `$42` set, battle index 4
       ├─ Zio Fort F4 `$8B`, `$1D` → Cutscene_AlysWounded `$8009`
       │    Hahn/Alys leave, Demi joins, Demi `$47` set
       ├─ Machine Center B1 Part2 `$B9`, `$1B` → Event_GettingLandRover `$2B`
       │    Control Key becomes Land Rover, vehicle 1, Land Rover `$44` set
       ├─ Motavia `$00`, `$06` → Event_MachineCenterAppearing `$0006`
       │    gated by Zio `$42`, then Machine Center `$43`
       ├─ Ladea Tower F2 `$8E`, `$19` → Event_RuneLadaeTower `$002E`
       │    Rune is written to slot 5 and Rune-again `$62` set
       ├─ Ladea Tower F5 `$91`, `$2A` → Event_PsycoWandChest `$002F`
       │    chest/battle setup sets Gy Laguiah `$69`, battle index 5
       ├─ Ladea Tower F5 `$91`, `$1E` → Cutscene_PsycoWand `$800A`
       │    After Alys Death `$63` and `$67` set
       ├─ Nurvus B4 Part2 `$D3`, `$1F` → Event_ZioNurvus `$0034`
       │    Zio Nurvus `$65`, battle index 6
       └─ Nurvus B4 Part2 `$D3`, `$20` → Cutscene_ZioDefeated `$800B`
            Gryz/Demi leave; Gryz Gone `$68`, Mota Spaceport `$66`, Plate Engine `$61`
  └─ player walks to the retail post-Zio maps
       ├─ Zelan F1 / dialogue handoff → Cutscene_MeetingWren `$800C`
       │    Wren joins slot 4; Wren Joined `$70`
       ├─ Mota Spaceport `$BF`, `$21` → Cutscene_InsideSpaceship `$800D`
       │    menu-selected Zelan route: Motavia → Zelan Space → Zelan
       ├─ Zelan `$18D`, `$28` → Cutscene_SpaceshipSabotage `$800E`
       │    Chaos Sorcerer `$71`, battle index 8
       ├─ Zelan Space `$18C`, `$29` → Cutscene_CrashLaanding `$800F`
       │    Dezolis `$001` → Raja Temple `$14C`; Raja joins; `$85/$88`
       ├─ Dezo player walk / Tyler gate → Cutscene_Landale `$8010`
       │    Dezo Spaceport `$82`
       ├─ Kuran `$190`, `$2C` → Event_KuranArrival `$003D`
       │    Kuran `$86`; `$2D/$2E` then set `$87/$83`, battle index 9
       └─ trigger `$32` → Cutscene_DarkForce1Defeated `$8011`
            Canceller out, Ice Digger `$97` in inventory, Ice Digger `$89`
```

The order of the shop and house detours is player-controlled; the census keeps
those dispatch surfaces separate instead of pretending they are one linear
map-event list. The post-Rika rows above are likewise trigger order, not a
claim that the player walks from Motavia to Nurvus without the intervening
field maps.

## Still out of scope (noticed, not transcribed)

- `RunEvent_MeetingSayaUnused` (`$09`) — genuinely unreferenced: it appears
  in `RunEventsJmpTbl` but no map's event list contains `$09`.
- `Event_ZioFortBarrier` (`$30`) and `Event_ZioFanatic` (`$31`) are direct
  `EventPtrs` bodies behind the Zio Fort map-data/interaction path, not entries
  written by the retail `RunEvent_*` chain covered here. Their `grand_cross=0`
  bytes are recorded in the census as direct bodies; they are not silently
  substituted with Grand Cross code.
- `Event_FortuneTeller` and `Event_AfterFortuneTeller`: the retail
  `EventPtrs` table stops at `$A0`, so `$A1/$A2` are not retail event pointers;
  the clone only has their Grand Cross includes. There is no retail byte range
  to transcribe.
- Molcum `$40` has only event index `$00`, and `RunEvent_Null00` returns
  immediately. Its aftermath is map/NPC flag state, not a retail scene body;
  no Molcum cutscene was invented. The later `$50`/`$801F` reunion trigger is
  still outside this slice: it requires Elsydeon `$D9` and is reached after the
  Kuran/Dezo beats covered here.
