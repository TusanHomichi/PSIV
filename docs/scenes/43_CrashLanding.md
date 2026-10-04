# `Cutscene_CrashLaanding`

- **Retail bytes:** `$07658A..$07714F` inclusive, 3,014 bytes.
- **Pointer:** `CutscenePtrs[$0F]` at `$05A580`; scene event `$800F`.
- **Trigger:** `RunEventsJmpTbl[$29]` after Chaos Sorcerer `$71` is set.
- **Data:** `post_zio_cutscenes.rs`, `CRASH_LANDING` (130 ops).

## Clone audit

The spelling `CrashLaanding` is the pointer-table name in the `grand_cross=0`
branch (`ps4.asm:120779`); the body label is `Cutscene_CrashLanding` at
`ps4.asm:155745`. The actual ROM pointer pair is `$07658A..$077150`; no
Grand Cross-only body is used.

## Retail transcription

| ROM offsets | Retail primitive / literal | Scene record |
|---|---|---|
| `$07658A..$076652` | crash panel `$80`; Foi/Tandle/Spark/Legeon sequence; dialogue entry `3` | panels/SFX/dialogue |
| `$076664..$076724` | blast panel `$86`; Megid/Legeon/`$AD`; variable red fade; stop music | panels/fade |
| `$07672A..$07681A` | reload; Dezolis field panel `$87`; tree 14, entry `7`; Dezolis `$001` from Zelan Space `$18C`; camera `($240,$5D0)` | tree/dialogue/map/camera |
| `$076820..$07684A` | load Raja Temple `$14C`, previous map Dezolis `$001`, clear load bit 3 | `LoadMap`; `$85` remains clear |
| `$076850..$076AE6` | park party at `($5F0,$120)`; flight/camera; play ElevatorOpen and call `$076E00` | typed motion; five live BG writes |
| `$076AEA..$076C06` | panel `$88`; dialogue entry `8`; actor staging; set `$85`, same-map refresh keeping objects | dialogue/flag/`LoadMap` |
| `$076C0C..$0770D1` | Wren/Chaz/Rika/Rune motions and facing; final dialogue resumes | actor motion/dialogue |
| `$0770D2..$07714F` | copy Raja into party slot 5, macro `8`; set `$88`; return `1` | party/macro/flag |

The long flight, palette flash and temporary objects are retained as
typed presentation/motion records. The durable edges are the Dezolis and Raja
Temple loads, `RajaTemple=$85`, Raja in slot 5 (zero-based slot 4), and
`RajaJoined=$88`.

## Verification and boundary

The headless arc ends this beat at Raja Temple with party
`[Chaz,Rika,Rune,Wren,Raja]`. The next movement to Hangar/Dezo Spaceport is
player-controlled; the next fixed cutscene is `Landale` (`$8010`).

## Live layout write: retail correction #67

**This helper writes Raja Temple `$14C`, not Dezolis `$001`.** The old census
also treated `$40(a1)` as column +64. The supported US ROM's bytes confirm
the map load, pointer arithmetic and source rows below; the clone labels are
only navigation aids. ROM SHA256:
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.

### Map, chunk source and collision

| Rule | Retail site / source lines |
|---|---|
| Load map `$14C`, previous map `$001`, start `(6,$2E)`, clear load bit 3 | `$076820..$07684A`; `ps4.asm:155869-155876` |
| Call the live helper after ElevatorOpen | `$076AE6` → `$076E00`; `ps4.asm:156017-156018` |
| BG dimensions are 64×64 chunks (stored row/column size `$3F`, add one) | header `$1A9A10..$1A9A13`; `ps4.asm:254149`; `GetMapLayoutOffset` `$053522..$053532`, `ps4.asm:110783-110790` |
| BG address is `$FFFFB000 + 9*64 + 47`; offsets `0,1,$40,$41` give `(47,9),(48,9),(47,10),(48,10)` | `$076E00..$076E2A`; `ps4.asm:156207-156220`; resolver `$053514..$053534`, `ps4.asm:110776-110791` |
| Chunk definitions come from Raja Temple's Kosinski blob `$1ABDA8` | record pointer `$1A9A1A`; `ps4.asm:254150`; this is distinct from Dezolis' `$114914` |
| Collision reads BG, because temple scroll mode is 1 | record `$1A9A14`; `ps4.asm:254149`; `GetChunkAndCollision` `$045A52..$045AE0`, `ps4.asm:90823-90885` |
| Each chunk imposes four nibbles from bit 14 of its four 2×2 tile quadrants | `$045AE0..$045B34`; `ps4.asm:90883-90918` |
| `$85` is set only after the animation, followed by a same-map refresh with load bit 3 set | `$076BEA..$076C06`; `ps4.asm:156071-156076` |
| Later loads reapply the final four chunks while `$85` is set | `MapDataMan_RajaTemple` `$052AAE..$052AF2`; writes `$052ADC..$052AEC`; `ps4.asm:109522-109541` |

### Helper, every instruction

