# `Cutscene_MeetingWren`

- **Retail bytes:** `$075FC8..$07606B` inclusive, 164 bytes.
- **Pointer:** `CutscenePtrs[$0C]` at `$05A580`; scene event `$800C`.
- **Dispatch:** `RunEventsJmpTbl[$21/$23/$24/$25]` writes `$800D`; the
  player reaches Zelan F1 and talks to Wren, which enters this cutscene
  through the dialogue event path.
- **Data:** `post_zio_cutscenes.rs`, `MEETING_WREN` (16 ops).

## Clone audit

The pointer-table `grand_cross=0` branch at `ps4.asm:120776` selects
`Cutscene_MeetingWren`. The body at `ps4.asm:155391` is the retail body used
here; no Grand Cross-only scene include or FortuneTeller body is involved.
The range was checked against the actual US retail ROM pointer pair.

## Retail transcription

| Ops | ROM offsets | Retail bytes / primitive | Scene record |
|---:|---|---|---|
| 0 | `$075FC8..$075FD7` | face `Field_Obj_Secondary` opposite `Character_1`; init/fade | face/init/fade |
| 1-5 | `$075FD8..$075FF3` | panel `$76`, DMA, corrected `20`-frame wait | panel/DMA/wait |
| 6-7 | `$075FF4..$07600B` | render sprites; `Event_GetAndRunDialogue5`, entry `1` | cutscene dialogue |
| 8-10 | `$07600C..$07603A` | construct Wren object `$20` at `($1F0,$C0)`; load/build sprites | object destination/rebuild |
| 11-13 | `$07603B..$076062` | write next party slot, `Event_AddMacro(7)`, clear NPC slot 0 | party/macro/despawn |
| 14-15 | `$076063..$07606B` | set `EventFlag_WrenJoined=$70`; return `0` | flag/return |

The runtime keeps the Wren object construction as a typed presentation record,
but the party slot and map-object clear are persistent state edges. The
headless proof asserts party `[Chaz,Rika,Rune,Wren]`, flag `$70`, and an
inactive Zelan F1 NPC index 0.

