# `Event_KuranArrival`

- **Retail bytes:** `$06FAD4..$06FAE5` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$3D]` at `$05A2B4`; event `$003D`.
- **Trigger:** Kuran `$190`, `RunEventsJmpTbl[$2C]`, Kuran `$86` clear.
- **Data:** `post_zio_cutscenes.rs`, `KURAN_ARRIVAL` (2 ops).

## Clone audit

The retail `else` EventPtrs table at `ps4.asm:120635` names
`Event_KuranArrival`; the body at `ps4.asm:149124` is outside the Grand Cross
hack-only includes. Actual-ROM pointer range: `$06FAD4..$06FAE6` exclusive end.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FAD4..$06FADD` | dialogue entry `4` | standard dialogue |
| 1 | `$06FADE..$06FAE5` | set `EventFlag_Kuran=$86` | flag |
