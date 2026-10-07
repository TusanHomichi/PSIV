# `Event_VahalFortEntrance`

- **Retail bytes:** `$07318E..$07319F` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$8D]` at `$05A2B4`; event `$008D`.
- **Trigger:** `RunEvent_EnterVahalFort` (`$75`, `ps4.asm:116508`), Vahal Fort `$C8`: `EventFlag_ZemaOldMan` (`$B3`) set and `EventFlag_VahalFort` (`$B4`) clear, anywhere on the map (`trigger_table.rs`).
- **Data:** `vahal_fort.rs`, `VAHAL_FORT_ENTRANCE` (2 ops).

Issue #82, transcribed by the S9 lane. The five story scenes of the fort and the
plant are here; the two chest events are [111](111_WrenSkillChests.md) and the
platforms, terminals and belts are [the field mechanics
record](../field/PLATFORMS_AND_BELTS.md). Every body was read from the US image
through its `EventPtrs` slot; the clone's `grand_cross=0` text (`ps4.asm:152965`,
`:153114`) agrees and is corroboration only (it cuts `Event_DominatorsDefeated`
off in the middle of an instruction, which is why that body is read from the
image alone). `tests/test_vahal_events.py` re-derives every literal below from
the image.

All five read the **map's own tree**, `DialogueTree43` (shared by the overworld
extension rooms, the Weapon Plant and Vahal Fort); none calls
`DialogueTreesToRAM`.

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$07318E..$073195` | `moveq #0, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 0, standard window |
| 1 | `$073196..$07319F` | `move.b #$B4, d0` / `jmp EventFlags_Set` | `SetFlag` event `$B4` |

Entry 0 is Demi's emergency call on the way in (her report of the attack on the
Plate System, then the party's talk about it). The flag keeps the conversation
once.

## `Event_VahalFortMidway`

- **Retail bytes:** `$0731A0..$0731B1` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$8E]`; event `$008E`.
- **Trigger:** `RunEvent_VahalFortMidway` (`$76`, `ps4.asm:116519`), Vahal Fort F2 `$CA` (the room with the conveyor belts): `$B4` set, `$B5` clear. The map's event list is `[$0D, $0E, $10, $76]`, so the elevator, platform and belt probes are asked first.
- **Data:** `vahal_fort.rs`, `VAHAL_FORT_MIDWAY` (2 ops).

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0731A0..$0731A7` | `moveq #3, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 3 |
| 1 | `$0731A8..$0731B1` | `move.b #$B5, d0` / `jmp EventFlags_Set` | `SetFlag` event `$B5` (`EventFlag_VahalFortMidway`) |

## `Event_VahalFortBarrier`

- **Retail bytes:** `$0731DA..$0731FF` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$90]`; event `$0090`.
- **Trigger:** dialogue `$F6 $00 $90`, `DialogueTree43` entry 9. Vahal Fort's objects 2 to 5 are four `InvisibleBlock` objects at (45..48, 73) with dialogue 9, so a talk to any of them reaches it. The map's `MapDataManager` entry `$7B` despawns objects 1 to 5 (the blocks and the `Barrier` sprite) once `$B3` is set (`$05337E..$05338C`), so the barrier only exists on the **first visit, before the Zema old man**.
- **Data:** `vahal_fort.rs`, `VAHAL_FORT_BARRIER` (5 ops).

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0731DA..$0731E5` | `move.b #$B9, d0` / `jsr EventFlags_Test` / `bne.s $731F8` | `BranchFlag` event `$B9` (`EventFlag_VahFortBarrier`) set -> op 4, clear -> op 1 |
| 1 | `$0731E6..$0731ED` | `moveq #1, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 1 (what the force field is) |
| 2 | `$0731EE..$0731F7` | `move.b #$B9, d0` / `jmp EventFlags_Set` | `SetFlag` event `$B9` |
| 3 | | (the `jmp` returns) | `End` |
| 4 | `$0731F8..$0731FF` | `moveq #2, d0` / `jmp Event_GetAndRunDialogue` | `RunDialogue` entry 2 (the short refusal) |

