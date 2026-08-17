# Enemy battle animations

Implemented 2026-08-17. This is the byte-level contract for enemy attack
presentation. It is additive to the 48-byte `Battle_EnemyData` projection and
to the existing enemy body/overlay art files.

## Retail authority

The only byte authority is `Phantasy Star IV (USA).md`, 3,145,728 bytes,
SHA-256:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

`Enemy_Attack` indexes the self-relative word table `EnemyAttackOffs` at
`$00D0A4..$00D1D5`. It contains 153 entries and resolves to 74 distinct
routines in `$00D218..$010ECC`. The extractor checks the surrounding retail
guards (`4E BA E6 02 4E 75` before the table and `00 28 3B 40 3A 52` after it)
and refuses another ROM hash.

The seven retail object-dispatch tables are:

| Group | ROM offset | ID range | Pointers |
|---|---:|---:|---:|
| 2 | `$01252C` | `$040..$128` | 59 |
| 3 | `$015E2C` | `$12C..$23C` | 69 |
| 4 | `$01B6F6` | `$240..$31C` | 56 |
| 5 | `$020590` | `$320..$3FC` | 56 |
| 8 | `$026EC0` | `$700..$814` | 70 |
| 9 | `$02D88E` | `$818..$8BC` | 42 |
| 10 | `$033456` | `$8C0..$934` | 30 |

The ranges are longword tables with IDs spaced by four: 382 object pointers
in total. Every table record carries `grand_cross: 0`. `reference/ps4disasm`
is a Grand Cross build (`grand_cross: 1`) and is used only for labels and
semantic cross-checks. It is never used to read retail bytes.

## What the decoder now proves

`psiv_tools.battle_animations` follows each enemy routine into its direct
object references and recursively reachable objects. Each `object_records`
entry retains the object ID, ROM span, helper calls, immediate `$08/$28`
pointer assignments, frame candidates, PLC loads, classification, and reason.

### Timed mapping records

Three retail timer contracts are decoded:

```text
loc_256AE fixed:    duration, count, mapping_pointer[count]
loc_256F4 variable: count, duration[count], pad if count is even,
                    mapping_pointer[count]
loc_25270 selector: count, duration[count], selector[count]
```

Selector bytes index a longword table assigned to `$28(a4)`. A negative
longword is a real retail hide record, not missing data; it becomes a JSON
mapping record with `hidden: true` and an emitted transparent frame. The
variable and selector records retain `frame_durations` and use their sum for
`total_frames`. The old fixed-only scout did not decode either of these
contracts.

Each selected mapping pointer is decoded as one count-minus-one byte followed
by six bytes per VDP sprite:

```text
Y, size, tile word, X, mirrored-X
```

The tile word uses `$07FF` for the pattern index, `$0800` for H flip, `$1000`
for V flip, `$8000` for priority, and `$6000` for palette selection. The
palette bits remain explicitly non-renderable for the line-relative PNG
surface.

### Enemy art and external attack art

The enemy art table is `$27F3AE`, 20 bytes per enemy:

```text
+$00 word VRAM reservation
+$02 long Art #1       +$06 long Art #2       +$0A long Art #3
+$0E long body mapping
+$12 byte half-width   +$13 byte height
```

The proven upload order is **Art #3, Art #1, Art #2**. Every exact selected
mapping rectangle is checked against that concatenated bank. When the pattern
range is outside the enemy bank, the extractor follows proven `LoadPLC1` art:

- loader target `$00B790` (`LoadPLC1`);
- PLC pointer table `$27E88A`;
- signed table-relative record pointer;
- Nemesis stream tile counts and compressed stream provenance;
- `$10(a4)` plus the statically decoded adjustment as the VRAM pattern base.

Those ranges are emitted as `tile_sources` with `kind: "attack_plc"`, and the
art index repeats their PLC call, record, stream, pattern start, and count.
There are 339 selected mapping source spans backed by attack PLC art; none is
silently treated as an enemy-bank hole. Across the per-enemy graph evidence,
383 `LoadPLC1` loads are retained and 122 distinct PLC table records decode
exactly. The duplicated load count is intentional because shared object
graphs are recorded for each enemy record.

Exact PNGs are emitted under `battle/art/enemy_attacks/`. Partial and deferred
entries remain in `battle/art/enemy_attacks.json`, but have no guessed PNGs.

### Movement fields

The movement scout records every recognized write to:

```text
$2C(a4) X       $2E(a4) Y
$30(a4) fixed X $34(a4) fixed Y
```

It decodes immediate/add-quick deltas, absolute writes, compare limits,
fixed-point accumulator updates, loads, adds, and commits. Reachable
projectile/effect objects retain separate `movement.tracks`; they are not
folded into the selected enemy mapping object's position.

The normalized runtime forms are `static_offset`, `linear`, `branch_table`,
and `fixed_point_branch`. The latter two preserve every decoded write in
`runtime.branches`; the census calls them exact when all observed coordinate
writes are structurally decoded, even when the retail routine contains several
control-flow branches rather than one linear lunge. The 32 frame-deferred
records have no proven mapping clock to which a movement track can attach, so
their movement remains deferred for that structural reason.

## Census delta

The left side is the honest post-wave-6 baseline from the pack census. The
right side is the current extractor and emitted art index.

