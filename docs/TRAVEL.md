# Native field travel

`psiv_tools/travel.py` extracts the ROM tables into the ordinary JSON runtime
pack. `psiv-data` validates their version, ROM hash, selectors and uniqueness.
The Rust runtime now consumes those tables; Godot's TECH menu exposes HINAS
and RYUKA when the character has learned them. ITEM exposes TELEPIPE and
ESCAPIPE without a character picker. Runtime play reads JSON, with
no ROM or emulator access.

RYUKA is technique 39 and costs 8 TP; HINAS is technique 40 and costs 4 TP in
the US pack. Learned-slot order and the original field caster rules apply:
death and paralysis block casting, while technique seal does not. Browsing
or cancelling RYUKA destinations is free. Choosing a visited destination pays
once and opens the teleport message. HINAS pays when an eligible exit message
opens. Confirm, cancel, Enter or controller Start acknowledges the paid message
and performs the ordinary map transition; acknowledging it is not a refund.
All four travel paths consume no RNG draws themselves.

TELEPIPE (item 132) uses the same town list without a caster, TP, status or
learned-spell requirement. Browse/cancel and blocked use preserve every item.
Confirming an available town removes the selected inventory slot, then runs
the original first-hole shift. ESCAPIPE (item 133) uses the remembered dungeon
exit and consumes its selected slot only when that exit is valid. A pending
message cannot consume twice. These item paths play the teleport SFX on
acknowledgement, without the technique-cast sound.

The source is `Win_ItemActionMain` ($5B6CC), `Win_TelepipePlaceList`'s
confirmation ($5CA0C), and `Win_ItemTeleportMsg` ($5CAD2). `GetInventoryOffset`
uses the selected eight-item page/row. Unlike a full chest's discard swap,
ordinary item use does not search for the first matching item ID. The town
and dungeon eligibility/destination routines are shared with TECH.

## Retail sources and coordinates

`ProcessTownTeleportData` allows RYUKA only when Dungeon_Teleport_Index and
Town_Teleport_Flag are both zero, and the World_Index BYTE is 0, 1 or 2.
The previously reversed town-byte helper is corrected. World_Index is at
$F400: it is the high byte of the raw word retained by the save serializer.

RYUKA reads Motavia town bits 0..15, Dezolis bits 16..24, or Rykros bit 25.
Motavia starts at $6115A; its 16-row loop overlaps the first three Dezolis
rows at $611C2. Those aliases are retained. Rykros has one row at $6120A.
The destination's table map becomes Field_Map_Index_2, and the current world's
overworld becomes Field_Map_Index. Coordinates are eight-pixel Map_Start
units; the runtime applies its normal standing-cell Y shift.

`ProcessDungeonTeleportData` reads 34 eight-byte rows at $61212..$61321.
Index zero is unavailable. The departing map becomes Field_Map_Index_2;
the selected row supplies the new map, start and facing.

`FieldRoutine_PlaceName` reads 62 six-byte records at $664BC..$6662F,
terminated by $FFFF. It matches current and previous map. A nonnegative
selector registers a town flag and clears the dungeon exit. $FE clears the
exit without registering a town. Other negative selectors set selector & $7F.
Passageway index 7 becomes 8 at actor X $520. Runtime construction and map
entry now apply these rules; normal warps update the previous-map word and
scene LoadMap honors its explicit prev_map.

A negative dungeon byte skips the store in both original map-load routines.
The JSON null shape is retained, with its documented meaning corrected to
inherit the current exit. This preserves the Valley Maze's entrance through
its interior maps.

## Save extension and limits

The cartridge keeps Dungeon_Teleport_Index at $ED51, outside its SRAM payload.
Native saves now preserve it in unused logical header bytes $F022..$F025:
ASCII TR, the index, and its complement. Retail payload and checksum bytes
are unchanged by this extension. Loading uses it only for inherited-exit
maps and only for a recognized selector; legacy files remain readable.
A legacy save deep inside an inherited-exit dungeon cannot always reconstruct
its entrance, so its exit defaults to zero unless the place-entry table
identifies it. This extension is a deliberate native restart improvement,
not a claim that the original cartridge saves $ED51.

World selection during later space-travel scenes still needs the explicit
original writes wired into the scene runtime. Native Motavia travel and the
raw saved world-byte interpretation are covered here. The existing general
map-entry follower alignment, the exact retail teleport visuals/window layout,
and the legacy standing-Y save-format discrepancy are
separate remaining work. Do not infer full travel or campaign fidelity.

