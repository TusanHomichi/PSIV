# Event engine: scouting notes for the design session

Scouted 2026-08-15 (Fable, solo pass while lanes ran). This is the fact base
for the Peter+Fable design session on the event engine — the last big
field-mode system. Nothing here is a decision.

## The shape of the system

Three layers, cleanly separated in the original:

1. **Triggers** (`RunEvents` -> `RunEventsJmpTbl`, 128 retail entries, run
   every rest-frame from the per-map event-id lists we already extract).
   Each is a tiny formulaic check: event-flag tests plus a position test
   (exact cell, coordinate range, or rect), setting `Event_Index` and
   flipping the game mode. Mostly *data wearing code's clothes* — a
   condition table with a few genuinely custom entries.
2. **Scenes** (~162 `Event_*` routines dispatched from `Event_Index`).
   Hand-written 68k, but composed almost entirely from a small primitive
   vocabulary: `Event_UpdateObjFacing`, `Event_GetAndRunDialogue`,
   `Event_MoveCamera`, `Event_AddMacro` (party join), move-object-and-wait
   loops, `Current_Party_Slots` writes, event-flag sets, NPC despawns,
   `MapDataManager` interplay. The Grand Cross DSL exists precisely because
   these patterns are regular.
3. **Flags** (`EventFlags_Set`/`Test`, ~174-flag space; `TempEveFlags` for
   transients — these live at $F140, which the disassembly mislabels
   `Chest_Flags`; real chest flags are $F120 bits shared with the extended
   event flags. See `psiv-core`'s `state.rs` module docs). Plus `Current_Party_Slots` and the character-slot copying
   that party changes do.

Worked example (`Event_AlysFound`): walk NPC-Alys to align with Chaz, face
right, run dialogue, write `Current_Party_Slots = (Alys<<8)|Chaz` — **Alys
leads the opening party** (Peter's memory confirmed) — copy Chaz to slot 2,
delete NPC-Alys, hand slot 1 to field-Alys, `Event_AddMacro`, move camera.

## The trap: the reference clone is contaminated here

17 scene routines include `script/scenes/*/event.asm` **ungated** — Rune
(Grand Cross's generator) has replaced retail scene code in this fork
unconditionally. For those scenes the disassembly is *not* a map of retail;
the cartridge bytes are the only source, and the fork tells us at most where
the scene begins. Any transcription lane must disassemble retail bytes for
scene content and treat the fork's scene files as adversarial. (Dialogue tree
17's wholesale rewrite was the same phenomenon on the data side.)

## Questions for the design session

- **Interpreter vs transcription.** Triggers want to be data (a condition
  table). Scenes: implement the primitive vocabulary once in psiv-core, then
  express each scene as a sequence over it — authored from retail
  disassembly, testable headless, oracle-verified later. Effectively a scene
  script per event, ours, checked against cartridge behavior. Alternative
  (pure per-scene Rust transcription) is the same thing with a worse diff
  story.
- **Where scenes live**: psiv-core module (deterministic, effects out) with
  the renderer consuming actor-motion effects — cinema mode hooks here.
- **Verification**: this is the system that makes the BizHawk oracle
  near-mandatory (scene actor paths, camera moves, timing). Suggest building
  the oracle harness *first or alongside*, not after.
- **Scope ladder**: v1 = flags + triggers + the ~6 opening-act scenes
  (through Alys joining and the basement), which makes the game *playable as
  a story* to the first dungeon. Full 162-scene coverage is a campaign, not
  a slice.
- **Party/event interlock**: `Current_Party_Slots` + `Event_AddMacro` are
  the party system; the follower work core-lane is doing now is its render
  half. Event engine owns the writes.
