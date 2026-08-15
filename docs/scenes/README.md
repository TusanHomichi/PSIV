# Opening-act scene transcriptions

Transcribed 2026-08-14 from the retail cartridge. These documents are the
evidence base the event-interpreter lane implements from: every scene in the
opening act, disassembled from cartridge bytes, expressed as an ordered
`SceneOp` sequence, with every RAM address, event flag and dialogue reference
resolved to a name.

**Scope**: power-on through leaving Piata with the basement quest done — the
v1 slice `docs/RUNTIME_DESIGN.md` calls the opening act.

## Why these were disassembled and not read

`reference/ps4disasm` is the fallible map everywhere; for scene routines it is
worse than fallible. `ps4.asm` carries **17** `include "script/scenes/<Name>/event.asm"`
lines, and **`script/scenes/` does not exist in the clone at all**. The clone as
checked in cannot assemble (`script/eventptrs.asm`, `script/cutsceneptrs.asm`
and `script/runeventsjmptbl.asm` are missing too), so "build it and diff" was
never an option either.

> **Correction to `docs/EVENT_ENGINE_SCOUT.md`.** The scout records "17 scene
> routines include `script/scenes/*/event.asm` **ungated**". The count of 17 is
> right; the *ungated* part is not. Walking the conditional nesting, only
> **7 of the 17 are ungated**:
>
> `BasementContainers`, `MeetingSaya`, `SuspicionOnPrincipal`, `AfterIgglanova`,
> `gamestart`, `PiataChazAlone`, `AlysFound`.
>
> The other 10 (`BioPlantAlarm`, `GirlsSneakingOut`, `AlshlineFound`,
> `ChazHouse`, `ZemaIgglanovaDefeated`, `RuneFlaeli`, `Alshline`,
> `MeetingRika`, `FortuneTeller`, `AfterFortuneTeller`) sit in an
> `if grand_cross=1 … else … endif`, so their **retail source survives in the
> `grand_cross=0` branch**. That is a materially smaller blast radius than the
> scout implies — but it does not help the opening act at all, because **all
> six** opening-act casualties are in the ungated seven.

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
act genuinely needs. **The interpreter lane implements exactly this union.**

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

## Out of scope (noticed, not transcribed)

- `Cutscene_ProfHolt` (`$8002`, retail `$73F22`–`$73F9C`) — fires from **tree 3**
  entry `$6A`, i.e. after Piata; warps toward Zema/Birth Valley. This is the
  next act's opening, not this one's close.
- `Event_MeetingSaya` (`$0D`) / `RunEvent_MeetingSaya` (`$16`) and the unused
  `RunEvent_MeetingSayaUnused` (`$09`) — Zema, past the boundary. Note the
  `Unused` one is genuinely unreferenced: it appears in `RunEventsJmpTbl` at
  index `$09` but no map's event list contains `$09`.
- `Event_MachineCenterAppearing` (`$06`) — trigger `$06` requires
  `EventFlag_Zio`, far past this act.
- The remaining 11 scene includes. Only one of them, **`MeetingSaya`**, is
  ungated and therefore needs the same cartridge-only treatment as the opening
  act's six. The other 10 (`BioPlantAlarm`, `GirlsSneakingOut`, `AlshlineFound`,
  `ChazHouse`, `ZemaIgglanovaDefeated`, `RuneFlaeli`, `Alshline`,
  `MeetingRika`, `FortuneTeller`, `AfterFortuneTeller`) keep retail source in
  their `grand_cross=0` branch — still worth byte-verifying against the
  cartridge before transcribing, but not archaeology from scratch.