## Verification

Three Python tests compare extracted coordinates, table aliases and opposite
maze entrances against ROM bytes. Seven Rust tests cover schema rejection,
town registration, previous-map save words, free cancellation, once-only costs,
status/TP/world gating, Passageway's X-dependent exit, both Valley Maze exits,
and fresh native save reload of inherited exits. These are data and runtime
fixtures. The broader workspace check is recorded under
`build/native-tonoe/workspace-travel-tests.log`.

`build/native-travel/route/receipt.json` is the completed native fixture:
ordinary TECH input casts HINAS from Tonoe basement, cancel acknowledges its
paid message, RYUKA browsing and cancellation remain free, and choosing MILE
lands on the correct overworld cell. Chaz TP is 50 -> 46 -> 46 -> 38; other
party state and money stay unchanged. The menus and both arrival renders were
inspected. SAVE writes slot 2 while the fixture's slot 1 stays unchanged.

`build/native-travel/continued/receipt.json` then selects slot 2 through the
actual title CONTINUE in a fresh process, walks south, and saves slot 3. The
input slot is unchanged; only the leader Y changes in the logical payload.
These are isolated ability fixtures derived from the native Tonoe save;
they do not grant these spells or change any campaign save.

The full Rust workspace passes 849 tests, strict Clippy passes, and eight
Python pack tests pass. Logs are under `build/native-tonoe`; native source and
binary hashes are recorded under `build/native-travel`. Software rendering
and dummy audio do not establish physical controller, sound or frame pacing
quality.

## Pipe verification

`build/native-pipes/runtime-tests.log` passes all nine runtime travel tests:
the six technique/entry/save cases plus selected-duplicate pipe consumption,
free cancellation and rejection, inherited dungeon exits without TP, and
revalidation after the inventory changes. Strict workspace Clippy passes in
`build/native-pipes/clippy.log`. The isolated `pipe_fixture` preserves the
native Tonoe campaign roster and gives five explicitly labelled test items;
it does not modify the campaign source file.

The first native input run completed blocked TELEPIPE, ESCAPIPE, TELEPIPE
browse/cancel/reopen/confirm, and SAVE. Its captures exposed cramped town-list
spacing; the corrected list uses two tile rows and a separate cursor column.
The corrected `build/native-pipes/route/receipt.json` completed blocked use,
ESCAPIPE, free town browse/cancel, selection of the later duplicate TELEPIPE,
MILE arrival and SAVE slot 2. Only the two selected pipes were consumed;
party HP/TP/status and money stayed unchanged. The corrected captures were
inspected. `build/native-pipes/continued/receipt.json` then used the actual title
CONTINUE for slot 2, walked down once and saved slot 3 in a fresh process.
Only leader Y differs in the logical save payload ($0660 -> $0670); both input
slots and the original Tonoe campaign save remain unchanged. Source/binary
hashes and byte comparisons are recorded alongside the receipts. These runs
use software rendering and dummy audio, not a physical-controller/audio test.


## Post-Rika northern crossing (2026-09-23)

**Completed bounded gate:** ordinary CONTINUE from the connected Rika save,
paid Zema inn recovery, traversal of the newly opened northern bridge,
Motavia `$00 (84,64)`, normal SAVE, and fresh-process CONTINUE followed by
Down to `(84,65)` and SAVE 2. All five members remain alive. This stops before
Aiedo and Zio's Fort; the Redshirt/Jev experiments remain archived.

### Acceptance and retail basis

The active graph was opened in the roadmap before implementation. Acceptance
required exactly one 100-meseta inn purchase, full HP/TP/skill-use recovery,
clear persistent status, the bridge crossing, unchanged party order, inventory
and event flags, then ordinary SAVE and fresh-process persistence. No shopping,
grinding, debug party/resource/flag injection or model-driven gameplay was used.

- Zema inn rate `$068116 = 01 14`, keeper row `$0683A4`: five occupied slots
  pay `20 * 5 = 100`, leaving 1103 before encounter rewards. `RecoverStats`
  starts at `$0662DA`; `$0662FE` was an older documentation error, corrected
  in SHOPS.md and the extractor docstring. No recovery rule changed.
- US `$053D16..$053D39` tests Rika-joined flag `$35`, clears FG chunk `(42,33)`
  and writes BG `$48`. Cells `(84..85,66..67)` change from water `$9` to normal
  `$0`. ROM-decoded topology gives 35 field steps from `(99,84)` to `(84,64)`
  with `$35`, and no path without it. This topology check is separate from
  the native input pass.
