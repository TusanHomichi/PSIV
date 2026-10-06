# `Event_EclipseTorchStolen`

- **Retail bytes:** `$06FE56..$070189` inclusive, 820 bytes.
- **Pointer:** `EventPtrs[$46]` at `$05A2B4`; event `$0046`.
- **Trigger:** Dialogue `$F6 $00 $46`, entry 53 of `DialogueTree38` (Gumbious Temple). The monk at
  (39,22) on `Gumbious_F1` `$162` (object 6, dialogue 36) opens it: entry 36 is
  `$FA $98 +1 / $FA $97 +17`, so with Lutz revealed (`$97`) and the torch not yet stolen (`$98` clear)
  the line jumps to entry 53. With `$98` set it speaks entry 37, "That it is lost is...".
- **Data:** `gumbious_events.rs`, `ECLIPSE_TORCH_STOLEN` (58 ops).

## Clone audit

Read from the US image through its `EventPtrs` slot; every instruction word matches
`ps4.asm:149362-149533`. Both `if revision>0` blocks are in the retail bytes (tree 37 at `$06FE56`,
tree 38 at `$070174`), which is why the tree-load census counts two.

## Retail transcription

Object addresses are `$FFFFC300 + slot * $40`: `$C480` is object 6 (the monk), `$C500..$C640`
objects 8 to 13, `$C680..$C740` objects 14 to 17. `Event_UpdateObjFacing` facing codes are 0 up, 4
down, 8 left, 12 right. `DoMapUpdateLoop(n)` and `DoMainUpdatesLoop(n)` are both `Wait(n + 1)` here, as
in [Zio Fort Barrier](90_ZioFortBarrier.md); the thieves' own step constants (`$20`) are not
modelled (the objects are presentation only, as in the other Dezolis scenes), so the second
group of `CreateFieldObject` records carries the figures' resting cells.

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FE56..$06FE61` | `move.l #DialogueTree37 ($1F9580), d0` / `jsr DialogueTreesToRAM` | `SetDialogueTree` |
| - | `$06FE62..$06FE75` | object 6: id word `$8194`, destination := current position | (renderer state, not typed) |
| 1 | `$06FE76..$06FE87` | `move.w $6(a3), d0 / bchg #2, d0 / jsr Event_UpdateObjFacing` on the monk | `FaceOppositeOf` |
| 2 | `$06FE88..$06FE8F` | `moveq #$32, d0` / `jsr Event_GetAndRunDialogue` | `standard(0x32)` |
| 3 | `$06FE90..$06FEA5` | `NemDecomp` of `$1B1D06` to tile `$36E` | `LoadArt` |
| 4-5 | `$06FEA6..$06FEB9` | monk faces down; `DoMapUpdateLoop($1D)` | `Face`, `Wait(30)` |
| 6-10 | `$06FEBA..$06FEF5` | `Character_1..5` face down (five `Event_UpdateObjFacing`) | `Face` x 5 |
| 11 | `$06FEF6..$06FEFD` | `DoMapUpdateLoop($1D)` | `Wait(30)` |
| 12 | `$06FEFE..$06FF05` | `Sound_Index := MusicID_TheBlackBlood ($A8)` | `PlaySound` |
| 13 | `$06FF06..$06FF15` | `Event_MoveCamera` to (`$270`, `$F0`) at speed 1 | `MoveCamera` |
| 14 | `$06FF16..$06FF1D` | `SFXID_Teleport ($DE)` | `PlaySound` |
| 15-20 | `$06FF1E..$06FFB1` | six objects at `$C500..$C640`: ids `$280` x 3 and `$28C` x 3, art `$36E`, cells (`$250,$F0`), (`$270,$E0`), (`$290,$F0`) twice | `CreateFieldObject` x 6 |
| 21 | `$06FFB2..$06FFB9` | `DoMainUpdatesLoop($61)` | `Wait(98)` |
| 22 | `$06FFBA..$06FFC7` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 23 | `$06FFC8..$06FFCF` | teleport SFX | `PlaySound` |
| 24-25 | `$06FFD0..$06FFEF` | `$C500` step 2, destination `$290`; `$C580` step `-2`, destination `$250` | `SetObjectDestination` x 2 |
| 26-33 | `$06FFF0..$070087` | four objects `$C680..$C740`, id `$294`, art `$36E`, steps `$18000`, 1, `$FFFE8000`, `$FFFF`, cells (`$250,$F0`) x 2 and (`$290,$F0`) x 2, destinations `$290`, `$290`, `$24F`, `$250` | `CreateFieldObject`, `SetObjectDestination` x 4 |
| 34 | `$070088..$07008F` | `DoMainUpdatesLoop($0F)` | `Wait(16)` |
| 35 | `$070090..$070099` | `move.w #0, (Field_Obj_Secondary).w` and `($FFFFC340).w`: the torch (object 0) and its invisible block (object 1) leave | `DespawnNpc` 0, 2 |
| 36 | `$07009A..$0700A1` | `DoMainUpdatesLoop($59)` | `Wait(90)` |
| 37 | `$0700A2..$0700AF` | `popdlg` / `Event_RunDialogue` | `RunDialogueResume` |
| 38 | `$0700B0..$0700B7` | teleport SFX | `PlaySound` |
| 39-44 | `$0700B8..$070101` | the six objects take ids `$288` (x 3, `bset #1, $2`) and `$290` (x 3), `$20` cleared on `$C500` and `$C580` | `CreateFieldObject` x 6 |
| 45-46 | `$070102..$070111` | `DoMainUpdatesLoop($55)`, `DoMapUpdateLoop($3B)` | `Wait(86)`, `Wait(60)` |
| 47 | `$070112..$070125` | `Event_MoveCamera` to `Character_1`'s `$30/$34`, speed 1 | `CameraToActor` |
| - | `$070126..$07012D` | object 6 id word back to `$8038` | (renderer state, not typed) |
| 48-51 | `$07012E..$07014D` | `Sound_StopMusic`, `DoMapUpdateLoop(9)`, `MusicID_Suspicion ($9F)`, `DoMapUpdateLoop(0)` | `PlaySound`, `Wait(10)`, `PlaySound`, `Wait(1)` |
| 52 | `$07014E..$07015B` | `popdlg` / `Event_RunDialogue` | `RunDialogueResume` |
| 53-55 | `$07015C..$070173` | `Sound_StopMusic`, `DoMapUpdateLoop(9)`, `MusicID_TempleNgangbius ($93)` | `PlaySound`, `Wait(10)`, `PlaySound` |
| 56 | `$070174..$07017F` | `move.l #DialogueTree38 ($1FA480), d0` / `jsr DialogueTreesToRAM` | `SetDialogueTree` |
| 57 | `$070180..$070189` | `move.b #$98, d0` / `jmp EventFlags_Set` (EclipseTorchStolen) | `SetFlag` `$98` |

The event returns nothing the dispatcher reads (an event's `d0` is ignored, `ps4.asm:120558`). The
dialogue is one entry of tree 37, `$32`, and its three resumes: the monk refuses to lend the torch
(entry), the Air Castle witches take it and challenge Lutz (first resume, after the figures
appear), the monk laments and the party agrees to fetch it back (second, after the torch objects
are cleared), and Rune explains the Air Castle and Lashiec (third, after the camera returns).
