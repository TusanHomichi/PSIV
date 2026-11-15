# Scene dialogue and map object lifetimes

How a scene picks its dialogue tree, how long the objects a scene adds or
removes live, where the dialogue rules run, and what the cartridge's text and
talk timing is.

## Dialogue tree selection

The current selection owner and full US-ROM census are in
[Dialogue tree selection](DIALOGUE_TREE_SELECTION.md). Type-0 interaction
areas use the live world table (including Motavia's high-entry override);
NPCs and default scenes retain the map tree. The historical scene-address
repair below concerns explicit scene tree loads, not world-area selection.

`SetDialogueTree` reached the presentation state, but the Godot lookup only
recognized trees 17, 39 and 42. Alshline explicitly selects tree 3 while
standing in Zema, whose map uses tree 4. The renderer silently used tree 4's
empty entries `$67..$69`, then reported that `RunDialogueResume` had no saved
cursor. Consequently all fifty pages of the three conversations disappeared.

The data reader now retains the pack's existing `rom_offset` metadata. The
renderer builds its address lookup from all 43 extracted trees and preserves
that lookup across scene resets. A scene with no explicit tree uses its map's
tree; an unknown explicit address reports an error and leaves the dialogue
barrier pending instead of silently advancing with unrelated text. Opening
plane text uses the same metadata. The pack reader rejects malformed or
duplicate supplied addresses.

The scene census also found five invalid late-game constants. Their corrected
values are present in the verified US ROM's `move.l #address,d0` instructions:

| Tree | Correct address | Example instruction |
| --- | --- | --- |
| 22 | `$1EF130` | `$077BB2`, Lashiec aftermath |
| 37 | `$1F9580` | `$077798`, Meeting Kyra |
| 38 | `$1FA480` | `$077BCA`, Lashiec aftermath |
| 39 | `$1FAAC0` | `$077E58`, Gumbious bishop |
| 42 | `$1FC920` | `$077EC8`, Meeting Seth |

The previous constants were `$1F471C`, `$2080BE`, `$209B36`, `$20A650` and
`$20DB0E` respectively. Older scene transcription notes containing those
addresses are superseded by this byte check. This corrects tree selection;
it does not certify the remaining staging of those late scenes.

## Temporary despawns

Alshline removes the seven Zema residents while staging Igglanova. Battle
return must preserve those live object removals; the post-battle full map
load must reconstruct the residents using the current map and story flags.
An interim session-wide despawn ledger incorrectly removed them again on
every subsequent map entry, producing a rescued town with nobody in it.

The existing extracted map effects already supply persistent, flag-gated
despawns. Full map loads must use those effects rather than replaying temporary
scene removals. NPC promotion must remove the live NPC immediately, just like
an explicit despawn. Battle/chunk refreshes retain live object state.

## Verification

The dialogue census and focused regressions cover tree selection, and the
campaign route plays the Zema rescue conversations (all 50 pages) with ordinary
input before it saves.

## Field entrance and panel cleanup

The scene creates Igglanova at `(480,160)`, keeps it alive until the explicit
slot-7 despawn, and executes `Event_StepObject` for **64** frames at a signed
16.16 Y step of `$8000`. It lands at `(480,192)` before the next camera move.
The object retains fractional coordinates across successive moves. Scene timing
blocks for the movement, and full map reloads clear temporary objects.

The sprite takes the fourteen palette words at `$052848..$052863`, keeping
Zema's last two words (not the petrified town palette), and plays its two-frame
sequence.

Full map reloads clear comic panel layers. The unfortunately named
`Render_Sprites_In_Cutscenes` byte is a **suppression** flag when event bit 15
is set, and also selects the panel portrait placement. The runtime exposes the
active event index; field-event dialogue keeps its F7 pause behavior while
using the appropriate field or panel portrait. F7 and FF both clear the byte;
only the text cursor survives F7. The window routine's panel destruction also
runs on F7, so earlier panels cannot leak into the resumed section. A flag
write precedes Alshline's resume, and another precedes the Zema aftermath's
`$69` dialogue (US instruction at `$074588`, scene range `$074556..$0745DD`).
A panel cutscene writes the byte again before each later text, since the last
one's close cleared it; a transcription that writes it only before its first
text draws every later portrait at the talk position (#630). `psiv-core`'s
`scenes::panel_byte_census` holds every registered scene to its documented
bytes: each `move.b #1` (`11FC 0001 ECFD`) and `clr.b` (`4238 ECFD`) to the byte
is a `SetRenderSpritesInCutscene` of the same value, in the same order.
`Event_GetAndRunDialogue5` and `Event_RunDialogue5` (`$05ADF8..$05AE72`,
`ps4.asm:121718-121747`) destroy their text windows, run
`PalFadeOut_ClrSpriteTbl`, then destroy the panels and clear that byte. The
runtime's shared dialogue return (`dialogue/close.rs`) waits for the CRAM
program before releasing the scene, for both `Cutscene` and `Cutscene5`,
including `$F7` suspensions. Panels stay up through the fade, and the display
stays off until a later enable. The pack regression follows Alshline's
`$0743EA` resume through its map reload and Igglanova construction to
`$074494`'s explicit enable, with a non-fading-window negative control.
This proves state and frame accounting; the whole exit cadence has not been
compared with the oracle.

A caller that shows the field again restores a palette and enables the display
itself. MeetingRika's two same-map `RefreshMap` calls (`$074632`, `$07475C`)
reload the field palette before their fade-ins; Alshline copies the saved
palette back at `$074366..$07437A`. Dorin's `$07411C` and Saya's `$074D12`
fade-ins reveal their rebuilt maps before departure or dialogue. The inline
windows in AlysWounded, ZioDefeated and CrashLanding use `DialogueWindow::Inline`:
their surrounding scene ops own the fade, so they must never borrow Get5's
exit. The caller census rejects that mistake; pack regressions exercise the
continuations with packed dialogue/Speak and with headless acknowledgements.

The extracted palette copy's NPC sheet replacements (`deferred_effects`) are
kept by the data reader and selected by the map-effect gate, so Zema's seven
residents take their restored sheets and Birth Valley's NPCs their petrified
palette before the rescue. The copy's own words reach the CRAM model too,
so those sheets draw from them
([CRAM state](SCENE_PRESENTATION.md#cram-state)). Regressions check both flag
directions, all seven restored Zema assets and both palette buffers.

## Where the dialogue rules run

The dialogue rules live in `rust/psiv-runtime/src/dialogue/`:

- `text_flow.rs`: the retail byte walk (`RunText_CharacterLoop`); every `$FA` reads the live `GameState` at the moment the branch
  is evaluated and `$F2` 11 writes it. There is no flag copy left.
- `mod.rs`: the `DialogueRunner` — the open animation, the typewriter, the
  yes/no window, the `$F7` cursor, `$F6`, the pending event, the tree — and
  the presentation signals it produces.
- `choice.rs`: the cursor and answer rules (`Cancel` is NO, `Speak` answers
  the cursor's row).
- `open.rs`: the pack's trees, the preamble `$FA` chains, the system messages
  and the scene resumes. A scene's line resolves against the tree its
  `SetDialogueTree` op selected (the map's own binding until then), so the
  entry index means what the cartridge meant it to mean — and a headless run
  resolves it without a presentation layer. The renderer keeps its own copy of
  that address for the panel-text path (`DrawTextToPlane`).
- `glue.rs`: the runtime API the shell calls, including the two frame halves.

Next to it, `rust/psiv-runtime/src/pad.rs` is the joypad byte
(`ButtonUp`=0 … `ButtonStart`=7, `ps4.constants.asm:1877-1884`), the runtime's
only input type.

`RuntimeEvent::SceneDialogue`'s handler calls
`Runtime::open_scene_dialogue(entry, panel_layout)`, whose three answers are
`Opened`, `Empty` (acknowledge the scene and run on) and `UnknownTree` (the
scene recorded an address no packed tree claims; the dialogue barrier stays
pending, as before).

`psiv-godot/src/dialogue.rs` is a renderer now: it takes a `DialogueView` per
frame and draws the pack's nine frame tiles, the glyph strip, the portrait and
the arrow. `Field::physics_process` builds the `Pad`, hands it to
`Runtime::dialogue_frame` before the field tick, runs the field, and calls
`Runtime::dialogue_tick` afterwards, because a `$F2` flag write must land after
the field tick that precedes it.

The pagination census over all 2,736 entries matches the extractor exactly.
The debug `PSIV_DEBUG_RETAIL_PACE` harness dismisses a page with a synthetic
`ButtonSpeak` press in the pad (so the accelerated-frame cadence applies on that
frame), and the field-status window's text is built in the shell and opened by
the runtime.

## Continuations and choices

**A scene's dialogue yields to its caller.** Retail `$F7` reaches
`TextCtrlCode_Terminate3` and saves the following text address before
returning to the event, so the scene's own ops run between chunks. The
post-Igglanova entry (tree 33, entry 18) has two such breaks: Chaz turns before
the second chunk, and Hahn moves before the third. The window keeps the tree
and cursor while those ops execute.

**Choice offsets start after both operands.** `TextCtrlCode_YesNo`
(`ps4.asm:142780`) stores the selection in `Yes_No_Option`, advances past both
operands, and calls `GetOffsetByID`. Zero continues in the current entry;
positive values count subsequent `$FF` delimiters. Cancel selects NO.
`Event_ChazHouse` (`:149074`) consults that saved answer after the response
closes: both branches set its temporary visit flag, and only YES calls the
recovery ([what a rest restores](../camp/SHOPS.md#what-a-night-restores)).

## Measured on the cartridge

Measured on the cartridge with the [oracle](../../oracle/README.md); frame numbers cite marks in
the named tape's log, which `oracle/verify.sh` regenerates.

### Talk range: facing only, not beside

**Settled, and it contradicts the remembered behaviour.**
`Interaction_ChkObjects` (`ps4.asm:118729`) builds a probe point from the
player's position projected to its step destination, plus a one-cell offset in
the facing direction from `FieldObj_MovementsTbl`, then accepts an object only
if both `|dx|` and `|dy|` are `<= $80000`. In the 16.16 fixed point these
positions use, `$80000` is **8.0 pixels**. The accept box is therefore 16×16
px — exactly one cell — centred one cell ahead of facing.

An NPC directly beside the player is 16 px off-axis, which fails the test.
**Standing beside an NPC and pressing Speak cannot talk to it.**  Beside-talk
is not supported by the cartridge.

Empirically confirmed on the facing side: Speak while facing an adjacent NPC
opens the dialogue (`logs/03_npc_talk.csv`, mark `speak`). The beside-but-not-facing press is tape 11, below.

### Dialogue timing

From `logs/03_npc_talk.csv`:

- Speak pressed f7185 → `Game_Mode_Routine` = 8 (`FieldRoutine_Interaction`).
- f7187 `Window_Render_Mode` = `$0600` — the open animation starts.
- f7196 `Windows_Opened_Num` = 1, render mode back to 0. **Window open takes
  9 frames; 11 frames from press to open.**
- The window then stays open with no input for the full 300-frame observation
  window. **The final page does not auto-close; it requires a press.**
- A press closes it over 9 frames (f7793 → f7802) and control returns at
  f7803.

**Text draws progressively at one character every 3 frames.** Sampling
`Win_Tile_Buffer` (`$FFFFE220`, `constants:2106`) every frame during the
dialogue shows writes at f7197, 7200, 7203, 7206, 7209, … in a strict 3-frame
cadence, with occasional 6-frame gaps (consistent with `$F9` pauses or line
breaks). At 60 Hz that is **20 characters per second**.

Retail is a 3-frame-per-character typewriter.

**The open steps.** `loc_686BC` (`ps4.asm:139790`) redraws the box two cells
wider a pass. `Event_RunDialogue3` and the other text routines set
`Window_Render_Mode` bit 1, whose `bchg #2` sends only every other pass, the
first included, through `DMAPlane_A_VInt`. The 34-cell box therefore shows 2,
6, 10 ... 34 cells on its nine frames (tape 43, frames 8203-8211, 16 to 272
pixels wide); `DialogueRunner` steps the same widths.

**Portrait loads are lag frames.** `TextCtrlCode_Portrait` (`ps4.asm:142661`)
destroys the portrait that is up with one `DMAPlane_A_VInt` (`$06A1B4`), then
decompresses the new art (`jsr NemDecomp`, `$06A1D6`) and writes its window
to `Plane_A_Buffer`, which reaches the screen with the next glyph's
`DMAPlane_A_VInt`. The VInts the decompression straddles find `VInt_Flag`
clear: they run no `ReadJoypads`, no `UpdateRNGSeed` and no
`Main_Frame_Count` (`ps4.asm:548-549`, `614-624`). Tape 43's
`Cutscene_Rykros`, by `main_frame_count`:

| What | Frames | Lag frames |
|---|---|---|
| a page with a new portrait (ids 1, 4, 6, 8) | clear 7400, destroy 7402, portrait and first glyph 7405 | 7401, 7403-7404 |
| a page without one | clear 7576, first glyph 7578 | 7577 |
| a new `Event_RunDialogue3` window with a portrait | box open 8211, portrait and first glyph 8216 | 8212-8215 |
| a new `Event_RunDialogue5` window with a portrait | box open 8619, portrait and first glyph 8623 | 8620-8622 |

Every portrait change on the tape lags two frames between its destroy and the
new portrait (7403-7404, 7467-7468, 7723-7724, 7915-7916, 8027-8028,
8507-8508). The runner models those two (`PORTRAIT_LOAD_LAG_FRAMES`): on a lag
frame it, the field tick and the RNG do nothing, and the portrait shows with
the first glyph after its `$F4`. `$F4 0` only restores the plane, with no
frame. Every `$F4` is its own change at its own glyph, in stream order:
two adjacent ones spend a load's two lag frames, then the next one's destroy
and lag. The typewriter never draws past a `$F4` whose frames have not run,
however the walk reached it (typing, a `$F9` running out, a `$F2` or a
branch), and a press cannot leave the page, answer its prompt or close the
window before they have. A control with an effect of its own after a `$F4`
(`$F2`, `$F6`, `$FA`) waits for those frames too. A `$F4 0` costs no frame
and no press, and the plane keeps what it shows until a `DMAPlane_A_VInt`
changes it: the next glyph, or a swap's destroy frame. The other lag on the table, the text routine's own work after the box
opens and after a page clears, is not modelled as lag: the typewriter's first
three frames stand in for it (deviation in
[the source notes](../source-notes/README.md)).

### `RunText3`: the ending's windows page on their own

`Cutscene_Ending`'s windows run their text through `RunText3`
(`ps4.asm:144053`), not `RunText`: the inline `RunDialogue(9, EndingIntro)`
(`$078F68..$078F90`) and every `Event_GetAndRunDialogue4`/`Event_RunDialogue4`
(`$5ADA2`, `DialogueWindow::Ending`). The loop reads no pad and draws no
arrow. It types a glyph a frame (`moveq #0, d0`, the US branch), breaks the
page where `RunText` would, and at the break (`Text3CtrlCode_Interrupt`) or
the end (`Text3CtrlCode_Terminate`, for `$F7` and the terminator) waits
`9n + 1` frames, `n` being the page's glyph count (`$FFFFED5B`). The glyph
that fills the second line branches to the interrupt before its own `.wait`,
so that page's wait starts on its own frame. `Event_RunDialogue4` then
destroys the windows with no `RunTextTerminateArrowLoop`.

Tape 38 logs `$FFFFED5B` (cleared at each turn) and `Main_Frame_Count` through
entry `$09`: every page turns `9n + 2` frames after the frame that drew its
last glyph (21 glyphs, 7280 to 7471; 43, 7519 to 7908; 8, 7916 to 7990 ...;
16, the last, 9747 to 9893), and the tape's C taps change nothing. A `$F9`
before the break adds its own frames, after the glyphs before it: 29 on the
page that ends 9544 to 9728, 184 frames for 17 glyphs.

`DialogueRunner` runs a scene window whose `DialogueWindow::runs_text3` holds
on that clock (`dialogue/text3.rs`): one glyph a frame whatever the pad, no
arrow, no press read, and the page turns, or the window closes, on its own.
`runner_tests/text3.rs` pins every page of entry `$09` against the tape, and
`session/ending_text3_tests.rs` plays `$8021` with no press and with presses,
frame for frame the same. The seventh page turns 248 counted frames after
its last glyph (263 wall-clock frames, including panel `$143`'s four and
`$144`'s eleven lag frames). Its `$F2` waits use the
[shared action clock](DIALOGUE_ACTIONS.md#retail-timing-and-semantics), with
no subtracted action-frame allowance in the regression.

**A `$F9` starts after the glyphs before it.** The pause is a byte of the
text loop, so its frames run once the typewriter has drawn the page up to it,
in `RunText` as in `RunText3`.

### A window opening is not proof of a talk

`Interaction_ContinueChecks` ends with
`beq.w Interaction_DoPlayerNothingMsg` — **pressing Speak at nothing opens a
window too**, carrying the player's "nothing here" line. `Windows_Opened_Num`
going 0 to 1 therefore proves only that *some* window opened.

That matters for anything reported off this harness: `Windows_Opened_Num` is safe as a talk indicator for a *positive* test (the
facing press really did open NPC dialogue), but it cannot support a negative
test. **The sound discriminator is `Text_Buffer` ($FFFF7000):** capture a
Speak in open ground as the "nothing here" reference and compare the bytes.

### Beside-press: disproven on hardware

Tape 11. The leader parks at cell (17,7) facing RIGHT, held there by the wall
at (18,7), with a map $13 NPC at rest at (16,7) — orthogonally adjacent,
directly behind the facing. An earlier Speak in open corridor supplies the
reference message.

**The beside press draws byte-identical text to the open-corridor press.** It
took the "nothing here" path; the adjacent NPC was never reached. Combined with
the facing press opening real dialogue, **talk range is facing-only**, with hardware evidence rather than only a
reading of the code.

Staging it has one trap: **the NPCs wander, so a test staged from pack spawn
cells is invalid by the time the leader arrives.** The tape's wait is therefore *measured* — the run is first
made with the leader parked, the log scanned for a frame where an at-rest
object is orthogonally adjacent but outside the faced cell, and the press
placed at that offset. A test that skips this fails silently: the NPC moves into
the faced cell and the "beside" test becomes a facing test.

### Accept press during the window-open animation

Sweeping the press offset across the animation (`python3 -m oracle.scripts.anim_sweep`) and then
holding the button (tape 15) shows what is really happening, and it is
simpler.

Speak lands at f7185, `Window_Render_Mode` goes `$0600` at f7187, and the
window opens at f7196. Draws are counted as `Text_Buffer` writes, relative to
the open frame:

| press offset (from Speak) | inside animation | extra draws vs control |
|---|---|---|
| +4 … +7 | yes | **0 — identical to control** |
| +8, +9 | yes | +1 |
| +10, +11 | yes | +2 |
| +12 and later | no (window already open) | +1 … +2 |

And holding the button rather than tapping it, over a 40-frame hold well after
the window is open:

| | draws in the 40-frame span | gap between draws |
|---|---|---|
| control | 13 | 3 frames |
| holding Speak | **39** | **1 frame** |

**The mechanic is hold-to-accelerate, not a buffered page advance.** Text
normally draws one character every 3 frames; **while Speak is held it draws one
character per frame**, a 3x speed-up, for exactly as long as the button is
down. The "extra draws" in the offset table are just a 4-frame hold overlapping
the draw schedule.

Two consequences for the renderer:

- **A press landing in the first half of the open animation (Speak+4 to
  Speak+7) really is dropped** — those offsets reproduce the control draw
  sequence exactly. So "swallowed" is right for that window.
- What we are missing is not press buffering but **hold-to-fast-forward while
  text is drawing**. Implementing a queued page-advance would be wrong.

### Holding Speak across a page boundary

The NPC line in tape 03 is **two pages**. With no input, page 1 draws 58
characters at one per 3 frames and finishes at f7371; the window then sits
indefinitely. Three runs settle what a held button does (tape 16, measured on
`Text_Buffer` writes):

| run | page 1 draws | page 1 cadence | page 2 |
|---|---|---|---|
| no input | 58, ends f7371 | 3 frames | never starts |
| **hold Speak across the boundary** | 58, ends f7257 | **1 frame** | **never starts** — dead stop while still held |
| press after page 1 completes, keep holding | 58, ends f7371 | 3 frames | **55 draws from f7395, 1-frame cadence** |

**Advancing a finished page needs a fresh press; a held button will not do it.**
Holding drew page 1 three times faster and then stopped at the boundary for the
remaining ~1100 frames with the button still down.

**But the next page does start accelerated if the button is still held.** After
a fresh press advances the page, page 2 drew all 55 of its characters at one per
frame while the hold continued.

So the two behaviours are independent and should be implemented as such:

- **page advance is edge-triggered** — a press, not a level;
- **draw acceleration is level-driven** — it applies to whatever page is
  currently drawing, including a page that has just been advanced into.

A single 24-frame gap separates the pages when advanced by a tap (page 1 ends
f7371, the tap lands f7393, page 2's first draw is f7395), so the advance costs
about two frames beyond the press itself.

### Still unmeasured

- **No page-index RAM field.** Tape 16 pins the page *boundary* behaviour, but
  only by watching `Text_Buffer` writes. Nothing in the RAM map distinguishes
  page N from page N+1, so a tape cannot assert "we are on page 2" directly.
- **`$F9` pause length in `RunText`.** The 3-frame draw cadence has 6-frame
  gaps, consistent with an explicit pause control code. Tape 38 measures the
  pause in `RunText3` (its operand in frames, after the glyphs before it; see
  above), but no tape has measured one in `RunText`.
- **Scroll-arrow timing.** `Text_Scroll_Arrow` ($FFFFC2C0) is mapped and its
  `offscreen_flag` logged, but that byte reads 0 for the whole dialogue, so it
  does not show the arrow's visibility; the right field has not been found.
