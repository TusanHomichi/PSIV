# Enemy battle animations

Implemented 2026-08-16. This is the durable contract for physical enemy
attack presentation. It is additive to the 48-byte `Battle_EnemyData` stats/AI
projection and to the wave-1 battle art files.

## Retail authority

The verified input is `Phantasy Star IV (USA).md`, 3,145,728 bytes, SHA-256:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

`Enemy_Attack` indexes the self-relative word table `EnemyAttackOffs`, creates
one or more battle objects, and lets them run once per battle update:

| Table | ROM offset | Entries | Encoding | Guard/build |
|---|---:|---:|---|---|
| `EnemyAttackOffs` | `$00D0A4..$00D1D5` | 153 | big-endian self-relative words | preceding `4E BA E6 02 4E 75`; following `00 28 3B 40 3A 52` |

It resolves to 74 distinct routine targets in `$00D218..$010ECC`. The seven
retail object-dispatch tables are `$01252C`, `$015E2C`, `$01B6F6`, `$020590`,
`$026EC0`, `$02D88E`, and `$033456`, covering object groups 2, 3, 4, 5, 8, 9,
and 10 (382 pointers total, IDs spaced by four).

**Grand Cross trap:** `reference/ps4disasm` has `grand_cross = 1`. It is used
for labels and semantic reading only. Every emitted table descriptor carries
`grand_cross: 0`, and the extractor refuses a changed retail hash or guard.
The reference clone is never byte authority.

## SFX and object graph

The direct retail write is `13 FC 00 <id> 00 FF 50 0A`; the delayed
object-local form is `13 7C 00 <id> 00 10`. `sfx_writes` retains every matching
write with its ROM offset and owning object. The record-level `sfx_id` selects
the first direct `Sound_Index` write in deterministic routine/object discovery
order. All 153 enemy ids are exact; no generic sound fallback exists.

Examples:

| Enemy | Root object(s) | Timed record | SFX |
|---|---|---|---|
| Helex (`0`) | `$048` | `$015504`, 6 x 3 | `$D7` `EnemyAttack3` |
| Monster Fly (`1`) | `$040` | `$015410`, 4 x 4 | `$D7` `EnemyAttack3` |
| Gunner Bit (`2`) | `$050`, `$054` | no proven fixed clock | `$D6` `MechEnemyAlarm` |
| Zoran Bult (`10`) | `$078` | `$015236`, 2 x 8 | `$D8` `EnemyAttack4` |
| Twin Arms (`87`) | `$364`, `$36C`, `$374`, `$378` | `$023532`, 2 x 4 | `$D5` `MoleAttack` |

## Frame-record and enemy-art composition

The timed record assigned to `$8(a4)` is:

```text
+$00 byte  duration
+$01 byte  frame_count
+$02 long mapping pointer, repeated frame_count times
```

`loc_256AE` consumes the clock. Each mapping pointer is a VDP sprite record,
not a five-byte tuple:

```text
+$00 byte count_minus_one       ; DBF renders count_minus_one + 1 sprites
+$01 byte Y offset
+$02 byte size
+$03 word tile word
+$05 byte X offset, normal
+$06 byte X offset, mirrored
```

That is one header byte plus **six bytes per sprite**. The `size` byte gives
`width_tiles = (size & 3) + 1` and `height_tiles = ((size >> 2) & 3) + 1`.
The tile word uses `$07FF` for the pattern index, `$0800` for H flip, `$1000`
for V flip, and `$6000` for palette bits. The emitted frame sheets reject
palette-bit mappings and pattern rectangles beyond the enemy bank.

The wave-1 authority is `psiv_tools.battle_art.enemy_records()` at
`$27F3AE`, 20 bytes per enemy:

```text
+$00 word VRAM reservation
+$02 long Art #1     +$06 long Art #2     +$0A long Art #3
+$0E long body mapping
+$12 byte half-width       +$13 byte height
```

The body mapping addresses the decompressed art banks in the proven order
**Art #3, Art #1, Art #2**. `battle_animations.py` uses the same three-bank
authority, then records every selected mapping record and its ROM offsets in
`frame_sequence.mapping_records`. Exact sheets are emitted under
`battle/art/enemy_attacks/`; partial/deferred enemies are present in
`battle/art/enemy_attacks.json` with no guessed PNGs.

The renderer's coordinate authority is also explicit: object `$2C(a4)` is X,
`$2E(a4)` is Y, `$30/$34(a4)` are fixed-point X/Y accumulators, and
`loc_12618` establishes the fighter pivot at Y `$D8`. The Genesis screen
coordinate uses the `-128` bias. All movement write sites are retained with
ROM provenance; only the normalized tracks below are playable.

## Movement scout and census

The extractor recognizes direct `addi/subi/addq/subq.w` writes, immediate
placements, compare limits, and fixed-point commits. It does not pretend that
a branch or helper call is a complete path. Per enemy, each surface is
`exact`, `partial`, or `deferred`, with a reason in the JSON.

