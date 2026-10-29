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

Exact PNGs are emitted under `battle/art/enemy_attacks/`, rendered from the
ROM for 121 records. The 16 receipt-backed routine bodies (32 records) ship
no attack frames and are `deferred` in the art index (#214): their oracle
captures are shared across bodies, so they are not each routine's art (the
retail basis is in [source notes](../source-notes/README.md#other-recorded-deviations)).
Godot keeps the body-flash fallback for them;
`python3 tools/verify_native_flash_attack.py` drives
that fallback in a native battle and fails on an orphan or added node (#92).

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
control-flow branches rather than one linear lunge. The formerly deferred
records use full-screen observed SAT frames for their effect motion and retain
their separate effect tracks; their local attack layer stays at the
receipt-backed formation anchor, so the movement census no longer hides a
presentation clock behind static-analysis silence.

## Census

`battle/enemy_animations.json` records timed frame records, composition and
movement track as exact for all 153 records, the 32 receipt records through
their observed clocks. The art index `battle/art/enemy_attacks.json` emits
960 attack frames for 121 exact records; the 32 receipt records are deferred
there with no frames (#214).

Direct SFX is 153 exact, with no generic fallback. The routine classification
census is:

| Class | Enemy records | Frame result |
|---|---:|---|
| `fixed_mapping` | 62 | 62 exact |
| `variable_mapping` | 49 | 49 exact |
| `selector_mapping` | 10 | 10 exact, including 19 hidden ticks |
| `tile_upload_only` | 21 | observed clock only; art deferred |
| `state_machine_without_sprite_mapping` | 9 | observed clock only; art deferred |
| `projectile_effect_graph` | 1 | ProtectBit: observed clock only; art deferred |
| `palette_or_plane_effect` | 1 | Seeker: observed clock only; art deferred |

These are per-enemy counts. Shared routines account for the 16 receipt-backed
routine bodies. The receipt fixture covers one representative per body and
applies the result to every enemy record sharing that retail routine.

| Routine | Representative | Records | Retail presentation | Oracle anchor |
|---|---|---:|---|---:|
| `$010E38` | ProtectBit (4) | 1 | projectile/effect sprite table | f29848 |
| `$010E7A` | Seeker (7) | 1 | CRAM/plane effect plus sprite table | f29962 |
| `$010C7C` | Locusta (14) | 3 | state-machine sprite table | f29950 |
| `$010AB4` | BalDuel (27) | 3 | PLC upload plus sprite table | f29738 |
| `$00FCCA` | SandNewt (56) | 3 | state-machine sprite table | f29961 |
| `$00F53C` | ToadStool (78) | 2 | state-machine sprite table | f29727 |
| `$00F088` | HewGilla (91) | 2 | PLC upload plus sprite table | f30033 |
| `$00EFB2` | Centaur (96) | 3 | PLC upload plus sprite table | f29910 |
| `$00ECDE` | TechUser (99) | 3 | streamed multi-sprite effect | f29756 |
| `$00E394` | Juza (114) | 3 | PLC upload plus sprite table | f29738 |
| `$00E2BE` | GyLaguiah (117) | 3 | PLC upload plus sprite table | f29725 |
| `$00E0EC` | SaLews (121) | 1 | streamed multi-sprite effect | f29804 |
| `$00DE22` | ReFaze (127) | 1 | static base with upload/hide transitions | f30115 |
| `$00DCD2` | Lashiec (128) | 1 | PLC upload plus sprite table | f30026 |
| `$00D998` | DarkForce3 (132) | 1 | PLC upload plus sprite table | f29946 |
| `$00D526` | Prophallus (151) | 1 | state-machine sprite table | f29926 |

The durable receipt is `oracle/fixtures/battle_animation_remainder.json`; the
ignored indexed PNGs rendered from its captures,
`oracle/fixtures/battle_animation_art/`, are oracle evidence only and no pack
input (#214). Each dense capture records a
sprite-table and VDP-VRAM SHA-256 over the entire sampled window; transition
offsets are the first frame whose raw sprite-table bytes differ, and the
durations are the resulting observed dwells. The fixture uses the one-enemy
`Enemy_Count` RAM patch at f24820, patches both the fighter and stats enemy id,
keeps HP at `$7FFF`, and records the unchanged first-slot `Enemy_Positions`
byte `$0E`; it does not patch ROM or runtime state. Each receipt records the
sampled `sprite_table`/`vdp_vram` frame offsets, observed clock durations, and
the formation-relative `origin_pixels` needed by the Godot local attack layer,
`psiv_tools.battle_animation_oracle` verifies those hashes and follows the
hardware SAT link chain when regenerating the evidence PNGs. Compared by pixel
index, eight bodies' captures are one identical sequence and three more share
a second; `tests/test_battle_art_pack.py` checks that when the PNGs are
present. The captured sprite table is therefore the shared hit presentation
(most likely the targeted party member), not each routine's attack art.

## Remaining genuine limits

DarkForce1 (enemy 130) is now exact. The valid formation patch is
`24794:FFFFECFC:09` (one byte; the two-byte `0009` form selects the wrong
endian value). Its mapping words carry `$6000 & 0x206A = 0x2000`, so palette
selector 1 selects CRAM line 1. The oracle formation receipt records the exact
line-1 words:

```text
0000 0EEE 0000 0C84 0848 0626 0424 0402
0EA8 0C86 0864 0642 0220 0000 0600 0CC4
```

The 16 receipt-backed bodies remain a genuine limit for art: their timing is
the observed clock, but their frames wait on rendering each routine's own
effect objects from the ROM. ProtectBit's projectile/effect graph and Seeker's
palette/plane effect keep their distinct presentation labels in the JSON;
they are not collapsed into ordinary enemy mapping classifications.

## Runtime contract and fixtures

`BattleAnimationEvent` carries the full duration vector. Rust playback uses the
cumulative vector for variable, selector, and oracle-observed clocks, while
fixed records use a repeated vector. Exact composition uses a child `Sprite2D`
with frames rendered from the ROM; a record whose art is deferred keeps the
body flash. Hidden selector frames remain transparent attack frames, not
flashes.

The unittest fixtures cover newly exact records from distinct decoder paths:

- Gunner Bit (2): variable `loc_256F4` timing;
- Worker Pod (24): variable timing plus an external `LoadPLC1` art span;
- Tower (39): selector `loc_25270` timing with hidden frames;
- King Rappy (149): variable timing with a separate enemy routine body.

The structural remainder fixture covers all 16 shared bodies, including the
DarkForce1 formation/CRAM receipt. Its command and patch discipline are in
`oracle/fixtures/battle_animation_remainder.json`; the evidence PNGs rendered
from its captures are under `oracle/fixtures/battle_animation_art/` and the
pack never reads them.

The live selector `PSIV_DEBUG_BATTLE=0x89` uses real formation `$0F7` from the
pack (Slave + Worker Pod) and injects a deterministic attack-only timeline for
the newly exact member. It does not mutate runtime battle state or alter scene,
vehicle, save, or dialogue code (`tools/godot.py` resolves the Godot
executable, as the [development guide](../DEVELOPMENT.md#launch-the-native-game)
says):

```text
PSIV_DEBUG_BATTLE=0x89 \
PSIV_DEBUG_SHOT=/tmp/psiv-newly-exact-battle.png \
PSIV_DEBUG_SHOT_FRAME=108 \
xvfb-run -a python3 tools/godot.py run -- \
  --audio-driver Dummy
```

## Verification

Run cargo through the [heavy-run lock](../DEVELOPMENT.md#heavy-run-lock), which
sizes each job's memory class; this laptop once OOM-killed a five-wide workspace
build run outside it.

```text
PYTHONPATH=. python3 -m unittest tests.test_battle_animations tests.test_battle_pack tests.test_battle_art_pack
PYTHONPATH=. python3 -m unittest discover -s tests
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace
PYTHONPATH=. python3 -m psiv_tools pack "Phantasy Star IV (USA).md" runtime-pack
```

The full-pack command is the final comparison: it must regenerate the battle
animation JSON, attack-art index, 960 exact PNG frames, and the manifest in the
same deterministic pack layout.
