# Battle captures and recipes

[Oracle setup and index](README.md)

### RNG trace: the cartridge's own battle rolls

PSIV's battle rolls are not pseudo-random in the usual sense. `UpdateRNGSeed2`
(`ps4.asm:86097`, ROM `$04239E`) is four instructions:

```
	move.w	$8(a5), d0		; the VDP HV counter at $C00008
	add.w	(Main_Frame_Count).w, d0
	sub.w	(RNG_Seed).w, d0	; d0 = the roll the caller receives
	ror	(RNG_Seed).w		; and the seed's high word rotates
```

so every roll is a function of *where the beam was* when the 68000 read the
counter. That is hardware timing `psiv-core` deliberately does not reproduce
(see `docs/RUNTIME_DESIGN.md`, "RNG design"); the port takes rolls through its
`Rolls` trait instead, and this flag captures the stream that trait replays.

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --groups core,battle,bhit,enemy,chars,rng,vehicle \
    --rng-trace oracle/logs/tape07_rolls.csv \
    --out  oracle/logs/tape07_battle.csv
python3 -m oracle.rng_trace check oracle/logs/tape07_rolls.csv \
    oracle/logs/tape07_battle.csv
python3 -m oracle.fixture --trace oracle/logs/tape07_rolls.csv \
    --log oracle/logs/tape07_battle.csv \
    --out rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json
cargo test --manifest-path rust/Cargo.toml -p psiv-core \
    -- tape07
