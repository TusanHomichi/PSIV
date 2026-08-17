# `Event_JuzaDefeated`

- **Retail bytes:** `$06FB5E..$06FBED` inclusive, 144 bytes.
- **Pointer:** `EventPtrs[$41]`; event `$0041`.
- **Trigger:** `RunEventsJmpTbl[$2F]`, Juza `$41` set and Juza Defeated `$48`
  clear.
- **Data:** `post_zio_cutscenes.rs`, `JUZA_DEFEATED` (6 ops).

## Clone audit

The body at `ps4.asm:149162` is the retail EventPtrs[$41] body selected by
the `grand_cross=0` table. Actual-ROM range: `$06FB5E..$06FBEE` exclusive end.
Grand Cross-only bodies and FortuneTeller records remain excluded.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$06FB5E..$06FB6F` | Door Opened SFX `$E2` | sound |
| 1-4 | `$06FB70..$06FBEC` | four repeated map-layout tile groups and update loops | typed object animation |
| 5 | `$06FBEC..$06FBED` | set Juza Defeated `$48` | flag |

The tile writes are presentation-owned; the flag is the only persistent story
edge. The next dispatch in the same retail trigger table is Rune Flaeli
`$30`, not a hidden Grand Cross FortuneTeller scene.

