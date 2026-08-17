# `Event_PsycoWandChest`

- **Retail bytes:** `$06EA16..$06EC61` inclusive, 588 bytes.
- **Pointer:** `EventPtrs[$2F]` at `$05A2B4`; scene event is `$002F`.
- **Trigger:** Ladea Tower F5 `$91`, `RunEvent_FindingPsycoWand` (`$2A`):
  Psycho Wand chest `$109` set and After Alys Death 2 `$67` clear.
- **Data:** `post_rika_events.rs`, `PSYCO_WAND_CHEST` (20 ops).

## Clone audit

The retail pointer range is the authority. This intermediate battle event is
in the `grand_cross=0` path; the Grand Cross script branch is not substituted.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0-3 | `$06EA16..$06EA72` | dialogue 7; stop music; move Rune to leader X/one row above; 60 update/vblank iterations | dialogue/follow/wait |
| 4-6 | `$06EA76..$06EA9C` | barrier SFX `$E6`; barrier object id 4; wait until it clears | sound/object/wait |
| 7-8 | `$06EA9C..$06EAB4` | construct 18 temporary art objects using tile `$2E6`; wait until they clear | object/wait records |
| 9-11 | `$06EAB4..$06EACC` | map-update `$3B+1=60`, stop music, VInt, dialogue 8 | waits/sound/dialogue |
| 12-15 | `$06EAD6..$06EB54` | camera Y `$160`; load tile `$2E6`; stage enemy `$20C`; enemy appearance `$A3`; wait state | camera/art/object/sound/wait |
| 16-18 | `$06EB54..$06EB74` | Black Blood `$A8`; resume dialogue; set Gy Laguiah `$69` | sound/dialogue/flag |
| 19 | `$06EB7A..$06EB88` | event battle index 5; routine-exit bit; return 1 | `StartBattle(5)`, `Return(1)` |

The 18-object loop at `$06EB8A` is kept as a typed temporary-object record;
none of those objects are promoted into the map census. This is why the
Psycho Wand chest event is a distinct beat before cutscene `$800A`.

## Verification

The arc test resolves event battle 5 and asserts Gy Laguiah `$69` before
entering `Cutscene_PsycoWand`.