- Completed Zema flags `$33/$37` are already present in the protected source.
  Retail trigger `$17` would replay the aftermath scene with `$33` set and
  `$37` clear. This condition was confirmed while correcting the regression
  fixture; the campaign save never needed alteration.

`source-audit.json`, `recovery-address-correction.json`, `route-census-02.json`,
`topology-check-02.json`, `map-load-order.json` and
`zema-fixture-correction.json` preserve the raw evidence under
`build/native-post-rika-20260923/`.

### Protected inputs, implementation and failures

Base revision remains `501fcdddb8b922f5d6001c2c4f777c955f66ab65` on `main`, plus
inherited uncommitted work. `inputs.json` and `inherited.patch` retain the
starting state. The source is
`build/native-bioplant-20260923/attempt-02/saves/slot_1.sram`, SHA256
`49dda77fae6dc79bb7a2ea8e6b249b2b3202f8e61ee9b7bb44c3626aee4d3790`.
Every native attempt starts from a separately hashed copy with explicit
`PSIV_SAVE_DIR`. `protected-inputs/` retains another exact copy. The entire
old 4783-file pack and old extension are preserved under `pre-repair/`, with
per-file hashes. Earlier healthy `$A7` saves, BioPlant evidence and inherited
adapter/example/tests/fixtures/Cargo changes are untouched.

`tools/native_post_rika.gd` adds a bounded ordinary-input route to the existing
BioPlant driver. It verifies the source before moving, accepts one paid inn
purchase, checks full recovery, walks through `(84,67)` to the north bank,
and requires `FILE SAVED`. It reuses the existing fixed battle and ANTI/RES
input behavior. Driver SHA256:
`e1ca820e9e224e6f6fe4f16e60c1f1605ae2e6fbeaa05f5dd7505443102312c2`.

Retained failures and corrections:

- `attempt-01` stopped before movement because JSON numeric arrays needed the
  same integer normalization as the live probe. `negative-control-01` also
  stopped on that type mismatch. The repaired checker rejects a deliberately
  wrong money receipt at the intended field, before movement, in controls 02
  and 03; all their save copies remain unchanged.
- `attempt-02` completed the 100-meseta rest and one victory, then stopped at
  `(84,68)` with no path onto the bridge. `$35` was present, but the runtime
  discarded the pack's raw overworld hooks. The broken bridge capture remains.
- The [page-hook repair](MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23)
  preserves raw provenance, emits separate flag-gated FG/BG composites and
  collision/raw-chunk data, and consumes them in Rust at map build. Existing
  Godot blitting replaces base and priority tiles. Old incomplete packs now
  fail with rebuild guidance. Only seven generated files changed in the full
  rebuild: manifest, two overworld JSON records and four atlas PNGs.
- Focused Rust runs 01–03 failed in a synthetic Zema-exit setup. Moving flag
  initialization before map entry was insufficient. Parent diagnostics exposed
  the missing completed-story flag `$37`; adding it to the fixture made run 04
  pass. No production code or pack changed after the first repair candidate.
- Earlier build-launch, relative-script-path parse and over-specific static
  path/crop assertions are retained. Corrected checks use absolute script
  paths, either valid bridge column, and the actual differing pixels inside
  the named tile. No failing capture or save was edited into a pass.

Primary `gpt-6-astra/max` routing was verified from live turn metadata.
Implementation was explicitly requested from `gpt-6-sol/max`; bounded driver
and source audit from `gpt-6-luna/max`. Child executor metadata was not exposed,
so those requests are not independent host-routing proof. Parent owns the
integration, source checks and native verification; the integration review is
self-review. Helper corrections are recorded in `repair-review.json`.

The final source freeze is `repair-candidate-03/receipt.json`; the runnable
freeze is `candidate-03.json` (also `candidate.json`), SHA256
`0d9d55a7cfde45d61f959a4e2141205b882843f60826da374c96616341c844be`.
The rebuilt full-pack manifest is
`018df2227406af1f09412b9ec3550724a2f9b8688aa0400c1cd707f5b4d05650`;
the extension is
`1a0dcdd62120d88182640640748ac55ee429902c1b7a01fb5ed33105ada2165d`.

### Connected result and persistence

