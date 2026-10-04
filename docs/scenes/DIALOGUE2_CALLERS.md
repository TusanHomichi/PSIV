# `Event_GetAndRunDialogue2` caller audit (H22 class fix)

`Event_ZioNurvus` ended its presentation with a `RunDialogueResume`. The
cartridge's last op there is `moveq #$B, d0 / jsr Event_GetAndRunDialogue2`
(`ps4.asm:148622-148623`), entry `$0B` of the map's own tree. A resume reopens
`Saved_Dialogue_Addr`, which in play is whatever dialogue ran last and after a
load is nothing, so the committed engine halted at Zio's trigger
([runner log H22](../campaign/RUNNER_LOG.md#h22-zio-at-nurvus-resumes-a-dialogue-instead-of-running-entry-0b-and-the-zio-phase-counter-is-unmodelled)).
This document is the audit of every caller of that routine, and the census
that keeps it true.

## What the routine is

The four `Event_GetAndRunDialogue*` entry points disassembled from the image
(`$5AC66`, `$5ACDC`, `$5AD32`, `$5ADF8`) start the same way: `jsr GetDialogueByID` (`$59164`, entry `d0` of `Current_Dialogue_Tree`),
play `SFXID_Selection`, create window 1, run the text and wait for a button.
What differs is the exit, read from the image (`$5ACDC..$5AD30`):

| Routine | Image | Exit |
|---|---|---|
| `Event_GetAndRunDialogue` | `$5AC66` | `Panel_DestroyAll` (`$5AE74`), destroy the second window if `$FFFFECA4` bit 0 is set, destroy window 1, reload the map chunks (`Map_LoadChunks`, `$51ADC`) |
| **`Event_GetAndRunDialogue2`** | **`$5ACDC`** | **`moveq #0, d0` then `move.b d0, Panel_Num`, `move.b d0, Windows_Opened_Num`, `move.b d0, $FFFFECA4`; `rts`.** No destroy and no chunk reload: the text window and every panel stay on screen, forgotten rather than closed |
| `Event_GetAndRunDialogue3` | `$5AD32` | as the first, with `Panel_DestroyAll` omitted |
| `Event_GetAndRunDialogue5` | `$5ADF8` | as the first, then `PalFadeOut_ClrSpriteTbl` and `Panel_DestroyAll` |

(The clone comments the `SFXID_Selection` store out of several of these;
the image has it in all four, `13FC 00F3 FFFF500A` at the second instruction.
The port is silent on dialogue open everywhere, which is a separate gap.)

**The rule:** a `Dialogue2` call is an ordinary `RunDialogue` of a literal entry
in the *current* tree, followed by a window that is never destroyed. It is not a
resume, it does not read `Saved_Dialogue_Addr`, and it never ends a scene on a
clean map: every one of its fourteen callers is followed by a battle, a map
change or another scene. The port names it `DialogueWindow::Retained`
(`rust/psiv-core/src/scene.rs`), written in scenes as `retained(entry)`
(`rust/psiv-core/src/scenes/mod.rs`); the Godot presenter destroys panels on
close only for `Standard`, `Cutscene` and `Cutscene5`, so a retained window
leaves them up as the cartridge does.

## Audit

The image has exactly fourteen `jsr $5ACDC.l`. The first column is the
`ps4.asm` line of the `jsr` (the clone's labels, used for navigation only); the
second is the address in the US image, which is the authority. "Tree" is what
`GetDialogueByID` indexes at the call: the tree the scene loads itself, or else
the tree bound to the map the scene runs on (`runtime-pack/maps/*.json`
`dialogue_tree`, from the trigger census in
[12_ArcTriggerCensus](12_ArcTriggerCensus.md)). Entry text is the first line in
`runtime-pack/dialogue/trees.json` for that tree and entry.

| # | `ps4.asm` | Image `jsr` | Scene (event) | Entry | Tree at the call | Entry text | Port before | Port after |
|---:|---:|---|---|---:|---|---|---|---|
| 1 | 148623 | `$06F3D4` | `Event_ZioNurvus` (`$34`) | `$0B` | map: Nurvus B4 Part2 (`$D3`), tree 36 | "Well, well... You've made it here" | **`RunDialogueResume`** (mismatch) | `retained(0x0B)` |
| 2 | 149136 | `$06FAFA` | `Event_DarkForce1` (`$3F`) | `$06` | map: Kuran F3 (`$198`), tree 35 | "What on earth? This feeling of oppression" | `RunDialogue` entry 6, `Standard` window | `retained(6)` |
| 3 | 149154 | `$06FB42` | `Event_Juza` (`$40`) | `$48` | map: Zio Fort Juza room (`$87`), tree 13 | "You are... I am Juza." | `RunDialogue` entry `$48`, `Standard` window | `retained(0x48)` |
| 4 | 150038 | `$070936` | `Event_SavingKyra` (`$4D`) | `$26` | scene: `DialogueTree37` loaded at `$070928` | "Look! Someone is being attacked!" | `Standard` window; **on-foot branch landed on op 8, replaying dismount dialogue `$34`** (mismatch) | `retained(0x26)`; on-foot branch to op 9 (`beq.w loc_70908`, `ps4.asm:149987-149988`) |
| 5 | 150056 | `$070978` | `Event_DarkForce2` (`$4E`) | `$3A` | map: Garuberk Tower Part7 (`$19F`), tree 22 | "What's that? Is that Dark Force?!" | `Standard` window | `retained(0x3A)` |
| 6 | 150173 | `$070B32` | `Event_XeAThoulBeforeBattle` (`$59`) | `$36` | map: Air Castle Xe A Thoul room (`$184`), tree 22 | "Ha, ha, ha, ha! You've come!" | `Standard` window | `retained(0x36)` |
| 7 | 150182 | `$070B58` | `Event_AirCastleFakeChest` (`$5A`) | `$3C` | map: Air Castle Inner B1 Part3 (`$187`), tree 22 | "Oh no! This torch is a fake!" | `Standard` window | `retained(0x3C)` |
| 8 | 150364 | `$070E1E` | `Event_LashiecAppearance` (`$5D`) | `$37` | map: Air Castle Inner B1 Part3 (`$187`), tree 22 | "You've finally come... Lutz!" | `Standard` window | `retained(0x37)` |
| 9 | 153841 | `$074502` | `Cutscene_Alshline` (`$8005`) | `$68` | scene: `DialogueTree3`, reloaded at `$0744EE` before the call | "How are we going to get in with that creature blocking the way" | `Standard` window | `retained(0x68)` |
| 10 | 157969 | `$078E9C` | `Cutscene_ProfoundDarkness` (`$8020`) | `$03` | map: The Edge Part9 (`$108`), tree 42 | "...Is this the shape The Profound Darkness has taken?" | `Standard` window | `retained(3)` |
| 11 | 150750 | `$071538` | `Event_AngerTowerAlys` (`$62`) | `$08` | not transcribed | | no scene | **untranscribed**, [#71](https://github.com/TusanHomichi/PSIV/issues/71) |
| 12 | 152563 | `$072B2E` | `Event_FractOozeFound` (`$7D`) | `$2F` | current tree of its dialogue trigger | | no scene | `retained(0x2F)`; scene [95](95_FractOozeFound.md) |
| 13 | 152871 | `$072FEA` | `Event_KingRappy` (`$88`) | `$36` | current tree of its dialogue trigger | | no scene | `retained(0x36)`; scene [96](96_KingRappy.md) |
| 14 | 152977 | `$0731B4` | `Event_DaughterTerminal` (`$8F`) | `$04` | current tree of its dialogue trigger | | no scene | `retained(0x04)`; scene [97](97_DaughterTerminal.md) |

Findings:

- **One resume mismatch (the H22 defect):** row 1. The census test
  `no_scene_resumes_before_it_opens_a_dialogue` extends that to every
  registered scene: none opens with a resume now.
- **One control-flow mismatch the audit surfaced (row 4):**
  `BranchIfVehicle { if_on_foot: 8 }` jumped onto the dismount dialogue `$34`,
  so a party on foot replayed "It won't work. Even with this..." before the
  attack. The cartridge's `beq.w loc_70908` skips to the red-alert music.
- **Ten of ten window-variant mismatches (rows 1 to 10):** nine transcribed
  callers used the standard window, which destroys the panels the cartridge
  leaves up, and the tenth was the resume of row 1.
- **One caller has no scene** (row 11, `Event_AngerTowerAlys`; rows 12 to 14 were
  transcribed by the event census lane): its event is in the retail `EventPtrs`
  table and fires from a dialogue `$F6` on Dezolis, but the port transcribes
  nothing for it, so it plays nothing. It is deliberately not hidden: the
  census lists it in `UNTRANSCRIBED` (`tests/test_dialogue2_callers.py`) and
  fails if the image gains or loses one, and when a scene lands its row has to
  move into `DIALOGUE2_CALLERS` in `rust/psiv-core/src/scenes/mod.rs`. Open
  work, not done here: transcribe it, tracked by
  [#71](https://github.com/TusanHomichi/PSIV/issues/71).
- **Not in scope, seen on the way:** `Event_CarnivorousTrees`
  (`ps4.asm:149973-149984`) and `Event_SavingKyra` (`ps4.asm:150039-150051`)
  move the *whole party* down `$20` pixels with a direct `move.w d1, $34(a4)`
  loop over `CalcPartyNumber` members, where the port runs `MoveActorOffset`
  on the leader with `wait`. A transcription difference of the Dezolis
  campaign records, not of the Dialogue2 class; tracked by
  [#71](https://github.com/TusanHomichi/PSIV/issues/71).

## Open-gap issue handoff (2026-10-03)

An audit at `82eed831af88234feec8ab8b9322a9d2624de91f` inspected all 44
open and closed GitHub issues. Open [#71](https://github.com/TusanHomichi/PSIV/issues/71)
already names all four missing callers and the party-move discrepancy; no
duplicate issue was filed. Its sibling-dialogue census work remains outside
this closeout.

The local image matches the [accepted US ROM hash](../DEVELOPMENT.md#prepare-the-local-pack).
`psiv_tools/newgame.py::event_ptrs` resolves `EventPtrs` to `$05A2B4` from the
retail dispatcher bytes. The pointer slots and adjacent entries establish the
following spans; labels and fork build-address comments supply no authority.
`tests/test_dialogue2_callers.py::rom_callers` independently finds fourteen
callers, including these four, and the existing image/registry/document census
reports no disagreement while retaining their explicit `UNTRANSCRIBED` status.

| Event | Pointer slot -> routine | EventPtrs-derived span (inclusive) | Entry load and `jsr $5ACDC` bytes |
|---|---|---|---|
| AngerTowerAlys `$62` | `$05A43C` -> `$07148A` | `$07148A..$071579` (includes direction data) | `$071536`: `7008 4EB9 0005 ACDC` |
| FractOozeFound `$7D` | `$05A4A8` -> `$072B2C` | `$072B2C..$072B51` | `$072B2C`: `702F 4EB9 0005 ACDC` |
| KingRappy `$88` | `$05A4D4` -> `$072FDC` | `$072FDC..$073017` | `$072FE8`: `7036 4EB9 0005 ACDC` |
| DaughterTerminal `$8F` | `$05A4F0` -> `$0731B2` | `$0731B2..$0731D9` | `$0731B2`: `7004 4EB9 0005 ACDC` |

**Missing-scene acceptance:** own the transcriptions under
`rust/psiv-core/src/scenes/` and their dispatch/table entries in `scenes/mod.rs`.
Each event must dispatch a scene with its retail tree binding, literal retained
entry, branches and ordered actor/flag/battle/return effects. Move its row from
Python `UNTRANSCRIBED` into the documented scene ranges and Rust
`DIALOGUE2_CALLERS`; the Python census and negative controls and Rust
`every_dialogue2_caller_is_a_retained_window` must pass. Add scene outcome and
branch assertions in `rust/psiv-core/tests/scene_outcomes.rs`. Retained-window
agreement alone does not certify the rest of a missing scene.

**Party-move basis and acceptance:** the US image at `$070826..$070847`
([CarnivorousTrees](53_CarnivorousTrees.md)) and `$07093C..$07095D`
([SavingKyra](54_SavingKyra.md)) loads the leader's Y, adds `$20` as a word,
calls `CalcPartyNumber` (`$05A6A4`), then writes that same Y to each occupied
party object. The writes are at `$07083C` and `$070952`: `3941 0034`
(`move.w d1, $34(a4)`), with `$40` object strides and `dbf`. They are immediate
position writes; X is untouched. Both port scenes in
`rust/psiv-core/src/scenes/dezo_campaign.rs` instead request a leader-only
`MoveActorOffset { dy: 0x20, wait: true }`; `scene_runner/ops.rs` schedules a
walk and blocks, and the follow chain in `scene_runner.rs` trails followers.
Resolve the shared primitive in those owning paths. Regression coverage in
`rust/psiv-core/tests/events/scenes.rs` and `scene_outcomes.rs` must check both
events, mounted/on-foot entry and one/five-member parties with distinct starting
Y positions: all occupied Y words become `(old leader Y + $20) & $FFFF` before
the battle request, X remains unchanged, and no walking wait is inserted.

These findings are US-image and source/registry evidence. This closeout earned
no runtime, ordinary-native-input, persistence or connected-route evidence for
these gaps; #71 remains open. A later campaign claim needs its own connected
input receipt.

## Guards

- `rust/psiv-core/src/scenes/mod.rs`: `every_dialogue2_caller_is_a_retained_window`
  (the table `DIALOGUE2_CALLERS`: scene, image address, entry; each scene must
  run that one entry through `retained`, and no other scene may) and
  `no_scene_resumes_before_it_opens_a_dialogue`.
- [`tests/test_dialogue2_callers.py`](../../tests/test_dialogue2_callers.py):
  re-derives the caller list and each entry from the US image, ties each
  address to a scene through the documented byte ranges, compares the registry,
  the Rust table and this document, and carries six negative controls (a scene
  back on resume, a wrong entry, a new image caller, a changed image entry, a
  caller dropped from the Rust table, a missing row here).
  `PYTHONPATH=. python3 -m unittest tests.test_dialogue2_callers -v`.

The same census shape (image callers by routine, scene by documented range)
extends to `Event_GetAndRunDialogue` (`$5AC66`) and `Event_GetAndRunDialogue5`
(`$5ADF8`); this pass audited `Dialogue2` only.