```

The last two steps are the replay: `python3 -m oracle.fixture` (the CLI of the
`oracle/fixture/` package) writes the battle's start state, its rolls with the
frame and role of each, and what the RAM log shows every action doing, and
`psiv-core` replays it - `rust/psiv-core/src/battle/replay/`, whose
`data.rs` is the one test that replays **every** fixture in
`rust/psiv-core/src/battle/replay_fixtures/` and holds each one that does not
match to its entry in `divergences.json`; the verdicts are in
[`BATTLE_ORACLE_REPLAY.md`](../docs/oracle/BATTLE_ORACLE_REPLAY.md) and
[`BATTLE_ORACLE_FORCED.md`](../docs/oracle/BATTLE_ORACLE_FORCED.md).

The host writes one row per call, in frame order:

| column | meaning |
|---|---|
| `frame` | emulated frame the call happened in (1-based, `retro_run()` count) |
| `call_index_in_frame` | 0-based index of this call within its frame |
| `pc` | 68000 program counter the core reported for the HV read |
| `hv` | the HV word the counter returned for that read |
| `frame_count` | `Main_Frame_Count` (`$FFFFEF1C`) the instructions saw |
| `seed_before` | `RNG_Seed` (`$FFFFEF0C`) longword the instructions saw |
| `roll` | `(hv + frame_count - seed_high) & $FFFF`, the value left in `d0`: the subtrahend is the word at `$FFFFEF0C`, the longword's high half |
| `seed_after` | the seed after `ror (RNG_Seed).w`, low word carried |

**The core patch.** `oracle/patches/0001-rng-hv-trace.patch` is ours; upstream
Genesis Plus GX has no such hook. It makes `vdp_hvc_r()` record `(pc, hv)` for
every HV counter read and exports
`psiv_hv_trace_enable/reset/count/dropped/get`, which the host resolves with
`dlsym` and drains once per frame. With tracing off the record is one
predictable branch and the value is returned unchanged, so emulation is
identical - `verify.sh` runs against the patched core and still passes.
`build_core.sh` applies every `oracle/patches/*.patch` after the pinned
checkout, in file-name order; re-applying is a no-op, and a tree that is not
the pinned revision fails loudly instead of building something unreproducible.
See [`patches/README.md`](patches/README.md) for what those patches must obey.
A checkout shipped without `gpgx-src/.git` (how a built one travels) is used as
it stands, so the build works offline: the patches are then what pins the files
this project depends on, and any other revision fails the build rather than
producing numbers nothing was measured against.

**Which reads are calls.** The trace keeps the reads the PC says came from
`move.w $8(a5),d0` at `$04239E`. Genesis Plus GX reports the counter with the
instruction's extension word already fetched, so the access arrives as
`$0423A2` - the address of the `add.w` that follows it. The accepted window is
`$04239E-$0423A6`, from the read itself to the start of the `sub.w (RNG_Seed).w`
two instructions later, so a core that reports the PC at another point inside
or just after the access still matches; neither of the other two instructions
reads the counter, so the window cannot admit a record that is not one of
these. Every matched row carries the PC it matched on, which keeps the choice
visible in the data. Tape 07 has no other HV reader at all - all 136 records
match - so nothing there rests on the window's width.

**Why those are the seeds.** `RNG_Seed` and `Main_Frame_Count` are not in the
core's records: the host reads them from work RAM around each frame and chains
the frame's calls, because `UpdateRNGSeed2` only rotates the high word, so the
next call starts from the word the previous one left. Two things can move the
seed between calls, and both are accounted for:

- The VBlank handler (`ps4.asm:612-625`) applies `UpdateRNGSeed` (the 41x
  multiply) and bumps `Main_Frame_Count` in one block, so a frame whose counter
  steps by exactly one is a frame whose rolls chain from the multiplied seed.
  That is the whole rule, and it is what the log's counter column says.
- Genesis Plus GX triggers VINT at the top of the emulated frame
  (`core/system.c`, `system_frame_gen`: the VCount is set to
  `bitmap.viewport.h` and the interrupt is taken before the vblank and visible
  lines run), so that block precedes every read of the frame rather than
  following the frame's last call. Tape 07's rolls sit in the visible lines
  (V counter `$14-$55`), where the game's attack code runs.

**What `python3 -m oracle.rng_trace check` proves.** It re-derives each row's
`seed_after` and insists that a frame's calls chain into each other, that its
first call starts from the log's seed for the frame before it or from that seed
after one `UpdateRNGSeed`, that its last `seed_after` is the log's `rng_seed`
for the frame, and that every row's `frame_count` is that frame's logged
`Main_Frame_Count`; the anchor and the counter step must agree. It prints the
first mismatch and exits non-zero, and the host refuses to call the run
trustworthy for the same reason before that. What stays unproven is the
`frame_count` column's *timing* - it is the frame's value sampled after the
frame, justified by the frame order above rather than by the chain - and any
frame the log does not cover, which is counted and reported as skipped. Playing
these rolls back through `psiv-core`'s damage path against the same battle in
the log is the end-to-end check that closes that gap, and
[`BATTLE_ORACLE_REPLAY.md`](../docs/oracle/BATTLE_ORACLE_REPLAY.md) is that check for
tapes 07 and 09.

**The `roll` column, and the word it subtracts.** `sub.w (RNG_Seed).w, d0` at
`$0423A6` reads the word at `$FFFFEF0C`, which on a big-endian 68000 is the
**high** half of the `RNG_Seed` longword (`ps4.constants.asm:2328`) - the same
word `ror (RNG_Seed).w` at `$0423AA` rotates. The host's `rng_trace_roll`
(`oracle/host/rng_trace.h`) subtracts that word, and `oracle/rng_trace.py`'s
`roll_for` re-derives the same arithmetic, as `oracle.fixture`'s `--trace`
reading insists row by row: a capture whose column subtracts the low half at
`$FFFFEF0E` is rejected with the frame and call of the first row that does,
rather than replayed.

That was not always so, and the capture was wrong for a while. O1 built the
host with `seed_lo`, O2 found the column was a per-frame-constant shift of the
cartridge's rolls, and `python3 -m oracle.rng_trace check` could not see it
because its own `roll_for` repeated the same subtraction - the two agreed with
each other and with nothing else. The cartridge settled it against tape 07's
RAM log: the high-half derivation reproduces the battle's nine turn-order
addends and all six of its damage values, the low-half one reproduces none of
them
([`BATTLE_ORACLE_REPLAY.md`](../docs/oracle/BATTLE_ORACLE_REPLAY.md)). The fix landed
in the host and in the checker, the capture was regenerated, and the two can no
longer drift apart unnoticed: `tests/test_oracle_rng_trace.py` compiles
`oracle/host/rng_trace.h` into a probe and compares it with the checker's
`roll_for` against numbers written out from the disassembly, and
`oracle.fixture` refuses a trace whose column is not the cartridge's.

Captured with the low-half subtraction by an otherwise identical host, the same
tape and core differ in the `roll` column alone, and in all 136 rows: `hv`,
`pc`, `frame_count`, `seed_before` and `seed_after` come out the same in both,
each roll's shift is exactly `seed_lo - seed_hi`, and the seed chain still
closes on the RAM log exactly.

The capture on tape 07: 136 rolls in 15 frames, between frames 24807 and 30306
of the battle at 24794-30428, every one of them in the visible lines. The
counts line up with the disassembly: single rolls are `Battle_CalculateChances`
(`ps4.asm:17339`, one call), runs of 16 are `Battle_CalculateDamage`'s loop
(`ps4.asm:17381`, `moveq #$F,d7` with `dbf d7,-`), and the frame where two
attackers each take a damage roll carries two of those runs (32 calls). Frame
29711, first two of its sixteen rows:

```
29711,0,0423A2,2292,28815,21E817F3,7139,10F417F3
29711,1,0423A2,23F3,28815,10F417F3,838E,087A17F3
```

(The same two rows from a host built with the pre-fix subtraction read `7B2E`
and `7C8F`, `seed_after` included - the low half's subtraction of the same raw
columns, `$2292 + 28815 - $17F3` and `$23F3 + 28815 - $17F3`. The columns differ
in `roll` alone: `7139` = `$2292 + 28815 - $21E8` and `838E` = `$23F3 + 28815 -
$10F4`.)

Tape 09's capture is the same three steps on its own tape and window: 137 rolls
in 16 frames, between frames 25015 and 31786 of the battle at 25002-31908, all
of them in the visible lines, with its fixture extracted by `--tape`,
`--battle-first` and `--battle-last` and nothing else. Both traces' pins, both
fixtures' provenance and both replays' draw accounting are in
[`BATTLE_ORACLE_REPLAY.md`](../docs/oracle/BATTLE_ORACLE_REPLAY.md).

### Forced battles: capturing any formation on demand

A tape can only meet the formations its own RNG path draws, which is a problem
when the ability under test belongs to an enemy the tapes never reach.
`python3 -m oracle.force` forces one:

```sh
python3 -m oracle.force --formation 0x5E --out build/forced/helex \
    --require-ability 2
python3 -m oracle.force --formation 0x53 --vehicle 2 \
    --out build/forced/icedigger
python3 -m oracle.force --event 8 --out build/forced/event8 --durable \
    --max-rounds 12 --require-ability 0x4B
```

It cuts a base tape at the frame its encounter fires, forces the group with one
patch and the entry within the group by patching `RNG_Seed`'s high word before
the draw, and drives the fight with a fixed input policy. `--vehicle N` picks
the vehicle record and `--event N` forces an event battle instead of a
formation. [`BATTLE_ORACLE_FORCED.md`](../docs/oracle/BATTLE_ORACLE_FORCED.md)
owns the mechanism with its citations, every option, the output directory, the
captures and what they prove.

`--delay N` inserts idle input frames at the encounter seam, before battle
loading, rather than waiting N frames at the ready command menu. It can change
fixed-policy captures (including the recorded Juza event capture), but it does
**not guarantee a different RNG path for `--event N`**. Event battles select
`Event_Battle_Index` directly, without a formation draw or a seed patch
(`oracle/force/selectors.py:event_selector`). With `--party-script`, the pilot
preserves the probe prefix through enemy loading, then supplies inputs when
menus are ready (`oracle/force/phases.py`, `oracle/force/script_capture.py`).
Idle frames during that load can leave both the seed and later inputs unchanged:
the Dark Force 3 scripted case produced identical action frames at delays 1–5
([observation](../docs/battle/ENEMY_ABILITIES_ENDGAME.md)).

For those event cases, increase `--max-rounds` to observe later natural rolls;
provide enough script rounds or `repeat_last: true`, and increase `--repeats`
if the pilot exhausts its frame budget. Require the desired ability in the
capture and check the extracted kept rounds. A delay value alone is not
evidence of changed rolls. The runner's case fields are `rounds`, `keep`,
`script` and `repeats`.

### Early event bosses

The event selector tests and party menu pilot also cover bosses with no
formation draw. To capture Juza's ZAN/FORCEFLASH and Zio's complete initial
barrier → nightmare → BLACK WAVE sequence:

```sh
python3 -m oracle.force --event 3 --prepare-script \
  --scout build/event-bosses/script-scout.json --out build/event-bosses/base
python3 -m oracle.sweep.event_bosses --capture --extract
```

The recipe writes `commands.json` and passes explicit `--ram-patch` party
fixtures at the encounter seam. All three party members DEFEND through the
ordinary menus. Juza uses 999 HP/defence/mental defence to survive sixteen
rounds; Zio uses 999 HP for three rounds. No phase counter or command RAM is
patched. These are isolated event fixtures, not a campaign route. The generic
Rust replay discovers `replay_fixtures/event_bosses/` automatically; the Python
recipe guard also requires both Juza abilities and Zio's ordered phases.
The `zio_psycho` case installs item `$39` in inventory (`$FFFFF410`) and uses
it through the item menu to change Zio's form, then captures CORRSION.
`zio_event06_psycho.json` is the earlier capture of that interaction.

### Capture sets

Every committed capture is a fixture under
`rust/psiv-core/src/battle/replay_fixtures/`, and its `provenance` names the
tape, selector and hashes it came from. The sets, and the recipe that
reproduces each:

| directory or files | what | recipe |
|---|---|---|
| `tape07_first_battle.json`, `tape09_second_battle.json` | the first battles, natural tapes | `oracle/tapes/07_*`, `09_*` |
| `forced_*.json` | single forced formations | `python3 -m oracle.force` ([forced](../docs/oracle/BATTLE_ORACLE_FORCED.md)) |
| `sweep_motavia/` | every Motavia formation | `python3 -m oracle.sweep` ([sweep](../docs/oracle/BATTLE_ORACLE_SWEEP.md)) |
| `arc_motavia/`, `arc_zelan/` | status and stat abilities | `python3 -m oracle.sweep.arc`, `oracle.sweep.zelan` |
| `arc_zelan_damage/`, `air_castle/`, `endgame/` | later damage and effect pairs, bosses | `oracle.sweep.forced_cases` (`air_castle`, `endgame`) |
| `siren386_*`, `browren486_*`, `zio_event06_psycho.json` | party-script captures | `oracle.force --party-script` |
| `player/` | party techniques and skills | `python3 -m oracle.sweep.player_abilities` |

What the status and stat captures show is
[ENEMY_EFFECT_ABILITIES.md](../docs/battle/ENEMY_EFFECT_ABILITIES.md#10-what-the-captures-show).

### One runner for capture recipes

All sweep recipes use `oracle.sweep.forced_cases`: a JSON case list or a
recipe's declared `Case` values enter the same capture, extraction and
validation loop. `arc`, `zelan`, `air_castle`, `endgame` and `event_bosses`
provide named recipes; route damage and player cases use `--cases`.
The region sweep supplies generated formation cases to that same runner.

```sh
python3 -m oracle.sweep.forced_cases --cases build/cases.json \
  --work build/captures --out build/fixtures --capture --extract
```

A minimal case file is:

```json
[
  {"name": "helex", "formation": 94, "ability": 2, "rounds": 5}
]
```

Each case selects exactly one `formation` or `event`. `rounds` bounds capture;
zero captures the whole battle. `keep` optionally retains a positive number of
rounds (at most the capture limit when bounded). `ability` and `also` require observed
enemy actions, `damage` additionally requires observed damage, and `allowed`
restricts the kept abilities. `seated` validates a reload's observed enemy
instead of an ability byte. `script` holds party commands and explicit party
fixtures; see [party scripts](#party-command-scripts). `durable` defaults to
true. Cases can opt into bounded retries with `delay` and `max_delay`; the
default is one attempt. Scripted event cases refuse a delay search: extend
the battle instead. Event/script delay limits are
[explained above](#forced-battles-capturing-any-formation-on-demand).

Each case keeps its force/extract logs and a receipt with commands, exit
statuses, timing and log hashes under `--work`. A fixture is published only
after the kept rounds pass validation. `--extract` without `--capture` reuses
existing evidence after checking hashes and the case fingerprint; captures
made before the shared runner need a fresh capture to obtain that fingerprint.
`--only NAME` selects a case; `--list` shows the plan without running it.
`--scout FILE` supplies a shared scout cache; scripted cases require one made
with `oracle.force --prepare-script`.

### Party command scripts

`--policy attack|defend` keeps its existing inputs and default (`attack`).
`--party-script commands.json` selects commands through the cartridge's own
COMD, lists and target cursors. First cache the base tape's actual party and
inventory at its encounter seam:

```sh
python3 -m oracle.force --formation 0x126 --prepare-script \
  --scout build/scripted/scout.json --out build/scripted/base
python3 -m oracle.force --formation 0x126 --durable --max-rounds 4 \
  --party-script commands.json --scout build/scripted/scout.json \
  --require-ability 0x1D --out build/scripted/siren
```

Scripted captures observe three to five occupied slots, including the last
two members’ HP, status, resources and battle stats. Legacy `alys`, `chaz`
and `hahn` column names alias slots 1–3; `party4` and `party5` alias slots
4–5. Each alias follows the character record actually selected in that slot.

The script is strict JSON. Every round names all occupied party fighters by
one-based battle slot (1..5), not character id. All keys shown are required;
DEFEND has no `id` or `target`:

```json
{
  "rounds": [
    {
      "1": {"command": "technique", "id": "0x01", "target": 6},
      "2": {"command": "attack", "target": 7},
      "3": {"command": "defend"}
    },
    {
      "1": {"command": "skill", "id": 6, "target": 6},
      "2": {"command": "item", "id": "0x39", "target": -1},
      "3": {"command": "defend"}
    }
  ],
  "repeat_last": true
}
```

IDs accept integers or hexadecimal strings. Targets are fighters 1..5 (party),
6..9 (enemies), or `-1` for a command with no target cursor (whole-side/self
range, or a multi-target weapon). Script rounds count **player command phases**:
an ambush round consumes no script round. `repeat_last` defaults to false;
exhausting a finite script refuses the run. The example is a format illustration,
not a promise that an arbitrary base party owns its named skills/items.

Preflight reads the hashed RAM observation and decoded definitions from the
ignored `runtime-pack/battle/` pack. It rejects unknown/unlearned techniques,
exhausted skills, unaffordable techniques, absent items and invalid ranges before
any probe or capture frame runs. Later list entries are checked live too. Explicit
`--ram-patch FRAME:FFFFADDRESS:HEX` fixtures are allowed only with a script, only
at `battle_first + 1`, and only within party/inventory RAM `$F400..$FA7F`.
They never write commands, menu cursors or list scratch. What a scripted capture
guarantees is [BATTLE_ORACLE_FORCED.md](../docs/oracle/BATTLE_ORACLE_FORCED.md#9-party-command-scripts-what-a-capture-guarantees).

For the complete party-side inventory, use
`python3 -m oracle.sweep.player_abilities --require-captures` and the generated
[player ledger](../docs/battle/PLAYER_ABILITIES.md). Its source-chain and
negative-control rules are owned there. A recipe batch uses the same driver:

```sh
python3 -m oracle.sweep.forced_cases --cases build/player/cases.json \
  --scout build/scripted/scout.json --work build/player/captures \
  --out rust/psiv-core/src/battle/replay_fixtures/player --capture --extract
cargo test --manifest-path rust/Cargo.toml -p psiv-core \
  --lib every_fixture_replays_as_recorded
```

Case files declare `name`, `formation` (or `event`) and a round count. In
`script`, declare command `rounds`, optional `defaults` and per-slot `party`
stat/status/resource settings. Optional `characters`
selects three distinct stored character records; observation addresses follow
those actual records. The capture wrapper splits each record patch into the
host's 64-byte chunks. It checks both stock log hashes, records each command's
exit/timing/log hash, and refuses an ability that was selected but never ran.
`--only NAME --extract` reuses completed, hash-verified observations;
it does not rerun or alter the cartridge evidence. Keep recipes and raw RAM in
ignored `build/`. These explicit fixtures do not certify a connected route.
Party scripts refuse vehicle-only formations before launching the host.
The pilot's terminal error names its last frame, menu state and queued presses;
inspect those logs before retrying.

`Battle_ProcessCOMD` (`ps4.asm:7635-7661`) advances the party slot, skips
status-ineligible actors and bypasses command input during an enemy ambush.
The command strip is horizontal (`Battle_CharCommand`, `ps4.asm:2192-2225`).
The cartridge builds reverse learned technique/skill lists and equipment-first
item lists (`Battle_FillTechList`, 1695-1730; `Battle_FillSkillList`, 1734-1792;
`Battle_FillItemList`, 1796-1816). Live list cursors select four-row pages
(`Battle_UpdateRedCursor2`, 1572-1603); disabled entries cannot be selected.
Enemy targets follow occupied objects (`Battle_PickTargetEnemy`, 1216-1250).
Party target order is spatial, so the pilot observes the actual cursor and
fighter coordinates (`Battle_PickTargetChar`, 1337-1373), without embedding a
ROM coordinate table. A pad press holds four frames then releases twelve.

A read-only pilot host pauses at input-block boundaries and observes RAM. It
preserves the probe's exact pad prefix through the measured formation draw:
changing it earlier can change the HV roll and select another formation.
`script-inputs.json` records the live decisions; `script-map.tsv/json` records
additional observations. The pilot freezes an ordinary tape, which the stock,
self-building host captures and verifies twice with the existing RNG checker.
The pilot is not the final evidence host. Extraction records each actual command,
its concrete item copy, inventory and end-of-round HP/TP/form/MDEF state; the
replay reads the decoded definitions those commands need from the local runtime
pack.

### Sweeping a region: `python3 -m oracle.sweep`

`python3 -m oracle.sweep` runs `oracle.force` over a region's whole formation
list (computed from the local pack; `--list` writes it under `build/`) and extracts a
fixture for each, three captures at a time, resuming where it stopped;
`--reextract DIR` re-extracts an existing sweep without the emulator.
[`BATTLE_ORACLE_SWEEP.md`](../docs/oracle/BATTLE_ORACLE_SWEEP.md) owns the
method (`--durable`, `--max-rounds`), the coverage and every divergence the
sweep found.
