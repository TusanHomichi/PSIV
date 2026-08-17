# `Event_RuneLadaeTower`

- **Retail bytes:** `$06E930..$06EA15` inclusive, 230 bytes.
- **Pointer:** `EventPtrs[$2E]` at `$05A2B4`; scene event is `$002E`.
- **Trigger:** Ladea Tower F2 `$8E`, `RunEvent_RuneLadeaTower` (`$19`):
  Rune-again `$62` clear and leader X `>=$3C0`.
- **Data:** `post_rika_events.rs`, `RUNE_LADEA_TOWER` (9 ops).

## Clone audit

The body is the retail `grand_cross=0` routine at the pointer range above.
Grand Cross script content is not used to infer Rune's equipment or party
slot.

## Retail transcription

| Op | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0 | `$06E930` | `Current_Party_Slot_5 ← CharID_Rune` (`$03`) | `JoinParty(slot 4, Rune)` |
| 1 | `$06E936..$06E96A` | current HP/TP ← max; equipment `$37,00,$36,$38`; update mods/elements | `ConfigureCharacter` |
| 2 | `$06E970..$06E99E` | construct Character 5, art `$554`, secondary-object copy | `ObjectAnimation` |
| 3-4 | `$06E9A4..$06E9CC` | VInt; step offset 0; optional move to `($3D0,$3B0)` | step/object presentation |
| 5 | `$06E9D6..$06E9E8` | face left; dialogue entry 6 | `RunDialogue(6)` |
| 6 | `$06E9EE..$06EA02` | move Rune beside Character 4; `Event_AddMacro(3)` | destination + macro record |
| 7-8 | `$06EA08..$06EA15` | step offset 1; set Rune Joined Again `$62` | step + `SetFlag($62)` |

The party write happens before the field object is constructed, exactly as in
the cartridge. The runner recasts the new character on the next map-changing
scene; this event's object choreography remains a presentation record rather
than a fabricated map NPC.

## Verification

The arc test asserts Rune in slot 5, the four equipment bytes, restored HP/TP,
and flag `$62` after the event.
