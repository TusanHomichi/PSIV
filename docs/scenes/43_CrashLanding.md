# `Cutscene_CrashLaanding`

- **Retail bytes:** `$07658A..$07714F` inclusive, 3,014 bytes.
- **Pointer:** `CutscenePtrs[$0F]` at `$05A580`; scene event `$800F`.
- **Trigger:** `RunEventsJmpTbl[$29]` after Chaos Sorcerer `$71` is set.
- **Data:** `post_zio_cutscenes.rs`, `CRASH_LANDING` (120 ops).

## Clone audit

The spelling `CrashLaanding` is the pointer-table name in the `grand_cross=0`
branch (`ps4.asm:120779`); the body label is `Cutscene_CrashLanding` at
`ps4.asm:155745`. The actual ROM pointer pair is `$07658A..$077150`; no
Grand Cross-only body is used.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-19 | `$07658A..$076652` | crash panel `$80`; Foi/Tandle/Spark/Legeon sequence; dialogue entry `3` | panels/SFX/dialogue |
| 20-38 | `$076664..$0767A4` | blast panel `$86`; Megid/Legeon/`$AD`; variable red fade; stop music | panels/fade |
| 39-48 | `$0767A5..$07682A` | reload; map-update `120`; Dezolis field panel `$87`; load tree 14; entry `7` | tree/dialogue |
| 49-58 | `$07682B..$0769D1` | load Dezolis `$001` from Zelan Space `$18C` at `($20,$BA)`; hide party sprites; camera `($240,$5D0)` | map/camera/presentation |
| 59-72 | `$0769D2..$076A9A` | load Raja Temple `$14C` from Dezolis; park party at `($5F0,$120)`; flag `$85` | map/flag/staging |
| 73-84 | `$076A9B..$076C8A` | Raja-temple flight/camera; elevator object; panel `$88`; dialogue continuation | typed motion/dialogue |
| 85-96 | `$076C8B..$0770D1` | Wren/Chaz/Rika/Rune motions and facing; final dialogue resumes | actor motion/dialogue |
| 97-98 | `$0770D2..$07714F` | copy Raja into party slot 5, macro `8`; set `$88`; return `1` | party/macro/flag |

The long flight, palette flash and temporary elevator object are retained as
typed presentation/motion records. The durable edges are the Dezolis and Raja
Temple loads, `RajaTemple=$85`, Raja in slot 5 (zero-based slot 4), and
`RajaJoined=$88`.

## Verification and boundary

The headless arc ends this beat at Raja Temple with party
`[Chaz,Rika,Rune,Wren,Raja]`. The next movement to Hangar/Dezo Spaceport is
player-controlled; the next fixed cutscene is `Landale` (`$8010`).