The runtime test (`session_vahal.rs`) talks to the blocks through the pad:
entry 1 and the flag the first time, entry 2 and no write the second, and the
party stays south of the blocks.

## `Event_DominatorsDefeated`

- **Retail bytes:** `$073200..$0733B9` inclusive, 442 bytes (the last twelve are the `loc_733AC` wait subroutine and the two palette tables sit before it).
- **Pointer:** `EventPtrs[$91]`; event `$0091`.
- **Trigger:** `RunEvent_DominatorsDefeated` (`$77`, `ps4.asm:116530`), Vahal Fort F3 `$CB`: `EventFlag_Dominators` (`$B6`, set by `Event_DaughterTerminal` before its battle) set and `EventFlag_DaughterShutDown` (`$BB`) clear, so it fires on the first field frame after the Dominators battle returns.
- **Data:** `vahal_fort.rs`, `DOMINATORS_DEFEATED` (49 ops).

The party gathers at Daughter's terminal, Wren confronts her, the force field's
palette dies away and the leader steps back. No battle, no map change.

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-3 | `$073200..$07324F` | `Event_GetCharacter` + `move.w #x, $38(a4)` / `move.w #y, $3A(a4)` for Chaz (0) `($2C0,$2E0)`, Rune (3) `($2E0,$2E0)`, Rika (5) `($300,$2E0)`, Wren (7) `($2E0,$2B0)`; no membership test | four `MoveActorTo` (`wait: false`) |
| 4-9 | `$073250..$073295` | `Event_GetCharacter` for Hahn (2), Gryz (4), Demi (6), Raja (8), Kyra (9), Seth (10), each followed by `bmi` (absent: next test) or `bra loc_73296` | six `BranchIfPartyMember` |
| 10-21 | `loc_73296`, `$073296..$0732A1` | `($2F0,$2E0)` written to the member `Event_GetCharacter` found | six `MoveActorTo` + `Jump` to op 22 |
| 22-23 | `$0732A2..$0732B1` | `clr.b FieldObj_Step_Offset`; `bset #0, Char_Move_Flags` (follow chain off) and `bset #1` (Y first) | `SetStepOffset 0`, `SetFollowMode 0b11` |
| 24 | `$0732B2..$0732BB` | `DoMainUpdatesLoop($77)` | `Wait 120` |
| 25 | `$0732C0..$0732C5` | `loc_5A97C` with `d0 = 4`: `Event_UpdateObjFacing` on every non-empty character object | `FaceParty Up` |
| 26 | `$0732C6..$0732CD` | `moveq #5, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 5 |
| 27 | `$0732CE..$0732E3` | 120 x (`RunMapUpdates`, `VInt_Prepare`) | `Wait 120` |
| 28 | `$0732E4..$0732EB` | `SFXID_PowerDown` (`$E4`) | `PlaySound $E4` |
| 29-30 | `$0732EC..$073301` | `Palette_Line_2` words 12 and 13 (`$FFFFFB00 + $18/$1A`) set to `$06CE`, `$0CE0`; `VInt_Prepare` | `SetPaletteWords`, `WaitFrames 1` |
| 31-38 | `$073302..$073327` | the table loop: rows `($06CE,$0CE0)`, `($04AC,$0AC0)`, `($028A,$08A0)`, `($0068,$0680)` (`$07339A`, `$0733A4`), `bsr loc_733AC` (16 `VInt_Prepare`) after each | four `SetPaletteWords` + `WaitFrames 16` |
| 39 | `$073310` | the `$FFFF` terminator's `VInt_Prepare` | `WaitFrames 1` |
| 40 | `$073328..$073331` | `loc_5A79C` with `d0 = $1D`: 30 x `DMAPlanes_VInt`, no map updates | `WaitFrames 30` |
| 41 | `$073332..$073341` | `Event_GetCharacter(7)` (Wren); `Event_UpdateObjFacing` with `d0 = 0` | `Face Character(Wren) Down` |
| 42 | `$073342..$07334F` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 43 | `$073350..$073359` | `move.b #$BB, d0` / `jsr EventFlags_Set` | `SetFlag` event `$BB` |
| 44 | `$07335A..$073365` | `bclr #0`, `bclr #1` on `Char_Move_Flags` | `SetFollowMode 0` |
| 45 | `$073366..$073377` | `Character_1` to `($2E0,$2F0)`: `Event_MoveSingleObject` | `MoveActorTo Leader` (`wait: true`) |
| 46 | `$073378..$07337F` | `DoMainUpdatesLoop($3B)` | `Wait 60` |
| 47 | `$073380..$073385` | `move.b #1, FieldObj_Step_Offset` | `SetStepOffset 1` |
| 48 | `$073386..$073399` | `Event_MoveCamera` on `Character_1`'s live `$30/$34`, speed 2, a tail `jmp` | `CameraToActor Leader 2` |

