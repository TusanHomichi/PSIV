# Torinco: `Event_DyingBoyFather`, `Event_FatherAfterBoyRecovery`, `Event_MeetingSekreas`

Two of the Hunter's Guild's Torinco jobs (`HuntersGuildData`, `ps4.asm:151519`): "Dying Boy"
(taken `EventFlag_DyingBoy` `$A8`, done `EventFlag_CulversAfterRecovery` `$AB`), whose father
Culvers lives in `CulversHouse` (`$0E7`), and "Man with Twist" (taken `EventFlag_ManWithTwist`
`$AD`), whose client Sekreas waits in `TorincoHouse2` (`$0E9`). Both maps use tree 24. The
coordinating issue grouped these as "Mile and Zosa"; on the cartridge they are Torinco.

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0084` `Event_DyingBoyFather` | `$072D44..$072D6B` inclusive, 40 bytes | `EventPtrs[$84]` at `$05A2B4 + $210` | `motavia_side.rs`, `DYING_BOY_FATHER` (3 ops) |
| `$0085` `Event_FatherAfterBoyRecovery` | `$072D6C..$072D93` inclusive, 40 bytes | `EventPtrs[$85]` at `$05A2B4 + $214` | `motavia_side.rs`, `FATHER_AFTER_BOY_RECOVERY` (3 ops) |
| `$0087` `Event_MeetingSekreas` | `$072FB4..$072FDB` inclusive, 40 bytes | `EventPtrs[$87]` at `$05A2B4 + $21C` | `motavia_side.rs`, `MEETING_SEKREAS` (3 ops) |

`$86` `Event_DyingBoy`, the Alis Sword scene between them, is not in this record; it has its own.

- **Triggers:** dialogue `$F6` in tree 24. (`$FA`'s operand is a forward delta:
  [trigger census](TRIGGER_CENSUS.md).)
  - Culvers (object 0, dialogue `$28`): `$FA $AC +5`, `$FA $AB +4`, `$FA $AA +3`, `$FA $A9 +2`,
    `$FA $A8 +1`. With the job taken and nothing later, entry `$29` fires `$84`. With the boy cured
    (`$AA`) and `$AB` still clear, entry `$2B` fires `$85`.
  - Sekreas (object 0, dialogue `$24`): `$FA $AF +3`, `$FA $AE +2`, `$FA $AD +1`. With the job
    taken and nothing later, entry `$25` fires `$87`.
- **Flags** (`ps4.constants.asm:1606-1614`): `$84` sets `EventFlag_Culvers` `$A9`, read by `$86`
  and by Culvers's own entry. `$85` sets `$AB`, the guild's done flag for the job. `$87` sets
  `EventFlag_Sekreas` `$AE`, read by `Event_KingRappy` (`$88`, `ps4.asm:152866`) before the fight.

## Clone audit

All three bodies were read from the US image through their `EventPtrs` slots, and every instruction
word matches `ps4.asm:152706-152725` and `ps4.asm:152856-152865`. None has a `revision` or
`grand_cross` conditional, and none loads a dialogue tree.

## Retail transcription

Each is `Event_RockyOwner`'s shape ([120](120_RockyOwner.md)), with `move.b #flag, d0` where that one
has `moveq`:

| Op | ROM offsets (`$84` / `$85` / `$87`) | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072D44..$072D59` / `$072D6C..$072D81` / `$072FB4..$072FC9` | `lea (Field_Obj_Secondary).w, a4` … `Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 1 | `$072D5A..$072D61` / `$072D82..$072D89` / `$072FCA..$072FD1` | entry `$39` / `$3A` / `$3C` | `RunDialogue` |
| 2 | `$072D62..$072D6B` / `$072D8A..$072D93` / `$072FD2..$072FDB` | `move.b #flag, d0` / `jmp EventFlags_Set` | `SetFlag` `$A9` / `$AB` / `$AE` |

## Evidence

- State: `rust/psiv-core/tests/torinco_scenes.rs`: each turns object 0 to the leader, speaks its
  one entry and sets its flag.
- Pad-driven in the headless runtime on the real maps, from hand-built saves
  (`rust/psiv-campaign/tests/motavia_side.rs`): Culvers before and after the cure, Sekreas with the
  job taken, and a negative control without the job for each.
- No campaign chapter: the route does not take the guild's jobs. No oracle comparison, no visual
  claim and no persistence claim.
