# `Event_FractOozeFound`

- **Retail bytes:** `$072B2C..$072B51` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$7D]` at `$05A2B4`; event `$007D`.
- **Trigger:** Dialogue `$F6 $00 $7D`, `DialogueTree12` entry 38 (Monsen); an `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$072B2E`, row 12 of [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit), issue #71).
- **Data:** `census_events.rs`, `FRACT_OOZE_FOUND` (5 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072B2C..$072B33` | `moveq #$2F, d0` / `jsr Event_GetAndRunDialogue2` (`$5ACDC`) | `retained(0x2F)` |
| 1 | `$072B34..$072B3B` | `moveq #$2F, d0` / `jsr EventFlags_Set` | `SetFlag` `$2F` |
| 2 | `$072B3C..$072B41` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 3 | `$072B42..$072B4D` | `move.b #7, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `7` |
| 4 | `$072B4E..$072B50` | `moveq #1, d0` / `rts` | `Return` `1` |

The retained window stays up through the battle request, as for every `Dialogue2` caller.

## Where the window goes (#644)

Tape 45 (`tools/certify.py` `TAPE_45`) runs the event on Monsen from a fixture.
The C on frame 7799 ends the last page ("It's coming for us!"), and the routine
returns with the window and Rika's portrait up: frame 7800 shows both
untouched. `RunEventBattle` (`ps4.asm:120803`) then runs `FieldRoutine_Battle2`'s
event arm, `loc_58498`: fifteen passes, each writing a white plane-A band two
cells taller (rows 104-119 on 7801, one cell more above and below on each
frame after) and calling `Pal_IncreaseTone`. The band covers the portrait by
7805 and the window by 7812, and the whole screen by 7814. `Game_Mode` turns to
the battle on 7815, which holds white until 7856 and fades the battle in from
7857.

The port builds on the held `Retained` page (entries and resumes, #686).
`battle_entry.rs` owns the fifteen passes and their half-open pixel rows:
`[104,120)`, then eight more pixels at each edge, clipped to `[0,224)`. Each
pass calls the shared CRAM model's `Pal_IncreaseTone` before its DMA/VInt wait;
it changes all 64 words when `Main_Frame_Count & 3` is zero. The Profound
Darkness's separate arm (`loc_58482`) tones for forty frames without a band.
Godot draws the runtime's opaque `$EEE` rows above the held page and field.
The dialogue runner drops the portrait once its packed rectangle is covered
(pass 5), then the page when the text rectangle is covered (pass 12).

The `retained-window-page` pair covers the last page before the press, at
tick 511 against frame 7797. `retained-window-press` and
`retained-portrait-press` cover the window `(24,160)` 272x48 and portrait
`(40,104)` 48x48 on tick 514 against frame 7800. The three
`event-battle-{band-first,portrait-covered,window-covered}` pairs cover the
full width of the band's rows at passes 1, 5 and 12, against frames 7801,
7805 and 7812. Separate comparisons of the toned remainder at passes 1, 5
and 12 match inside the existing field-edge boundary; pass 5's whole
`1,1,319,223` region matches. The full frame still differs on the first row
and column outside the band ([#409](https://github.com/TusanHomichi/PSIV/issues/409));
these band crops do not certify that edge, every intermediate tone frame,
or the subsequent stage sweep ([source notes](../source-notes/README.md#other-recorded-deviations)).

## The battle's entry (#663)

`GameMode_LoadBattle` (`ps4.asm:9911`) fills the palette with `$EEE`, loads the
stage while `Main_Frame_Count` holds (7818-7854), and `loc_6B20` fades the
stage in from white, one nibble a frame toward the battle palette, while the
planes sweep in from off-screen: 7857-7859 show the backdrop alone (`$CCC`,
then `$AAA`), the stage is in from 7860 and settled by 7870. Only then does
`loc_B62A` roll the opening priority (the seed word rotates on 7875). Every
encounter takes the same mode after its own field arm (`loc_584EE`).

The runtime runs the 76 frames from the request to that roll and counts the
36 the cartridge counts (`battle_entry.rs`); clone tick t shows frame
t + 7286. Ticks 528-570 are the white hold and 571-573 the fade's first
steps, equal to frames 7814-7859 over `1,1,319,223`; the `event-battle-*`
pairs certify 7814, 7840, 7857 and 7859. Ticks 574-583 go on fading the
backdrop alone and 583-589 hold it black, where the cartridge sweeps the stage
in and then shows it settled; the battle starts on tick 590 (7876).