`attempt-03/route/route.json` completes one ordinary battle and the crossing.
The inn restores HP `[76,61,53,45,39]`, TP `[22,43,28,50,25]` and all skill-use
pools. Final party order is Gryz/Alys/Chaz/Hahn/Rika, HP
`[75,60,53,45,39]`, TP `[22,43,28,50,25]`, status bytes all zero and **1129
meseta** (`1203 - 100 + 26`). Inventory and every persistent event flag match
the source, including `$34/$35`. No items were consumed or new story flags earned.

**Next campaign source:**
`build/native-post-rika-20260923/attempt-03/saves/slot_1.sram`, map `$00 (84,64)`,
SHA256 `2590e0e97ff3125774951ed2c474eba832892e3644cb79c8cdbb443d6499379e`.
Route receipt SHA256:
`b30dabe9008fd87abe0a2537cffef28052805ed824cfe6b3cc82cd212e8f8c98`.

`continue-01` copies that save into a fresh directory and launches a new Godot
process. Title CONTINUE preserves party/resources/inventory/flags/money; Down
moves to `(84,65)`, then SAVE 2 succeeds. The byte validator reports only
logical payload `$309: 00 -> 10`, plus header `$13..$17` for slot/checksums.
Input slot 1 remains unchanged. Slot 2 SHA256:
`52d3ffe7f8aae427f052bbe97da11d2dfd2b3fc605fb9367ce591d8191e4ef01`.
Validation receipt SHA256:
`af487bb9337463858e9383cd5f67c38cea63581ba043712d6b86bfdaa111da5e`.
`run-readback.json` independently checks raw SRAM checksum/location/party,
HP/TP/status/skill bytes, inventory/event bank and every per-run artifact hash.

### Current checks and reproduction

All times below are UTC on 2026-09-23. Each receipt records exact argv,
revision, environment, start/end, status and log hash. Expensive Python, Rust
and oracle runs were serialized; Rust used `CARGO_BUILD_JOBS=1`, with
`--test-threads=1` for tests. No Godot process held the extension during builds.

| Check | Receipt under evidence root | UTC start → finish | Seconds | Exit |
| --- | --- | --- | ---: | ---: |
| Full pack: 361 maps | `rebuild-pack.json` | 21:50:24.660690 → 21:54:48.868682 | 264.207978 | 0 |
| Focused Python: 31 passed | `repair-focused-python.json` | 21:54:59.396991 → 21:56:11.429630 | 72.032662 | 0 |
| Bridge runtime regression: 1 passed | `repair-focused-rust-04.json` | 22:11:04.641716 → 22:11:17.463163 | 12.821445 | 0 |
| Full Python: 952 total, 22 skipped | `full-python.json` | 21:57:41.643113 → 22:08:56.606332 | 674.963221 | 0 |
| Rust formatting | `fmt-check.json` | 22:11:26.004348 → 22:11:27.020823 | 1.016478 | 0 |
| Rust workspace: 902 passed | `full-rust.json` | 22:11:27.124477 → 22:18:40.539313 | 433.414842 | 0 |
| Strict workspace Clippy | `clippy.json` | 22:18:55.653927 → 22:19:31.544711 | 35.890802 | 0 |
| GDExtension build | `build-repaired.json` | 22:19:49.303902 → 22:20:04.763300 | 15.459394 | 0 |
| Godot driver parse | `driver-parse-04.json` | 22:20:22.107059 → 22:20:22.673528 | 0.566480 | 0 |
| negative-control-03/route | `negative-control-03/receipt.json` | 22:20:23.219129 → 22:20:34.220932 | 11.001805 | 1 |
| attempt-03/route | `attempt-03/receipt.json` | 22:20:58.195856 → 22:21:39.743826 | 41.547977 | 0 |
| continue-01/continue | `continue-01/receipt.json` | 22:22:09.835733 → 22:22:26.292306 | 16.456581 | 0 |
| continue-01/byte-validation | `continue-01/receipt.json` | 22:22:26.292672 → 22:22:26.407166 | 0.114507 | 0 |

Exit 1 for `negative-control-03` is the expected pre-movement rejection.
Full Python has **930 passed, 22 skipped**: all skips are the optional adapter
whose pinned Redshirt checkout was not on `PYTHONPATH` (`python-skips.json`).
No benchmark, live model request or native AI controller was introduced.
The full Rust result is 902 passed, zero failed/ignored.

