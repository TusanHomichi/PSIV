# `Event_RockyOwner`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0075` `Event_RockyOwner` | `$072872..$072897` inclusive, 38 bytes | `EventPtrs[$75]` at `$05A2B4 + $1D4` | `aiedo_events.rs`, `ROCKY_OWNER` (3 ops) |

- **Trigger:** dialogue `$F6`, `DialogueTree11`, from Rocky's owner (`RockyHouse` `$065`,
  object 1, dialogue `$42`). Entry `$42`'s chain is `$FA $25 +6`, `$FA $24 +5`, `$FA $1F +3`,
  `$FA $1E +2`: with the guild's dog job taken (`$1E`) and the owner not yet spoken to (`$1F`
  clear, and neither later dog flag set) it lands on entry `$44`, `$F6 $00 $75`.
  (`$FA`'s operand is a forward delta: [trigger census](TRIGGER_CENSUS.md).)

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every instruction word
matches `ps4.asm:152389-152398`. It has no `revision` conditional and no `DialogueTreesToRAM`
call.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072872..$072887` | `lea ($FFFFC340).w, a4` (map object 1) / `lea (Character_1).w, a3` / `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 1, of the leader |
| 1 | `$072888..$07288F` | `moveq #$5E, d0` / `jsr Event_GetAndRunDialogue` (`$5AC66`) | entry `$5E`, standard window |
| 2 | `$072890..$072897` | `moveq #EventFlag_RockyOwner, d0` (`$1F`) / `jmp EventFlags_Set` (`$57666`) | `SetFlag` event `$1F` |

The owner asks the party to find the dog. This scene is the only writer of
`EventFlag_RockyOwner` (`ps4.constants.asm:1509`). It is not one of the guild's flags: the dog
job's `HuntersGuildData` row is `$1E`, `$24`, `$25`. Its readers are the owner's own chain and
the map-data routines that place the runaway dog, `MapDataMan_KrupRocky` (`ps4.asm:110391`) and
`MapDataMan_MonsenRocky` (`ps4.asm:110420`). With `$1F` set and the dog not yet found, Monsen
shows him. Those routines and the dog's own events belong to the Monsen and Krup work.

## Tests

- `rust/psiv-core/tests/aiedo_scenes.rs`,
  `the_three_talks_face_the_leader_speak_and_set_their_flag`: object 1 turns to face the leader,
  the line is `$5E`, and `$1F` is set.
- `rust/psiv-campaign/tests/aiedo_side.rs`, `rockys_owner_asks_for_the_dog`: from a hand-built
  save in `RockyHouse` with `$1E` set, the driver talks to the owner with pads and `$1F` is set;
  without `$1E` the owner only talks and `$1F` stays clear.
