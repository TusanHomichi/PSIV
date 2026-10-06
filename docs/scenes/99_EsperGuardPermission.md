# `Event_EsperGuardPermission`

- **Retail bytes:** `$0709A2..$070A29` inclusive, 136 bytes.
- **Pointer:** `EventPtrs[$4F]` at `$05A2B4`; event `$004F`.
- **Trigger:** Dialogue `$F6 $00 $4F`, entry 51 of `DialogueTree21` (and `DialogueTree20`): the
  two `EsperGuard` objects of `EspMansionEntrance` `$166` (objects 0 and 1, dialogue 51).
- **Data:** `esper_events.rs`, `ESPER_GUARD_PERMISSION` (14 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot and every instruction
word matches `ps4.asm:150070-150113`; the clone's label names it and nothing else. It has no
`revision` conditional and no `DialogueTreesToRAM` call.

## Retail transcription

`Field_Obj_Secondary` is `$FFFFC300`, object 0 of the map; `$FFFFC340` is object 1, and
`$14(a4)` is a field object's `dialogue_id`, so the writes at `$0709FA..$070A05` and `$070A14..$070A1F` are
`(Field_Obj_Secondary)+$14` and `+$54`, one for each guard.

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-1 | `$0709A2..$0709CD` | `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing`, once for `$C300` and once for `$C340` | `FaceOppositeOf` guard 0, guard 1 |
| 2 | `$0709CE..$0709D9` | `move.b #$9E, d0` (Dark Force 2) / `jsr EventFlags_Test` / `bne` | `BranchFlag` `$9E` |
| 3 | `$0709DA..$0709E5` | `move.b #$95, d0` (Carnivorous Trees) / test / `bne` | `BranchFlag` `$95` |
| 4-5 | `$0709E6..$0709ED` | `moveq #0, d0` / `jmp Event_GetAndRunDialogue`: the event ends there | `standard(0)`, `End` |
| 6-9 | `$0709EE..$070A07` | `moveq #2, d0` / `jsr Event_GetAndRunDialogue`, `move.b #2, $14(a4)`, `move.b #2, $54(a4)`, `bra` | `standard(2)`, `SetNpcDialogue` 0 and 1 to 2, `Jump` |
| 10-12 | `$070A08..$070A1F` | `moveq #1, d0` / dialogue, `move.b #1, $14(a4)`, `move.b #1, $54(a4)` | `standard(1)`, `SetNpcDialogue` 0 and 1 to 1 |
| 13 | `$070A20..$070A29` | `move.b #TempEveFlag_EspMansionGuards ($1A), d0` / `jmp TempEveFlags_Set` | `SetFlag` temp `$1A` |

Dialogue 0 is the turning-away line ("Who dares enter? Suspicious people will not be allowed to
enter!"), 1 is "Oh, it's you, Kyra! Welcome back!" and 2 is "So you're a friend of Kyra. Please
come in." (tree 21 entries 0 to 2). The guards step aside because their object routine
(`FieldObj_EsperGuard`, `ps4.asm:96529`, the test at `:96545`) tests the temp flag every frame; the port's
`BespokeFlag::EspMansionGuards` is that test, and the object init clears the flag on every map
load, so leaving the mansion shuts the door again.

`SetNpcDialogue` is a new scene op (the first write of a field object's dialogue id): the
runtime puts the id in the map's dialogue overrides, the table a map effect already feeds, so a
map reload drops it as the object rebuild does in retail.

## In the route (C7)

The `esper-mansion-door` chapter talks to the left guard with the carnivorous trees fought
(`$95` set, `$9E` clear): entry 1, flag `$1A`, the guards walk off the door row.
