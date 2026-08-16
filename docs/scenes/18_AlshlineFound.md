# `Event_AlshlineFound`

- **Retail bytes:** `$06DBD8..$06DBE7` inclusive, 16 bytes.
- **Pointer:** `EventPtrs[$28]` from `$05A2B4`.
- **Trigger:** TonoeBasement_B3 `$4A`, trigger `$18`, chest flag `$08` set and
  `EventFlag_AlshlineFound` clear.
- **Data:** `next_arc.rs`, `ALSHLINE_FOUND_OPS`.

## Clone audit

The retail body is in the clone's `grand_cross=0` conditional branch; the
Grand Cross branch includes a missing scene file. The 16-byte retail stream is
the minimal two-call routine below and was not replaced by the similarly
named dialogue control in the clone.

## Retail transcription

1. Run standard dialogue tree entry `$27`.
2. Set `EventFlag_AlshlineFound` (`$32`).

## Verification

The per-scene runtime test drives the dialogue to `SceneEnded`; the focused
trigger test verifies the chest-bank requirement and the clear `$32` guard.
