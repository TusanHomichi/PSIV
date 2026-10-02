# Revision-conditional op audit (H17)

`Cutscene_AlysWounded` and `Cutscene_PsycoWand` loaded `DialogueTree6` before
their Krup conversations in the cartridge and not in this port, so the scene
read Krup Inn F1's bound tree 5 instead: entry `$2C` landed on tree 5's empty
entries 44 to 46, the window never opened, and the following `RunDialogueResume`
had no cursor ([runner log H17](../campaign/RUNNER_LOG.md#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree)).
Every one of the four missed loads sits under `if revision>0`, which is why no
op-count test noticed: the transcriptions had skipped a whole conditional
class, not one line.

This is that class fixed. Two independent passes, neither trusting the other:

1. **Retail bytes.** Each scene's documented range in the US image is scanned
   for the ten-byte `move.l #<tree>, d0 / jsr (DialogueTreesToRAM).l`
   (`20 3C xxxxxxxx 4E B9 00 05 3F 00`, `DialogueTreesToRAM = $53F00`). This is
   decode-free and authoritative: it reports what the cartridge runs, whatever
   the clone's text says.
2. **The clone's conditionals.** Every `if revision…` / `else` / `endif` inside
   each transcribed scene's body in `reference/ps4disasm/ps4.asm` is listed with
   the branch the port took. The clone is a fallible map and its build-address
   comments come from a Grand Cross build, so it is used for *structure* only —
   `AGENTS.md`, "Retail and layer boundaries".

This project builds **`revision = 1`** (English,
`reference/ps4disasm/ps4.options.asm:4`), so the `if revision>0` branch is the
one the port must take, and a `revision=0` block must not reach the registry.

## Tree-load census

Retail `DialogueTreesToRAM` calls per scene range, against the registry's
`SceneOp::SetDialogueTree` count. Guards:
`rust/psiv-core/src/scenes/mod.rs::every_scene_loads_the_trees_its_retail_bytes_load`
(the checked-in table, no ROM needed) and
[`tests/test_scene_trees.py`](../../tests/test_scene_trees.py), which re-derives
the retail column from the image and fails if this table rots.

| Scene | Retail bytes | Retail loads (tree @ address) | Port before | Port after |
|---|---|---|---|---|
| `Cutscene_AlysWounded` | `$074B72..$0751FF` | 3: 6 `@$074D5A`, 6 `@$075044`, 5 `@$0751F0` | **1** | 3 |
| `Cutscene_PsycoWand` | `$075200..$075A11` | 3: 6 `@$07533C`, 6 `@$0758EA`, 5 `@$075A02` | **1** | 3 |
| `Cutscene_ZioDefeated` | `$075A12..$075FC7` | 1: 36 `@$075D84` | **0** | 1 |
| `Cutscene_Alshline` | `$0741E6..$074555` | 3: 3 `@$07434E`, 3 `@$0744EE`, 4 `@$074528` | 3 | 3 |
| `Cutscene_ZemaIgglanovaDefeated` | `$074556..$0745DD` | 1: 3 `@$07457A` | 1 | 1 |
| `Cutscene_MeetingRika` | `$0745DE..$074A7D` | 1: 34 `@$0749A2` | 1 | 1 |
| `Cutscene_CrashLaanding` | `$07658A..$07714F` | 2: 14 `@$076758`, 14 `@$076DE2` | 2 | 2 |
| `Cutscene_LashiecDefeated` | `$077A68..$077BD9` | 2: 22 `@$077BB2`, 38 `@$077BCA` | 2 | 2 |
| `Cutscene_GumbiousBishop` | `$077DC6..$077EAB` | 1: 39 `@$077E58` | 1 | 1 |
| `Cutscene_MeetingSeth` | `$077EAC..$077F2D` | 1: 42 `@$077EC8` | 1 | 1 |
| `Cutscene_Rykros` | `$07818E..$078345` | 1: 39 `@$0781C4` | 1 | 1 |
| `Cutscene_Ending` | `$078F3E..$07A811` | 1: 42 `@$078F3E` | 1 | 1 |
| `Cutscene_MeetingKyra` | `$077788..$077895` | 1: 37 `@$077798` | 1 | 1 |
| `Event_CarnivorousTrees` | `$070774..$070855` | 1: 37 `@$070812` | 1 | 1 |
| `Event_SavingKyra` | `$070856..$070975` | 2: 37 `@$0708F4`, 37 `@$070928` | 2 | 2 |
| `Event_EclipseTorchUsed` | `$07018A..$070481` | 1: 37 `@$070452` | 1 | 1 |
| `Event_GameStart` | `$073946..$073ECD` | 4: 17 `@$073952`, 17 `@$073A6A`, 17 `@$073AEA`, 17 `@$073BC8` | 4 | 4 |
| `Event_PiataGuardsReprimand` | `$0738D2..$073945` | 2: 43 `@$073924`, 1 `@$073938` | 2 | 2 |

Addresses are the `move.l #<tree>, d0`; the `jsr (DialogueTreesToRAM).l` that
executes it is six bytes later.

Every other transcribed scene's range holds **zero** `DialogueTreesToRAM`
calls, and every one of those scenes has zero `SetDialogueTree` ops.

### `Cutscene_ZioDefeated` is not a revision defect

Its missing load is the only one in the table that is *not* under a revision
block. `$075D84` sits unguarded after the `Event_MotaSpaceportAppearing` call,
before the `$73`/`$74`/`$75` panels and the `popdlg` resume that follows them
(`ps4.asm:155204`). The port's resume therefore re-read the Motavia binding
instead of tree 36. Fixed in the same pass; the guard covers it either way.

## Every other revision-conditional op in a transcribed body

| Scene | Block (clone line) | Retail branch content | Port status |
|---|---|---|---|
| `Cutscene_AlysWounded` | `154305`, `154465` `if revision>0` | `DialogueTree6` loads before dialogues `$2C`, `$2D` | **was missing** → added |
| `Cutscene_AlysWounded` | `154558` `if revision>0` | `DialogueTree5` restore at the end | present |
| `Cutscene_PsycoWand` | `154633`, `154938` `if revision>0` | `DialogueTree6` loads before `$2E` and the late resume | **was missing** → added |
| `Cutscene_PsycoWand` | `155002` `if revision>0` | `DialogueTree5` restore | present |
| `Cutscene_Alshline` | `153741`, `153835`, `153852` | trees 3, 3 and 4 | present (3 ops) |
| `Cutscene_ZemaIgglanovaDefeated` | `153881` (nested) | tree 3 | present |
| `Cutscene_ZioDefeated` | `155017` `if revision=0` | three `SFXID_Tandle` + 2-frame waits + `#*$27` | **not taken**: the port plays the revision-1 branch (common Tandle + two, waits 6/6/3, `$77` → 120 frames) |
| `Cutscene_Elsydeon` | `157483` `if revision>0` | `move.b #SFXID_Deban` before the `$12B` panel | **was missing** → added (`SFX_DEBAN = $DC`) |
| `Cutscene_Ending` | `158031` `if revision>0` | hold Menu (`ButtonCamp`) at scene start jumps to the credits | not ported; see "Recorded deviation" below |
| `Cutscene_Ending` | `158656` `if revision=0` | two credit lines `$12`/`$13` at planes `$850C`/`$880C` | **not taken**: the port transcribes the `else` branch — lines `$12`–`$15` at `$8406`/`$8586`/`$8706`/`$8886` |
| `Cutscene_ProfoundDarkness` | `157992` `if revision=0` | palette line 2 forced to `$EEE` | **not taken**: the `else` ramp is the one the port's presentation follows |
| `Event_AngerTowerTop` | `151969` `if revision>0` | `jsr (RecoverStats)` | present |
| `Event_ChazHouse` | `149090` `if revision=0` | `popdlg` + `cmpi.b #$AB, -$2(a0)` (reads the drawn line) | **not taken**: the port's `BranchChoice` is the revision-1 `Yes_No_Option` test |
| `Event_DarkForce2` | `150061` `if revision=0` | `Saved_Sound_Index = MusicID_DezorisField2` | **not taken**: the port sets `Sound_StopAll`, the `else` value |
| `Event_EclipseTorchUsed` | `149579` `if revision=0` | `wait $1D`, `SFXID_BarrierBroken` | **not taken** |
| `Event_EclipseTorchUsed` | `149609`… `if revision>0` | `SFXID_Deban`, palette copies, `DoMapUpdateLoop` waits | present (the `SFX_DEBAN` op and the waits) |
| `Event_GameStart` helpers | `183609`, `183629` `if revision=0` | `GameStart_FadeToWhite`/`GameStart_PlaneAFadeOut` pace on `#7`; the `else` uses `#3` | helper routines outside the event body; the intro's fade cadence is the renderer's (see [01](01_GameStart.md)) |

Only the four `DialogueTree6` loads and the tree-36 load change scene state;
`SFXID_Deban` changes one sound byte. Everything else above is the port
already taking the revision-1 branch, or a revision-0 branch it correctly
skips.

## Recorded deviation: the Menu-button credits skip

`Cutscene_Ending` (`ps4.asm:158029-158033`) lets a US/EU player holding Menu at
the scene's start jump straight to the credits (`loc_79D0C`). The port's ending
transcription has no input branch: it plays the whole ending. This is an
affordance, not state — no flag, party, inventory or map edge depends on it —
and adding it would mean a new held-button branch op and a second entry point
into the ending's 367 ops. Recorded here rather than silently dropped, per
`AGENTS.md`. The rest of the ending follows the revision-1 branch, verified
above.