| Surface | Before exact | Before partial | Before deferred | After exact | After partial | After deferred |
|---|---:|---:|---:|---:|---:|---:|
| Timed frame records | 79 | 0 | 74 | 121 | 0 | 32 |
| Mapping/art composition | 60 | 15 | 78 | 120 | 1 | 32 |
| Movement track | 30 | 49 | 74 | 121 | 0 | 32 |
| Exact attack PNG frames | 393 frames / 60 enemies | — | — | 956 frames / 120 enemies | — | — |

Direct SFX remains 153 exact, with no generic fallback. The current routine
classification census is:

| Class | Enemy records | Frame result |
|---|---:|---|
| `fixed_mapping` | 62 | 62 exact |
| `variable_mapping` | 49 | 49 exact |
| `selector_mapping` | 10 | 10 exact, including 19 hidden ticks |
| `tile_upload_only` | 21 | deferred mapping; PLC records exact |
| `state_machine_without_sprite_mapping` | 9 | deferred mapping |
| `projectile_effect_graph` | 1 | deferred mapping; `loc_25978` stream state |
| `palette_or_plane_effect` | 1 | deferred mapping; `loc_258AC` state |

These are per-enemy counts; shared routines account for the 16 distinct
deferred routine bodies. The 32 deferred enemy records are grouped below.

| Structural class | Enemy records | Why no frame PNG exists |
|---|---|---|
| Projectile/effect graph | ProtectBit (4) | `loc_25978` constructs tile/DMA stream state and no mapping timer consumes it; `loc_258AC` is also present. |
| Palette/plane effect | Seeker (7) | `loc_258AC` consumes effect state but selects no enemy VDP mapping sequence. |
| No sprite mapping state machine | Locusta, Fanbite, Grasshound (14–16); SandNewt, Mistralgec, FlameNewt (56–58); ToadStool, Shrieker (78–79); Prophallus (151) | The JSON says whether there is no frame-helper call, or a helper call whose immediate `$08/$28` assignments fail the fixed/variable/selector structure. |
| PLC/effect upload only | BalDuel, DragerDuel, JurafaDuel (27–29); HewGilla, Elmelew (91–92); Centaur, KingSaber, DarkRider (96–98); TechUser, TechMaster, DarkWitch (99–101); Juza, Greneris, Radhin (114–116); GyLaguiah, LwAddmer, CulaBellr (117–119); SaLews (121); ReFaze (127); Lashiec (128); DarkForce3 (132) | `LoadPLC1` records and Nemesis art are proven, but no timed enemy mapping record is constructed. The PLC evidence is emitted; a PNG is not invented from art without a mapping consumer. |

The precise per-record `frame_sequence_why_not` is the authority for these
32 records. It names the helper, assignment shape, or effect-object boundary;
it does not use a generic “unknown animation” bucket.

## Remaining genuine limits

Only one composition is partial: DarkForce1 (enemy 130). Its selected mapping
records resolve to the enemy bank, but their tile words set palette bits
`$6000`. The current indexed PNG surface is line-relative and cannot claim
which CRAM line those sprite entries select. The mapping records and raw tile
words are retained; no pixels are guessed.

The other 32 composition/frame remainders are not missing enemy art. They are
effect-object routines with no statically proven timed mapping consumer. The
deferred PLC groups now expose their exact table records and Nemesis streams,
but a full retail presentation for them still requires decoding the separate
effect object's update/DMA contract and, where applicable, its control-flow
selection of mapping data. Treating a PLC tile stream as an enemy frame loop
would be false provenance.

## Runtime contract and fixtures

`BattleAnimationEvent` now carries the full duration vector. Rust playback uses
the cumulative vector for variable and selector timers, while fixed records
use a repeated vector. Exact composition uses a child `Sprite2D`; deferred
composition keeps the existing body-flash fallback only when timing itself is
proven. Hidden selector frames are transparent attack frames, not flashes.

The unittest fixtures cover newly exact records from distinct decoder paths:

- Gunner Bit (2): variable `loc_256F4` timing;
- Worker Pod (24): variable timing plus an external `LoadPLC1` art span;
- Tower (39): selector `loc_25270` timing with hidden frames;
- King Rappy (149): variable timing with a separate enemy routine body.

The live selector `PSIV_DEBUG_BATTLE=0x89` uses real formation `$0F7` from the
pack (Slave + Worker Pod) and injects a deterministic attack-only timeline for
the newly exact member. It does not mutate runtime battle state or alter scene,
vehicle, save, or dialogue code:

```text
PSIV_DEBUG_BATTLE=0x89 \
PSIV_DEBUG_SHOT=/tmp/psiv-newly-exact-battle.png \
PSIV_DEBUG_SHOT_FRAME=108 \
xvfb-run -a /home/peter/.local/bin/cairn-godot-4.7.1 \
  --path godot --audio-driver Dummy
```

## Verification

Run one cargo process at a time; this laptop has OOM-killed a five-wide
workspace build.

```text
PYTHONPATH=. python3 -m unittest tests.test_battle_animations tests.test_battle_pack tests.test_battle_art_pack
PYTHONPATH=. python3 -m unittest discover -s tests
CARGO_BUILD_JOBS=2 cargo fmt --manifest-path rust/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml --workspace
PYTHONPATH=. python3 -m psiv_tools pack "Phantasy Star IV (USA).md" runtime-pack
```

The full-pack command is the final comparison: it must regenerate the battle
animation JSON, attack-art index, 956 exact PNG frames, and the manifest in the
same deterministic pack layout.