The full gate commands are the unchanged [DEVELOPMENT.md](DEVELOPMENT.md#run-checks)
commands. Local orchestration after the source/build freeze was:

```bash
python3 build/native-post-rika-20260923/run_native.py route attempt-03
python3 build/native-post-rika-20260923/run_native.py continue continue-01 --from-route attempt-03
python3 build/native-post-rika-20260923/readback_run.py attempt-03 continue-01
python3 build/native-post-rika-20260923/compare_native_bridge.py continue-01
```

These ignored helpers have fixed evidence/source paths: inspect them and use
new output names before reuse. The tracked entrypoints are
`tools/native_post_rika.gd`, `tools/native_continue.gd` and
`tools/verify_native_continue.py`. Native runs use Godot 4.7.1, Xvfb, software
GL, Dummy audio, `--fixed-fps 8`, `--disable-vsync` and a 1280×800 viewport.
The wrapper strips inherited `PSIV_*` variables and enables only the read-only
route probe and explicit route/save configuration; no scene auto-close.

### Visual evidence and remaining limits

Parent inspected the inn's 1103-meseta receipt, the connected bridge crossing,
`FILE SAVED` and fresh-continued field captures at 1280×800. The fresh CONTINUE
capture's centered 960×672 area is normalized by integer 3× sampling to
320×224. Its bridge rectangle `(152,136)..(184,168)` is an exact **32×32** match
to original retail frame 1650: **0 differing pixels, RMSE 0.0**
(`native-oracle-bridge.json`). The world rectangle is
`(1344,1056)..(1376,1088)`; retail FG/BG cameras are both `(1192,920)`.

The original oracle loads an isolated position-adjusted SRAM copy through
normal title CONTINUE. Only position/checksum were changed; the on-case retains
the source flags. The off-case clears `$35` in a separate visual negative
control. Raw rolling layouts are FG `$00`/BG `$48` on, FG `$47`/BG `$21` off;
the video difference lies wholly inside that tile. `oracle-retail-readback.json`
retains inputs, RAM, camera and capture hashes. The fixture explicitly accounts
for the [legacy native/retail Y discrepancy](SAVE_SCOUT.md#native-inherited-dungeon-exit-extension-2026-09-13).
It is **not** original connected-campaign evidence or whole-scene parity.

Same-map flag changes followed by retail page streaming, later Dezolis chunk
rewrites, whole-scene/sprite/water-animation parity, physical controllers and
audio/frame pacing remain outside this gate. The route log still reports the
existing Motavia battle-background fallback to asset 0, Xvfb input-method/VSync
warnings and one ObjectDB exit warning; these are retained, not certified away.

At gate closure, this work was local and no commit/push/merge/deployment grant
had been given. A later explicit owner request authorized this integration.
`final-audit.json` records preserved inputs, frozen code/pack and Git state
at that closure. Owned native/build/oracle processes have exited. The next
action is recorded
only in the [roadmap](ROADMAP.md#1-continue-from-the-post-rika-checkpoint).

### Archived northern-crossing task graph

Owner assignment, 2026-09-23: return to ordinary-input campaign development;
the Redshirt/Jev experiments remain archived. The smallest selected gate is
Zema recovery followed by the Rika-opened northern crossing, stopping on
Motavia `$00 (84,64)`. Aiedo, optional Krup/Saya, Zio's Fort interior and bosses
are excluded. The retail `$35` patch at `$053D16..$053D39` changes the crossing
at `(84..85,66..67)` from water to walkable ground. ROM-decoded path inspection
finds 35 field steps from `(99,84)` to `(84,64)` with `$35`, and no route without
it. The source/topology evidence is separate from the completed input pass above.

The five-person Zema inn costs `20 * 5 = 100` meseta (`$068116`, counter
`$0683A4`), leaving 1103 before encounter rewards. Rest must refill HP, TP and
skill uses and clear status. No shopping, grinding, party/flag/resource
injection or model-driven gameplay is authorized by this graph.

```yaml
outcome: "Rest the post-Rika party and cross the newly opened northern bridge through ordinary input"
canonical_record: "docs/TRAVEL.md#archived-northern-crossing-task-graph"
base_revision: "501fcdddb8b922f5d6001c2c4f777c955f66ab65 plus preserved inherited work"
authority: "Owner 2026-09-23 campaign request: local implementation/checks/repairs; no commit, push, merge or deployment grant"
effort_policy: "Continue scoped repairs until acceptance passes; no fixed cycle limit"
common_inputs: ["docs/BIOPLANT_NATIVE.md#connected-rika-continuation-2026-09-23", "docs/SHOPS.md", "docs/TRAVEL.md", "docs/DEVELOPMENT.md", "build/native-post-rika-20260923/inputs.json"]
next_action: "Closed; the roadmap holds the one next campaign action"
nodes:
  - id: NR-01
    outcome: "Verify protected inputs and retail recovery, route and story conditions"
    depends_on: []
    owner: "gpt-6-astra / max, live turn_context verified; read-only source audit requested from gpt-6-luna / max"
    inputs: ["common_inputs", "US ROM", "runtime-pack/maps/000_Motavia.json"]
    acceptance: "Source save/ROM/pack hashes match handoff; copy isolated; retail bytes and flag-dependent path checked before implementation"
    state: verified
    evidence: ["build/native-post-rika-20260923/inputs.json", "build/native-post-rika-20260923/source-audit.json", "build/native-post-rika-20260923/route-census-02.json", "build/native-post-rika-20260923/map-load-order.json"]
    effort: inherited
  - id: NR-02
    outcome: "Prepare a frozen, reviewed ordinary-input driver and current-source extension"
    depends_on: [NR-01]
    owner: "gpt-6-astra / max integration; gpt-6-luna / max bounded driver implementation"
    inputs: ["NR-01", "tools/native_bioplant.gd", "tools/native_zema_outfit.gd", "tools/native_continue.gd"]
    acceptance: "No fixture/debug mutation; exact rest/resource/party/flag/inventory checks; Godot parse and applicable focused checks pass; candidate files and binary hashed"
    state: verified
    evidence: ["build/native-post-rika-20260923/candidate-03.json", "build/native-post-rika-20260923/driver-parse-04.json", "build/native-post-rika-20260923/negative-control-03/receipt.json"]
    effort: inherited
  - id: NR-02A
    outcome: "Apply retail overworld page patches to native collision and rendering"
    depends_on: [NR-01]
    owner: "gpt-6-sol / max implementation; gpt-6-astra / max design/review/integration"
    inputs: ["attempt-02 failed at map00 (84,68)", "US ROM loc_53D16", "psiv_tools.overworld", "existing map-effect and patch-atlas pipeline"]
    acceptance: "Data-derived flag35 opens the bridge collision and picture; flag-off negative control stays closed; load/warp/reload regression coverage and applicable Python/Rust/build gates pass before native rerun"
    state: verified
    evidence: ["build/native-post-rika-20260923/repair-candidate-03/receipt.json", "build/native-post-rika-20260923/rebuilt-pack.json", "build/native-post-rika-20260923/repair-focused-rust-04.json", "build/native-post-rika-20260923/full-python.json", "build/native-post-rika-20260923/full-rust.json", "build/native-post-rika-20260923/clippy.json", "build/native-post-rika-20260923/native-oracle-bridge.json"]
    effort: inherited
  - id: NR-03
    outcome: "Cross the northern bridge with the connected five-person party and ordinary SAVE"
    depends_on: [NR-02, NR-02A]
    owner: "gpt-6-astra / max, serialized native verification"
    inputs: ["NR-02 frozen candidate", "fresh copy of verified post-Rika source"]
    acceptance: "100-meseta full rest, observed bridge crossing, map00 at84,64, party Gryz/Alys/Chaz/Hahn/Rika alive with zero persistent statuses; original inventory and event flags retained; normal FILE SAVED receipt"
    state: verified
    evidence: ["build/native-post-rika-20260923/attempt-03/receipt.json", "build/native-post-rika-20260923/attempt-03/route/route.json"]
    effort: inherited
  - id: NR-04
    outcome: "Prove fresh-process persistence, review receipts and close the gate"
    depends_on: [NR-03]
    owner: "gpt-6-astra / max review/integration"
    inputs: ["NR-03 save and route", "tools/native_continue.gd", "tools/verify_native_continue.py"]
    acceptance: "Fresh title CONTINUE matches party/resources/inventory/flags/money; Down to84,65 and SAVE2 pass byte validator; artifacts read back, scoped captures inspected, ledger/overview updated, graph archived and owned processes stopped"
    state: verified
    evidence: ["build/native-post-rika-20260923/continue-01/receipt.json", "build/native-post-rika-20260923/continue-01/flow/validation.json", "build/native-post-rika-20260923/run-readback.json", "build/native-post-rika-20260923/native-oracle-bridge.json", "build/native-post-rika-20260923/final-audit.json"]
    effort: inherited
```
