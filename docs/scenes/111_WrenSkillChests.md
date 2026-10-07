# `Event_Burstroc`

- **Retail bytes:** `$0724A4..$07251F` inclusive, 124 bytes.
- **Pointer:** `EventPtrs[$6E]` at `$05A2B4`; event `$006E`.
- **Trigger:** interaction area type 2, `Interaction_EventIndexes[$0E] = $6E` (`ps4.asm:134938`), Weapon Plant F2 `$C6`: a one-cell area at (51,29) over a white chest, gated on **chest flag `$74`** (116), which the interaction gate tests and sets before the dispatch (`ps4.asm:118845-118869`).
- **Data:** `weapon_plant.rs`, `BURSTROC` (14 ops).

Issue #82, transcribed by the S9 lane. Wren installs the Burst Roc unit he finds
in the chest; the Positron Bolt unit in Vahal Fort F3 is the same routine at
another object. Both bodies were read from the US image through `EventPtrs`;
the clone's text (`ps4.asm:152126`, `:152152`) agrees and is corroboration only.
`tests/test_vahal_events.py` re-derives every literal from the image.

The chest is object index 7 of Weapon Plant F2 (`$FFFFC4C0 = $C300 + 7 * $40`:
seven map objects, then the chest), and object 8 of Vahal Fort F3 (`$FFFFC500`:
eight map objects). The port appends a map's chests after its objects
(`attach_chests`), so the same indices name the chests, and a chest whose object
faces up draws open: the scene's `Event_UpdateObjFacing` is the lid.

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0724A4..$0724AB` | `SFXID_ChestOpened` (`$E1`) | `PlaySound $E1` |
| 1 | `$0724AC..$0724B7` | `lea $FFFFC4C0, a4` / `moveq #4, d0` / `jsr Event_UpdateObjFacing` | `Face Npc(7) Up` |
| 2 | `$0724B8..$0724BF` | `DoMapUpdateLoop($27)` | `Wait 40` |
| 3 | `$0724C0..$0724C7` | `moveq #8, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 8 |
| 4 | `$0724C8..$0724CF` | `DoMapUpdateLoop($27)` | `Wait 40` |
| 5-6 | `$0724D0..$0724DF` | `SFXID_AndroidSkillImplant` (`$CF`); `DoMapUpdateLoop($27)` | `PlaySound $CF`, `Wait 40` |
| 7-8 | `$0724E0..$0724EF` | `SFXID_AndroidSkillImplant`; `DoMapUpdateLoop($13)` | `PlaySound $CF`, `Wait 20` |
| 9-10 | `$0724F0..$0724FF` | `SFXID_AndroidSkillImplant`; `DoMapUpdateLoop($3B)` | `PlaySound $CF`, `Wait 60` |
| 11 | `$072500..$07250D` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 12 | `$07250E..$072517` | `lea Wren_Stats, a0` / `move.b #$0E, skills+5(a0)` (`$67`) | `SetCharacterSkill {Wren, slot 5, skill $0E}` |
| 13 | `$072518..$07251F` | `moveq #$74, d0` / `jmp EventFlags_Set` | `SetFlag` event `$74` (`EventFlag_Burstroc`) |

Entry 8 of the map's tree has a yield after "I shall install it immediately."
(`end: close`); the three implant sounds and their waits fall between it and the
resumed text ("Device installation complete"), the same shape as
`Event_DominatorsDefeated`'s mid-dialogue palette.

`Wren_Stats` is `$FFFFF880` (`Character_Stats + 7 * $80`), and `skills` is record
offset `$62` (`ps4.constants.asm:56`), so the write is byte `$67` of Wren's
record: the sixth of eight skill bytes. Only the id byte is written; the use
counts stay as they were. `SkillID_Burstroc` is `$0E`.

## `Event_PosiBolt`

- **Retail bytes:** `$072520..$07259D` inclusive, 126 bytes.
- **Pointer:** `EventPtrs[$6F]`; event `$006F`.
- **Trigger:** interaction area type 2, `Interaction_EventIndexes[$0F] = $6F`, Vahal Fort F3 `$CB`: one cell at (76,61) over a white chest, gated on **chest flag `$90`** (144).
- **Data:** `vahal_fort.rs`, `POSI_BOLT` (14 ops).

The same fourteen ops with three literals different: the chest at `$FFFFC500`
(`Face Npc(8) Up`, `$072528..$072533`), dialogue entry 6 (`$07253C`), and the
skill write `move.b #$0F, skills+6(a0)` (`$07258A..$072593`, byte `$68`:
`SkillID_Posibolt`, the seventh skill byte). The closing flag is
`move.b #$90, d0` / `jmp EventFlags_Set` (`$072594..$07259D`):
`EventFlag_MuskCats` in the clone's constants, a name the clone gives the id from
a different place in the game (`ps4.constants.asm:1583`). The port sets event
flag `$90` as the bytes say and does not rename it.

**Two banks, one number.** The interaction gate sets *chest* flag `$74` (or
`$90`) before the scene runs, and the scene's own `EventFlags_Set` sets *event*
flag `$74` (`$90`). They are different bits in different banks (`$FFFFF120` and
`$FFFFF100`); the chest flag is what stops the area firing twice, and the event
flag is the story's record.

## Verification

- `mechanics_tests.rs`: the sound and wait sequence, the lid, the skill byte
  (and only it), the flag; a skill slot past the array and an unseated Wren
  fault with `BadWrite` instead of writing.
- `scene_vahal_tests.rs`, from the real maps through the interaction probe:
  both chests open, Wren's record differs from before in exactly the one byte,
  both banks' flags are set, the lid is up; a second confirm in front of the
  opened chest starts nothing (the chest flag has retired the area).
- `tests/test_vahal_events.py`: the bytes above against the scene source, and a
  negative control that changes the skill id byte in the image.

These are state tests from constructed saves: **the campaign route does not
reach Vahal Fort or the Weapon Plant**, and the route lane that does runs them
for real. The skills' battle behaviour (`SkillObj_Burstroc`, `SkillObj_Posibolt`)
is the battle lanes' ([player abilities](../battle/PLAYER_ABILITIES.md)).
