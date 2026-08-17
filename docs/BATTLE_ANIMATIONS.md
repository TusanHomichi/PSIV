# Enemy battle animations

Implemented 2026-08-16. This is the durable scout and extraction contract for
enemy physical-attack presentation. It is separate from the 48-byte
`Battle_EnemyData` stats/AI projection and from the already-emitted
`battle/art/enemies.json` body records.

## Retail dispatch

The verified input is `Phantasy Star IV (USA).md`, 3,145,728 bytes, SHA-256:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

`Enemy_Attack` loads the acting enemy id from the fighter record, indexes the
self-relative word table `EnemyAttackOffs`, creates one or more battle objects,
and lets those objects run once per battle update. The retail table is:

| Table | ROM offset | Entries | Encoding | Guard/build |
|---|---:|---:|---|---|
| `EnemyAttackOffs` | `$00D0A4..$00D1D5` | 153 | big-endian self-relative words | preceding `4E BA E6 02 4E 75`; following `00 28 3B 40 3A 52` |

It resolves to 74 distinct routine targets in `$00D218..$010ECC`. Repeated
targets are intentional: several enemy ids share one attack routine.

The reference checkout at `reference/ps4disasm` is the Grand Cross hack. Its
`ps4.options.asm` sets `grand_cross = 1`, so it is used only for labels and
semantic reading. It is never byte authority. Every emitted table descriptor
in `battle/enemy_animations.json` carries `grand_cross: 0`; the extractor
refuses a ROM with the retail hash/guards changed.

The object pointer ranges transcribed from the retail ROM are:

| Retail table | ROM offset | Object IDs | Entries |
|---|---:|---:|---:|
| `BattleObjsGroup2Ptrs` | `$01252C` | `$040..$128` | 59 |
| `BattleObjsGroup3Ptrs` | `$015E2C` | `$12C..$23C` | 69 |
| `BattleObjsGroup4Ptrs` | `$01B6F6` | `$240..$31C` | 56 |
| `BattleObjsGroup5Ptrs` | `$020590` | `$320..$3FC` | 56 |
| `BattleObjsGroup8Ptrs` | `$026EC0` | `$700..$814` | 70 |
| `BattleObjsGroup9Ptrs` | `$02D88E` | `$818..$8BC` | 42 |
| `BattleObjsGroup10Ptrs` | `$033456` | `$8C0..$934` | 30 |

There are 382 object pointers. IDs advance by four because each table entry is
a longword; treating them as consecutive IDs silently points at the wrong
object. The extractor follows routine-created object references recursively,
with a retail-specific span guard for `ForcedFly`, which branches into the
shared `MonsterFly` body.

## SFX writes

The direct retail encoding is `13 FC 00 <id> 00 FF 50 0A`; the delayed
object-local form is `13 7C 00 <id> 00 10`. The extractor keeps every matching
write in `sfx_writes`, including its ROM offset and owning object ID. The
record-level `sfx_id`/`dispatch` selects the first direct `Sound_Index` write
in deterministic routine/object discovery order. That is the sound attached
to the ordered `BattleEvent::Attacked` event. Child/effect writes remain in
provenance instead of being collapsed into a fake second attack event.

Examples from the retail object graph:

| Enemy | Root object | Mapping record | Selected write | Retail SFX |
|---|---:|---:|---:|---|
| Helex (`0`) | `$048` | `$015504`: duration 6, 3 frames | `$0154BE` | `$D7` `EnemyAttack3` |
| Monster Fly (`1`) | `$040` | `$015410`: duration 4, 4 frames | `$0153EC` | `$D7` `EnemyAttack3` |
| Gunner Bit (`2`) | `$050`, `$054` | no proven fixed mapping in this graph | `$0155D2` | `$D6` `MechEnemyAlarm` |
| Zoran Bult (`10`) | `$078` | `$015236`: duration 2, 8 frames | `$0151EC` | `$D8` `EnemyAttack4` |

The existing wave-1 art extractor remains the body authority. Its
`psiv_tools.battle_art.enemy_records()` reads the 20-byte graphics record at
`$27F3AE`: VRAM reservation, three art pointers, body mapping pointer,
half-width, and height. `battle/art/enemies.json` and
`battle/art/enemy_overlays.json` already use those records. This new file does
not duplicate body pixels or invent a second body mapping; it joins the attack
object graph to the same enemy IDs.

## Fixed frame records and current presentation

When an attack object assigns a pointer to `$8(a4)`, the pointed record has the
form:

```text
+$00 byte  duration
+$01 byte  frame_count
+$02 long  mapping/object pointer, repeated frame_count times
```

`loc_256AE` consumes that record and advances the object timer. The extractor
marks a `frame_sequence` only when both the record is structurally valid and
the object span contains the retail `loc_256AE` call/jump. It records the raw
mapping pointers, duration, count, and `total_frames`; it does not claim those
pointers are already decoded enemy body pixels.

The current census is:

| Surface | Exact/proven | Deferred |
|---|---:|---:|
| Enemy records covered | 153 | 0 |
| Direct per-enemy attack SFX | 153 | 0 generic |
| Fixed timed frame sequences | 79 | 74 |
| Movement/lunge presentation | 0 | 153 |
| Attack sprite-sheet composition | 0 | 153 |

Godot now consumes `BattleAnimationEvent` alongside the ordered sound sidecar.
For the 79 proven timing records it runs a body flash for `total_frames`; this
is a bounded timing presentation, not a guessed lunge. The existing enemy body
and overlay textures continue to use the wave-1 art records and their proven
idle/dynamic replacement path. Movement and attack-sheet replacement stay
explicitly deferred until the retail object mapping pointers are decoded to
the corresponding pixels.

## Pack schema and proof commands

`battle/enemy_animations.json` has:

- `source`: retail hash, table offsets/counts, guards, and build flags;
- `animations[153]`: enemy id/symbol, routine target, root/reachable object
  IDs, exact selected SFX, every SFX write, optional frame sequence, and
  explicit movement/flash/sprite-sheet proof flags;
- `census`: exact-vs-generic SFX and timing/deferred counts.

The additive pack file is emitted by `psiv_tools.battle_pack.emit_battle()`.
The typed loader validates count, enemy-id coverage, all `grand_cross` flags,
and that each selected SFX is present as a direct `Sound_Index` write before
`psiv-runtime` builds its enemy lookup. Useful checks are:

```text
python3 -m unittest tests.test_battle_animations tests.test_battle_pack
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-runtime -p psiv-sound
```

The Godot debug selector `PSIV_DEBUG_BATTLE=0x88` uses Zoran Bult followed by
Gunner Bit. Its ordered audio proof is `$D8`, `$D6`, then miss `$B8`; the
register-log fixture repeats those two distinct retail IDs deterministically.