| Surface | Exact | Partial | Deferred | Meaning |
|---|---:|---:|---:|---|
| Enemy records | 153 | 0 | 0 | one record per `EnemyAttackOffs` entry |
| Direct SFX | 153 | 0 | 0 | exact `Sound_Index`, no generic fallback |
| Timed frame records | 79 | 0 | 74 | positive-duration `loc_256AE` records |
| Mapping/art composition | 60 | 15 | 78 | all rectangles valid / some valid / no usable record |
| Movement track | 30 | 49 | 74 | normalized / writes seen but not safe / no clock |

Movement examples decoded into the pack:

- Zoran Bult and Xanafalgue use exact relative Y offsets (`+$10`) on the
  selected object.
- Twin Arms' object `$370` starts at Y `$20`, adds `$40` per update, and stops
  at `$E0`; this is emitted as an exact linear track with pixel offsets
  `[-184, -120, -56, 8, ...]` relative to the shared `$D8` pivot.
- Helex/Monster Fly have reachable child objects with additional coordinate
  writers; their selected frame sheets are exact, but the whole graph is
  correctly marked movement `partial` until those child layers are composed.
- Igglanova's `$30/$34` fixed-point projectile path and the more complex mole,
  bit, tower, and spell-object branches remain partial. Their sites and
  helper provenance are retained rather than reduced to a fake lunge.

The resulting current counts are in the `census` object of
`battle/enemy_animations.json` and in the per-enemy records. The art index
repeats the composition census and contains 393 exact frame PNGs across 60
enemies.

## Runtime/Godot contract

`rust/psiv-runtime` continues to emit ordered `BattleAnimationEvent` sidecars
beside `BattleEvent` and `BattleSoundEvent`. The event now carries
`movement_proven` and `sprite_sheet_proven` alongside the decoded duration,
count, and total. No event is synthesized from an unordered art lookup.

`rust/psiv-godot/src/battle/attack.rs` consumes that sidecar. When the event
and art index both prove composition, it creates a child `Sprite2D`, advances
the exact frame clock, and applies the normalized position track. When the
composition is partial/deferred, it retains the existing body flash only for
the proven timing record. `ui.rs` and its ordered dispatch loop are unchanged.

## Deterministic fixtures and live proof

The Rust fixtures assert state, not screenshots:

- Zoran Bult: duration 2, eight frames, deterministic frame indices
  `0,0,1,1,2,2,3,3`.
- Twin Arms: duration 2, four frames, exact Y motion `-184,-120,-56,8,8,8,8,8`
  after terminal clamping.

The Python fixtures assert the same two enemy records, six-byte mapping shape,
art-bank validity, movement kind/offsets, and the three-way census. The live
debug selector uses Zoran Bult followed by Twin Arms, so it exercises a static
exact attack and the visible retail lunge:

```text
PSIV_DEBUG_BATTLE=0x88 \
PSIV_DEBUG_SHOT=/tmp/psiv-battle.png PSIV_DEBUG_SHOT_FRAME=108 \
xvfb-run -a /home/peter/.local/bin/cairn-godot-4.7.1 \
  --path godot --audio-driver Dummy
```

On a host whose `xvfb-run` supports it, add `--display-driver x11` after
`-a`. This laptop's wrapper rejects that option, so the equivalent `xvfb-run
-a` command above was used. It reached the debug battle, dispatched Zoran's
`$D8` and Twin Arms' `$D5`, and produced the requested viewport capture.

## Verification

```text
PYTHONPATH=. python3 -m unittest tests.test_battle_animations tests.test_battle_pack tests.test_battle_art_pack
CARGO_BUILD_JOBS=2 rustfmt --edition 2024 --check \
  rust/psiv-data/src/battle/animations.rs rust/psiv-data/src/battle/mod.rs \
  rust/psiv-data/src/lib.rs rust/psiv-runtime/src/events.rs \
  rust/psiv-runtime/src/battle_interim.rs rust/psiv-godot/src/battle/attack.rs \
  rust/psiv-godot/src/battle/enemy_overlay.rs rust/psiv-godot/src/battle/art.rs \
  rust/psiv-godot/src/battle/sfx.rs rust/psiv-godot/src/battle/mod.rs
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml --workspace
PYTHONPATH=. python3 -m psiv_tools pack "Phantasy Star IV (USA).md" runtime-pack
```

The task-owned Rust files and the repository-wide `cargo fmt --all -- --check`
both pass. The full Python suite is 909 tests green, the full Rust workspace
is green, strict workspace clippy is green, and the sequential full-pack build
completes with the attack art index in its manifest.

The full pack is generated sequentially because the laptop's previous five-wide
build was OOM-killed. Generated `runtime-pack/` art remains ignored; its new
attack index is required by the Godot art loader and is included in the pack
manifest's battle-art file census.
