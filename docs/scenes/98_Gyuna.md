# `Event_Gyuna`

- **Retail bytes:** `$070C82..$070CB3` inclusive, 50 bytes.
- **Pointer:** `EventPtrs[$5C]` at `$05A2B4`; event `$005C`.
- **Trigger:** dialogue `$F6 $00 $5C`, `DialogueTree16` entry 48: the Ryuon
  pub's keeper (map `$14A`). Not a `RunEventsJmpTbl` event; the census finds it
  as a dialogue source ([EVENT_COVERAGE](EVENT_COVERAGE.md)).
- **Data:** `dezolis_route.rs`, `GYUNA` (7 ops).
- **New op:** `BranchIfSavedDialogueByte`.

On the route: the only writer of event flag `$81` in the cartridge is
`$070CA8`, in this routine, and Tyler's grave inscription (`DialogueTree14`
entry 29, `FA 81,6`) only reaches the `$F6 $44` of
[`Event_TylerGraveOpening`](92_TylerGraveOpening.md) once `$81` is set. The
grave's own text says why: "Gyuna said to inspect this grave closely".

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$070C82..$070C8C` | `move.b #$81, d0` / `jsr EventFlags_Test` / `bne.b loc_70C92` | `BranchFlag` `$81` (set to op 3) |
| 1-2 | `$070C8E..$070C90` | `moveq #$31, d0` / `bra.b loc_70C94`, then `jsr Event_GetAndRunDialogue` (`$070C94`) | standard `RunDialogue` `$31`, `Jump` over op 3 |
| 3 | `$070C92` | `loc_70C92: moveq #$32, d0` (falls into the same call) | standard `RunDialogue` `$32` |
| 4 | `$070C9A..$070CA6` | `moveq #-1, d0 / move.w $ECF0.w, d0 / movea.l d0, a0` (`popdlg`), `cmpi.b #$35, (a0)`, `bne.b loc_70CB2` | `BranchIfSavedDialogueByte` `$35` |
| 5 | `$070CA8..$070CB0` | `move.b #$81, d0` / `jsr EventFlags_Set` | `SetFlag` `$81` |
| 6 | `$070CB2` | `rts` | `End` |

## What the byte is

`Saved_Dialogue_Addr` (`$FFFFECF0`) is written by `pushdlg`
(`ps4.macrosetup.asm:152`) in `TextCtrlCode_Terminate2`
(`ps4.asm:142643`, retail `$06A12C`). The `$F7` handler
(`TextCtrlCode_Terminate3`) is a `bra.w` to it, and so are both terminators
(`TextCtrlCode_Terminate` falls into it). `RunText_CharacterLoop` has read the
terminator with `move.b (a0)+, d0`, so the stored address is the byte *after*
it. It is therefore not the last glyph shown: for an `$F7` it is the next byte
of the same entry, and for `$FE`/`$FF` it is the first byte of the next entry
(`GetDialogueByID` counts `$FF`-separated entries).

Gyuna's `$35` is the glyph `.`. Entry 60 ("Do you want to know about the
whereabouts of the space ship?", then the answer about a grave) is the one
entry whose successor opens on `$35` (entry 61, "...What's the use now?"). So
the conversation sets `$81` exactly when it ends in entry 60:

| Answers at the prompts (entry 49, 53, 56, 60) | Ends in | Next entry opens with | `$81` |
|---|---|---|---|
| NO, NO, NO, YES | 60 | `$35` | set |
| NO, NO, NO, NO | 62 ("Thank you.") | not `$35` | clear |
| YES | 51 (the storm) | not `$35` | clear |

The runtime rebuilds the byte from the pack (`stop_byte.rs`): a control's code,
or the first character's glyph byte; `every_entry_opens_on_the_byte_its_segments_give`
holds that to every entry's `raw_hex`. The decoded stream drops the null
controls and terminators, so an entry opening on `$F0`/`$F1`/`$F8`/`$FB` would
be the one case it could not rebuild; the test finds none.

## New op

- **`BranchIfSavedDialogueByte { value, if_equal, if_not }`**: `popdlg` followed
  by `cmpi.b #value, (a0)`. The runner holds the byte its runtime last reported
  (`SceneRunner::set_dialogue_stop_byte`, called when the window closes), and
  nothing is equal before any dialogue has run. The Godot cutscene consumer
  ignores it (control flow).

Tests: `rust/psiv-core/tests/dezolis_scenes.rs` (the flag set only on `$35`,
an off-by-one stop byte failing it, the greeting by `$81`),
`rust/psiv-runtime/src/dialogue/stop_byte/tests.rs` (the byte against the pack
and against the window driven by answers), and the route chapters
`dezolis-gyuna` and `dezolis-tyler-grave`.
