# `Cutscene_MeetingKyra`

- **Retail bytes:** `$077788..$077895` inclusive, 270 bytes.
- **Pointer:** `CutscenePtrs[$13]`; scene `$8013`.
- **Trigger:** the completed `$35` custom arm after Saving Kyra `$95`.
- **Data:** `dezo_campaign.rs`, `MEETING_KYRA` (26 ops).

## Clone audit

The retail cutscene table's `grand_cross=0` entry selects this body; the
Grand Cross include is excluded. ROM range: `$077788..$077896` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-10 | `$077788..$0777F5` | panel `$140`, tree `$2080BE`, dialogue `$27`, then Fal and panel `$99` | panel/dialogue/music |
| 11-17 | `$0777F6..$07785A` | dialogue `$28`, waits `60/120`, panel `$9B`, resume | dialogue/presentation |
| 18-21 | `$07785B..$07787D` | write Kyra to party slot 5, macro `$09`, set `$A0` | party/flag |
| 22-25 | `$07787E..$077895` | load Dezolis `($174,$1C)`, Land Master music, mount vehicle 2 | map/vehicle |

