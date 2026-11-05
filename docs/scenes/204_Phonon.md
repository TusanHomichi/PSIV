# `Event_Phonon`

- **Retail bytes:** `$0722F6..$072427` inclusive.
- **Pointer:** `EventPtrs[$6C]` at `$05A2B4 + $1B0`; event `$006C`.
- **Trigger:** interaction area 3 of `PlateSystem_F3` (`$0BD`): `Interaction_EventIndexes[$0C]`, the chest cell (27,20), gated on `ChestFlag_Phonon` (`$42`). The chest is object 3, after the floor's three elevators.
- **Data:** `plate_wreckage.rs`, `PHONON` (33 ops).
- **Flags:** with Demi absent, clears `ChestFlag_Phonon` (`$42`, `ChestFlags_Clear` on `$F120`, `Flag::chest`) so the chest can be opened again. With Demi present, writes `SkillID_Phonon` (`$11`) to her skill slot 5 and no flag. No other lane's event reads either.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:152031-152097`. It has no `revision` or `grand_cross` conditional and loads no tree. The US
flag routines were read too: `ChestFlags_Clear` (`$0576B2`) works on `$F120`, the bank `Flag::chest`
names, whatever the disassembly's constants file says.

## Retail transcription

`DoMapUpdateLoop(n)` and `DoMainUpdatesLoop(n)` are `Wait(n + 1)`.

| Op | Retail primitive | Scene record |
|---:|---|---|
| 0-2 | `SFXID_ChestOpened`; object 3 faces up; `DoMapUpdateLoop($27)` | `PlaySound`, `Face`, `Wait(40)` |
| 3 | `FindCharacterSlot(CharID_Demi)`, `bmi.w loc_72404` | `BranchIfPartyMember` Demi |
| 4 | `moveq #$4E` / `Event_GetAndRunDialogue` | standard `RunDialogue` `$4E` |
| 5 | `$FFFFED52 := 5`, `Pal_VariableFadeOut` | `VariablePaletteFade` out, mode 5: blocks for six frames per level of the palette's largest nibble |
| 6-18 | waits `$27`, `$27`, `$E`, 9, `$1D`, `$3B`, `$27` between `SFXID_AndroidSkillImplant` x3, `SFXID_Megid`, `SFXID_Recovery`, `SFXID_AndroidSkillImplant` | `Wait` and `PlaySound` |
| 19-25 | `trap #1` of 64 words `Palette_Table_Buffer_2` to `Palette_Table_Buffer`, `DoMainUpdatesLoop(0)`, fade in (mode 2), fade out (mode 0), the copy and wait again, fade in (mode 0) | `PaletteCopy`, `Wait(1)`, `VariablePaletteFade` |
| 26-28 | `popdlg` / `Event_RunDialogue`; `move.b #SkillID_Phonon, skills+5(Demi_Stats)`; `rts` | `RunDialogueResume`, `SetCharacterSkill`, `End` |
| 29-32 | `loc_72404`: entry `$4F`, `SFXID_ChestOpened`, object 3 faces down, `jmp ChestFlags_Clear` with `ChestFlag_Phonon` | `RunDialogue`, `PlaySound`, `Face`, `SetFlag` chest `$42` clear |

## Verification

`rust/psiv-core/tests/plate_wreckage_scenes.rs` holds both arms. `rust/psiv-campaign/tests/plate_system.rs`
opens the chest with pads on the real map with and without Demi.