All sites are decoded from the retail ROM, not fork build-address comments.
`ps4.asm` line numbers refer to the local `reference/ps4disasm/ps4.asm`.

| Site | Instruction / effect | Source line |
|---|---|---:|
| `$076E00` | `move.w #$2F,d1`: column 47 | 156207 |
| `$076E04` | `move.w #9,d2`: row 9 | 156208 |
| `$076E08` | `moveq #1,d3`: select BG | 156209 |
| `$076E0A` | `jsr GetMapLayoutOffset`: resolve `a1` | 156210 |
| `$076E10` | `lea $076E58,a0`: animation table | 156211 |
| `$076E16` | `moveq #5,d0`: six iterations under DBF | 156213 |
| `$076E18` | `moveq #0,d1` | 156214 |
| `$076E1A` | `cmpi.b #$FF,(a0)`: table terminator | 156215 |
| `$076E1E` | `beq $076E56`: stop before copying the terminator | 156216 |
| `$076E20` | `move.b (a0)+,(a1)`: upper left | 156217 |
| `$076E22` | `move.b (a0)+,1(a1)`: upper right | 156218 |
| `$076E26` | `move.b (a0)+,$40(a1)`: lower left | 156219 |
| `$076E2A` | `move.b (a0)+,$41(a1)`: lower right | 156220 |
| `$076E2E` | save `d0/a0-a2` | 156221 |
| `$076E32` | `jsr RefreshPlaneBG` (`$0540FA`) | 156222 |
| `$076E38` | restore `d0/a0-a2` | 156223 |
| `$076E3C` | save `d0/a0-a2` | 156225 |
| `$076E40` | `jsr RunMapUpdates` (`$054938`) | 156226 |
| `$076E46` | restore `d0/a0-a2` | 156227 |
| `$076E4A` | `jsr DMAPlane_B_VInt` (`$0416A6`) | 156228 |
| `$076E50` | `dbf d0,$076E3C`: six updates for this frame | 156229 |
| `$076E54` | `bra $076E16`: advance table, retain the same layout address | 156230 |
| `$076E56` | `rts` | 156232 |

| Table source | Upper left/right; lower left/right | Source lines |
|---|---|---|
| `$076E58..$076E5B` | `$50,$51; $58,$59` | 156234-156235 |
| `$076E5C..$076E5F` | `$52,$53; $58,$59` | 156236-156237 |
| `$076E60..$076E63` | `$54,$55; $5A,$5B` | 156238-156239 |
| `$076E64..$076E67` | `$56,$57; $5C,$5D` | 156240-156241 |
| `$076E68..$076E6B` | `$56,$57; $5E,$5F` | 156242-156243 |
| `$076E6C` | `$FF` terminator | 156244 |

For the temple chunk definitions, `$58,$5E,$5F` impose `[8,8,8,8]`; the
other chunks `$50..$5F` impose `[0,0,0,0]`. The third frame opens the lower
left; the last closes both lower chunks. This is live collision, not only art.
The source is the `$1ABDA8` blob and the collision routine cited above;
the Python test independently decodes those nibbles from the ROM.

The accepted pack already had `$56,$57,$5E,$5F` from the load hook. The
extractor now adds the remaining 12 raw temple tiles through the existing
`scene_patch_chunks` → `patch_atlas` path, retaining pixels, priority and
collision in the normal pack schema. The manifest adds
`map_effects.scene_patch_chunks`, a per-map census also covering the existing
door scenes. `psiv-data` already loads these tiles; no schema fork is needed.
The scene uses five `WriteMapChunks` operations, each followed by six ticks,
then the retail flag and refresh. The [live-layout census](LIVE_LAYOUT_WRITES.md)
records the closed row. Correction ladder: rung 2, gate-run source/emission and
live-runtime regressions with negative controls; the existing typed op remains
the single application path.

## #67 delivery receipt

Task-local logs and generated ROM-derived assets are retained under
`build/x67-evidence/`, `build/x67-pack/` and `build/x67-route/`; they are
ignored and not committed. Retail instruction receipts are in
`build/x67-evidence/retail-source-complete.txt`. Verification here is extractor and
headless runtime state, not native input or an original-frame visual comparison.

Full local pack command (positional arguments, as `pack --help` requires):

```bash
python3 -m psiv_tools pack "Phantasy Star IV (USA).md" build/x67-pack
```

The 184 ignored battle-animation PNG inputs are read-only symlinks from the
owner's verified fixture directory; its tracked README and the accepted pack
symlink are untouched. The build exits 0 and emits all 361 real maps, skipping
the 56 retail null records. Accepted manifest SHA256:
`7fe1e64abfb4d55230a1039f5ac2deea4b45f6e94e5b029bba10107a39a016de`;
new manifest SHA256:
`83f431b872122393361b2f0204bb8f3cde74fa276f28bbc418c539d349f41f78`.

