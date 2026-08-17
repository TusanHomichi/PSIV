# `Event_MeetingLeRoof`

- **Retail bytes:** `$070482..$07069D` inclusive, 540 bytes.
- **Pointer:** `EventPtrs[$48]`; event `$0048`.
- **Trigger:** `RunEventsJmpTbl[$33]`, Le Roof `$D1` clear.
- **Data:** `dezo_campaign.rs`, `MEETING_LE_ROOF` (17 ops).

## Clone audit

The retail `EventPtrs` body is selected by the `grand_cross=0` table at
`ps4.asm:120622`. `reference/ps4.options.asm:10` sets `grand_cross=1` for the
clone build, so its hack-only body is excluded. ROM range: `$070482..$07069E`
exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-3 | `$070482..$07051F` | move Chaz to `($1F0,$1E0)`, red fade and waits | actor/fade/wait |
| 4-8 | `$070520..$0705B7` | tower art `$1DB2C8`, scene asset `$1DDE0A`, DMA, red return | presentation |
| 9-12 | `$0705B8..$07065F` | waits, dialogue entry `0`, music stop/reload | dialogue/presentation |
| 13-16 | `$070660..$07069D` | tower music `$9A`, set `$D1`, return | music/flag |

