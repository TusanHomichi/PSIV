# Scene dialogue and map object lifetimes

The connected Alshline return exposed two faults that isolated scene-outcome
tests missed. The first native attempt is preserved under
`build/native-zema-missing-dialogue/`; it reached a save, but is **not a valid
campaign checkpoint** because it skipped the rescue conversations.

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

`build/native-dialogue-trees/` holds the dialogue census, workspace tests and
focused regressions. The native route now observes fully revealed dialogue
pages through a read-only probe and checks both the conversations and seven
restored residents before saving. The intermediate rerun is preserved at
`build/native-zema-dialogue-restored/`: all **50** pages appeared, nine random
encounters were escaped, Igglanova was defeated with normal commands, all seven
residents returned, and slot 1 was saved outside Zema with 3063 meseta. That
run exposed the presentation faults below, so it is not visual parity proof.

## Field entrance and panel cleanup

The old `$0188` object lasted one frame and its subsequent `$8000` Y velocity
was incorrectly represented as a replacement object id. The scene now creates
Igglanova at `(480,160)`, keeps it alive until the explicit slot-7 despawn,
and executes `Event_StepObject` for **64** frames at a signed 16.16 Y step
of `$8000`. It lands at `(480,192)` before the next camera move. The object
retains fractional coordinates across successive moves. Scene timing blocks
for the movement, and full map reloads clear temporary objects.

The sprite extraction now applies the original fourteen palette words at
`$052848..$052863`, retaining Zema's last two words. The old asset wrongly
used the petrified town palette. The sprite plays its two-frame sequence.

Native page 38 also showed old comic panels covering the restored map. Full
map reloads now clear those panel layers. The unfortunately named
`Render_Sprites_In_Cutscenes` byte is a **suppression** flag when event bit 15
is set, and also selects the panel portrait placement. The renderer previously
treated it as an enable flag and gave every scene the panel portrait placement.
The runtime now exposes the active event index; field-event dialogue keeps
its F7 pause behavior while using the appropriate field or panel portrait.
F7 and FF both clear the byte; only the text cursor survives F7. The window
routine's panel destruction also runs on F7, so earlier panels cannot leak
into the resumed section. The original flag write before Alshline's resume
was restored. The missing flag write before the Zema aftermath's `$69`
dialogue was restored from the
US instruction at `$074588` (the scene range is `$074556..$0745DD`).

`build/native-igglanova/` holds the motion, renderer, palette and integrated
checks. The connected rerun in `build/native-zema-return/` completed all
50 pages, the monster entrance position checks, battle, restored residents,
SAVE and fresh title Continue. The halfway capture visibly shows the green
monster. At the recorded arrival the next dialogue panel already covers it;
the position probe records `(480,192)`. Page 38 has its proper new panel and
field portrait, with the old collage cleared. This establishes those fixes
without certifying every staging frame.
Exact fade cadence for the window routine's exit still needs an oracle
comparison; the current cleanup fixes the stale state without certifying that
timing.

The town capture also revealed that the seven active residents still used
their grey sprite sheets. `deferred_effects` already contains the extracted
palette copy's NPC sheet replacements, but the Rust data reader discarded
them. The reader now retains and validates them, the existing map-effect
gate selects the replacements, and Godot uses those selected sheets when
building map objects. The same path gives Birth Valley's NPCs their petrified
palette before the rescue. A regression checks both flag directions and all
seven restored Zema assets. Native re-entry proof is pending in the outfit run.

## The interpreter moved into the runtime (2026-10-01, node S1)

The dialogue rules used to live in the Godot crate: `psiv-godot`'s
`dialogue/text_flow.rs` walked the bytes, `dialogue.rs` owned the window's
clock and a *copy* of the 512-entry event-flag bank, and `dialogue/choice.rs`
read the pad inside the node. Its own header admitted the flow was on the
wrong side of the layer line.

They now live in `rust/psiv-runtime/src/dialogue/`:

- `text_flow.rs`: the retail byte walk (`RunText_CharacterLoop`), unchanged,
  except that every `$FA` reads the live `GameState` at the moment the branch
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

`RuntimeEvent::SceneDialogue`'s handler no longer resolves a tree; it calls
`Runtime::open_scene_dialogue(entry, panel_layout)`, whose three answers are
`Opened`, `Empty` (acknowledge the scene and run on) and `UnknownTree` (the
scene recorded an address no packed tree claims; the dialogue barrier stays
pending, as before).

`psiv-godot/src/dialogue.rs` is a renderer now: it takes a `DialogueView` per
frame and draws the pack's nine frame tiles, the glyph strip, the portrait and
the arrow. `Field::physics_process` builds the `Pad`, hands it to
`Runtime::dialogue_frame` before the field tick, runs the field, and calls
`Runtime::dialogue_tick` afterwards — the order two nodes had before, kept
because a `$F2` flag write must land after the field tick that precedes it.

Evidence: `build/lane-evidence/` (the pre/post test-name lists and their diff,
the `Ctrl::` grep over `psiv-godot`, the negative controls for the new runner
tests, and the gate output). The pagination census over all 2,736 entries
moved with the flow and still matches the extractor exactly. Two behaviour
differences are recorded in the node's receipt: the debug `PSIV_DEBUG_RETAIL_PACE`
harness now dismisses a page with a synthetic `ButtonSpeak` press in the pad
(so the accelerated-frame cadence applies on that frame), and the
field-status window's text is built in the shell and opened by the runtime.

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
- **`$F9` pause length.** The 3-frame draw cadence has 6-frame gaps,
  consistent with an explicit pause control code, but the pause's own duration
  has not been measured against the dialogue data.
- **Scroll-arrow timing.** `Text_Scroll_Arrow` ($FFFFC2C0) is mapped and its
  `offscreen_flag` logged, but that byte reads 0 for the whole dialogue, so it
  does not show the arrow's visibility; the right field has not been found.