`build/x67-evidence/pack-comparison.json` records all file hashes and the
manifest diff: 4,788 files in each pack, 4,784 byte-identical, no file additions
or removals. The only changed files are `manifest.json`,
`maps/14C_RajaTemple.json`, `maps/14C_RajaTemple_patch.png`, and
`maps/14C_RajaTemple_patch_over.png`. Manifest changes are the new scene-chunk
census, temple atlas count 4 → 16, and that map's JSON hash. Every Dezolis file
is byte-identical. Existing atlas entries reindex because raw ids are sorted.

Focused extraction: `PYTHONPATH=. python3 -m unittest tests.test_crash_landing`
exits 0, five tests, zero skips. Focused runtime:
`PSIV_RUNTIME_PACK=$PWD/build/x67-pack CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --lib scene_map_tests:: -- --test-threads=1`
exits 0, six tests, zero ignored. The first focused Rust compilation failed
with `E0308` (test coordinates used `u32` instead of `Cell`'s `u16`); the
corrected run and the original failure remain separate logs.

The same crash regression against the unmodified accepted pack is an explicit
negative-control run: exit 101, zero passed/one failed, because the first live
write rejects missing raw tile `$50`. The log is
`build/x67-evidence/rust-old-pack-negative.log`. This is an expected control
failure, separate from acceptance against the new pack.

### Workspace acceptance and route

Self-reviewed frozen candidate: base `3344f183232d24b802d5dd58f0a4a4e0424bd964`
on `cx/x67-crash`, plus the exact patch/file hashes in
`build/x67-evidence/candidate.json`. A later docs-only correction of two
overview ranges and the complete instruction-capture link is recorded in
`candidate-doc-amendment.json`; implementation, tests and pack stayed frozen.
All heavy jobs ran sequentially. Rust used `CARGO_BUILD_JOBS=2` and tests used
`--test-threads=1`; `PSIV_RUNTIME_PACK=$PWD/build/x67-pack` was set throughout,
and Python used `PYTHONPATH=.`. These are the brief's acceptance commands,
not the repository gate's differently configured one-job receipt.

| Command | Result |
|---|---|
| `cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1` | exit 0; 1,288 passed, 0 failed, 3 ignored; 987.377 s |
| `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | exit 0; 56.036 s |
| `cargo fmt --manifest-path rust/Cargo.toml --all --check` | exit 0 |
| `python3 -m unittest discover -s tests` | exit 0; 1,246 passed, 0 failed, 0 skipped; 777.255 s wall time |
| `python3 tools/size_guard.py` | exit 0; 1,030 files scanned, 0 over the limit |
| `python3 tools/check_docs.py` | exit 0; 176 files, 774 links, 145 anchors, 98 command paths, 0 problems (before final receipt text) |
| `git diff --check` | exit 0 |
| `cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign` | exit 0; 64.509 s |

Rust's three ignored cases are the optional Aiedo/whole-route integration
tests and `dump_manifest_entries`, a harvesting tool. The release CLI route
below separately runs the whole tracked route. `gh issue view 67` failed with
exit 1 because the sandbox could not reach GitHub; the supplied issue brief
was used. No required local input was absent.

The supplied route command exits 0 with digest `949c2abe3342e838`, but the
campaign CLI's `pack_dir` reads **`PSIV_PACK`**, not `PSIV_RUNTIME_PACK`.
Thus that command is an accepted-pack baseline, retained separately in
`build/x67-route-default-baseline/` with its raw log in
`build/x67-evidence/campaign-route.log`. To actually verify the new pack:

```bash
PSIV_RUNTIME_PACK=$PWD/build/x67-pack ./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/x67-route --tape build/x67-route/run.tape --report build/x67-route/report.json --pack build/x67-pack
```

That explicit-pack run exits 0 in 36.191 s, completes all 29 chapters in
2,615,778 frames, and retains the same digest `949c2abe3342e838`. The tapes,
reports and all 30 save files match the accepted-pack baseline byte-for-byte.
Tape SHA256:
`892dfce15918f8d79bdad0bdf9f6699b53179ef70026f1a4c2d38cc302f4835a`.
The ordinary camp SAVE and final chapter snapshot share SHA256:
`4aeff0b18e219836bec033fb1ee15e26cc93484875fabd25adda67dd0472ec62`.
Crash Landing remains past this route's endpoint; the dedicated regression
and the synthetic scene chain cover it separately. No native/visual campaign
completion is claimed.

Raw command timestamps, exit codes, counts and logs are in
`build/x67-evidence/acceptance.json`; the corrected stdout-summary parsing and
explicit-pack route are composed in `acceptance-final.json`, with no raw
receipt overwritten. `route-comparison.json` records the route hashes and
`atlas-retention.json` the unchanged four old base/priority tile crops and
collision definitions after reindexing. Final docs/size/whitespace rechecks
are recorded separately in `final-rechecks.json`.

Next integration action: use the rebuilt pack when landing this branch; the
accepted pack still lacks the intermediate frames. No push, merge or issue
write was authorized or performed by this worker.