(Op numbers are the scene's indices; the palette loop appears as ops 29 to 38
because the retail initial write and the first table row are the same pair and
both run: the loop's first iteration repeats it.)

The first window ends at the entry's `$F7` yield (the page "Fo...rn..."); the
palette and the 120-frame wait fall between it and the resumed text, which is why
`RunDialogue` and `RunDialogueResume` bracket them, as in the other scenes that
`popdlg`.

**The fifth member.** Whoever of Hahn, Gryz, Demi, Raja, Kyra or Seth holds the
last party slot, tested in that order, stands at `($2F0,$2E0)`; the first found
ends the search. A four-member party places nobody there. Chaz, Rune, Rika and
Wren are placed with no test, so the scene assumes they are in the party (the
story guarantees it), and the runner faults with `UnknownActor` rather than
inventing a seat if one is not.

**The ring at the first window.** With the follow chain off, each member walks
to its own destination during the `Wait 120`: Chaz `(44,47)`, Rune `(46,47)`,
Rika `(48,47)`, Wren `(46,44)` and the fifth `(47,47)` (pixel targets over 16 plus
the one-row standing shift). After `SetFollowMode 0` the followers trail the
leader as usual, so their final cells are the follow chain's, not the ring's.

## `Event_WeaponPlantArrival`

- **Retail bytes:** `$0733BA..$0733CB` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$92]`; event `$0092`.
- **Trigger:** `RunEvent_EnterWeaponPlant` (`$78`, `ps4.asm:116541`), Weapon Plant `$C4`: `EventFlag_WeaponPlant` (`$C7`) clear, no other condition.
- **Data:** `weapon_plant.rs`, `WEAPON_PLANT_ARRIVAL` (2 ops).

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0733BA..$0733C1` | `moveq #7, d0` / `jsr Event_GetAndRunDialogue` | `RunDialogue` entry 7 (what the plant is) |
| 1 | `$0733C2..$0733CB` | `move.b #$C7, d0` / `jmp EventFlags_Set` | `SetFlag` event `$C7` |

## Verification

- `rust/psiv-core/src/scene_runner/mechanics_tests.rs`: each scene's dialogue
  entries and flag writes; the barrier's two branches; `Event_DominatorsDefeated`
  for each of the six possible fifth members (targets in order, ring at the
  first window, leader's last cell, flag `$BB`) and for a four-member party.
- `rust/psiv-runtime/src/scene_vahal_tests.rs`, from the real maps' triggers:
  entrance, midway and arrival fire once on their flags and are silent without
  them, and the Dominators scene runs from trigger `$77` with the real party
  (negative control: silent once `$BB` is set).
- `rust/psiv-runtime/tests/session_vahal.rs`: the barrier talk through
  `Session::frame` with pads.
- `tests/test_vahal_events.py`: the bytes above against the scene source.

These are state tests from constructed saves: **the campaign route does not
reach Vahal Fort**, and the route lane that does runs them for real.
