# `Cutscene_Elsydeon`

- **Retail bytes:** `$078584..$078A7B` inclusive, 1272 bytes.
- **Pointer:** `CutscenePtrs[$1E]`; scene `$801E`.
- **Trigger:** the Elsydeon cave interaction after `$801D`; Reunion `$DA` is
  still clear.
- **Data:** `dezo_endgame.rs`, `ELSYDEON` (114 ops).

## Clone audit

Retail `grand_cross=0` selects this cutscene body. The Grand Cross alternative
is excluded. ROM range: `$078584..$078A7C` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-15 | `$078584..$078604` | fresh screen, panel `$129`, 160 frames; sprites on and dialogue `$31` through `Event_GetAndRunDialogue3` (`$0785B2`); 120 frames, the revision>0 `SFXID_Deban`; panels `$12B` and `$12C` with 30 and 120 frames; `MusicID_TheAgeOfFables` | panel/dialogue/sound |
| 16-29 | `loc_78608..$078698` | forty frames of `Pal_IncreaseTone`; four `Panel_Destroy`; panel `$12D`; line 4 saved to `Palette_2_Line_4` and filled white (`loc_78652`); 60 frames; forty of `loc_58666` (`loc_7866A`); 60 frames | `PaletteIncreaseTone`, `PanelDestroyLast`, `PaletteCopy`, `PaletteRestoreTone` |
| 30-49 | `$07869C..$078760` | panel `$12E`, 20 frames, the `Event_RunDialogue3` resume (`$0786AC`); 30 frames, panel `$12F`, 240 frames; the whole buffer saved and filled white (`loc_786F2`), two `Panel_Destroy`, forty restore frames (`loc_7870C`), 60 frames; panel `$130`, 20 frames; the first inline page (`popdlg` at `$078740`) | panel/palette/dialogue |
| 50-77 | `loc_7878A..$0788AE` | 30 frames, panel `$131`, 50 frames, panel `$132`, 240 frames; buffer saved and filled white (`loc_787E4`), three `Panel_Destroy`, the windows' teardown; forty restore frames (`loc_7882E`), 60 frames; panels `$18C`, `$133`, `$134`, `$135`, 160 frames each | panel/palette |
| 78-96 | `loc_788B2..$078996` | forty tone frames, five `Panel_Destroy`, panel `$136`; lines 0 and 1 saved and filled white (`loc_78902`); 120 frames; forty restore frames (`loc_7891A`); the second inline page (`popdlg` at `$07892A`); map update 60; forty tone frames (`loc_7897E`); `Panel_DestroyAll` | palette/panel/dialogue |
| 97-108 | `$0789AE..$078A23` | fresh screen, map update 60, fade in; panels `$139`, `$13A`, `$13B` with 120 and 160 frames; dialogue `$32` through `Event_GetAndRunDialogue5` (`$078A1E`) | panel/dialogue |
| 109-113 | `$078A24..$078A7B` | set Elsydeon `$D9`; Elsydeon `$77` in Chaz's hand; restore the saved party slots; load Inner Sanctuary B1; return | flag/equipment/party/map |

The two inline pages run in the scene's own window code (`Window_Create`,
`RunText`, a button wait) and the cartridge leaves each window up until the
next `Window_Destroy` (`$078810`, `$0789A2`). The port draws them through the
`Event_GetAndRunDialogue3` window, which closes at the button
([source notes](../source-notes/README.md#other-recorded-deviations)).

## Revision-conditional ops

One op runs only on `revision>0` builds, which the US retail cartridge is: the
`SFXID_Deban` sound before the `$12B` panel. It is listed in the [revision audit](REVISION_AUDIT.md#every-other-revision-conditional-op-in-a-transcribed-body).

The restored slots are the retail `Saved_Char_ID_Mem_1..5` bridge; the
[native save carries it](../camp/SAVE_FORMAT.md) while it is pending, so a
CONTINUE between the two scenes restores the parked party
([source note](../source-notes/saved-char-id-mem.md)).
