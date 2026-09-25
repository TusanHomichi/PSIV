# Scene dialogue and map object lifetimes

The connected Alshline return exposed two faults that isolated scene-outcome
tests missed. The first native attempt is preserved under
`build/native-zema-missing-dialogue/`; it reached a save, but is **not a valid
campaign checkpoint** because it skipped the rescue conversations.

## Dialogue tree selection

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
