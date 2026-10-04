# X86: scripted party commands, BARRIER and Psycho Wand

2026-10-04, delivery branch `cx/x86-policy`, base `00f4936`.
Scope: issues [86](https://github.com/TusanHomichi/PSIV/issues/86) and
[72](https://github.com/TusanHomichi/PSIV/issues/72), through the party-script
and Zio capture boundary. No campaign/session/scene changes.

## Result and authority

Four cartridge captures, **20 rounds and 979 battle RNG calls**, replay exactly
through `every_fixture_replays_as_recorded`. Three formation draws are retained
outside the battle streams; the event battle draws none. The divergence manifest
remains empty. Each capture passed the RNG arithmetic/seed-chain checker and
two stock-host runs produced byte-identical traces and RAM logs.

| fixture under `rust/psiv-core/src/battle/replay_fixtures/` | rounds | battle calls | observed use |
|---|---:|---:|---|
| `siren386_126_magic.json` | 4 | 209 | fighter 6 BARRIER, round 3, f25745; both enemies' MDEF 2 → 52 |
| `siren386_127_magic.json` | 4 | 280 | fighter 7 BARRIER, round 3, f26079; all three enemies' MDEF 2 → 52 |
| `browren486_1ac_barrier.json` | 6 | 276 | fighter 6 regular-slot BARRIER, round 4, f26069; both enemies' MDEF 3 → 78 |
| `zio_event06_psycho.json` | 6 | 214 | Alys ITEM `$39` at f25473; enemy 139 → 140, HP 16383 → 2889, MDEF 255 → 38 |

These are capped battle/state/RNG proofs, not connected campaign, native-input
or visual-parity proofs. The cartridge receives ordinary joypad menu inputs;
formation/seed forcing, 999-HP durability and the explicitly supplied Psycho
Wand are declared fixture setup. Original assets remain ignored local inputs.

## The party-command path

The format and menu rules have one owner:
[oracle/README.md](../../oracle/README.md#party-command-scripts).
`script.py` validates every written round against a hashed observation of the
base tape's actual party, TP, learned lists and inventory. `script_menu.py`
reads live cursor/list state and expands commands to joypad presses.
`script_capture.py` uses a read-only frame hook, freezes the input tape, then
the existing capture/verify path runs the stock self-building host.

The script never writes `Character_Command_Data`, `Current_Command`, target
cells, list scratch or menu cursors. Queue-build readback rejects another
command, record or target instead of patching it. The probe's input prefix is
preserved through its measured formation draw and selector restoration: pad
changes before that draw can change HV and select another formation.

Fixture extraction reads command bytes 1..5 and each selected id/target, plus
the selected equipment/inventory copy. No-RNG party turns are observed through
`loc_57C4` (`ps4.asm:8045-8055`), only outside menus and after the actor's `$EE`
status exclusion (`loc_576A`, 8033-8043). List scratch aliases the actor/command
words and must not create phantom turns. Inventory, party TP, enemy forms,
HP, status and battle MDEF are also checked at round end.

Party definitions in the replay's existing generated mirror are emitted by
`oracle.sweep.replay_pack` from the ignored, psiv_tools-decoded runtime pack.
The native runtime still loads that pack through `psiv-data`; no retail ability
record is transcribed into new Rust/Python source. Party ability ids are kept
separate from enemy ability ids when selecting mirror records.

## BARRIER's complete chain

The owning rule/chain is
[ENEMY_EFFECT_ABILITIES.md](../battle/ENEMY_EFFECT_ABILITIES.md#barrier-1d-siren386-and-browren486).
Siren386's `$08` arm tests reaction bit 1 and clears the byte
(`EnemyAI_MagicDamageReceived`, `ps4.asm:22500-22511`); magic damage sets that
bit at `loc_27D4` (3988-3989). Browren486 reaches the same arm from its regular
list. `EnemyAttack_Warren286` (22515), `loc_100A6` (22559), guards with a
**signed** comparison at 22567-22570. Raised caster MDEF returns through
`loc_10016` (22518) to a cleared-ability physical swing.

The parent `$1E0`, `loc_1769E` (32706), calls the effect dispatcher once at
32771. `AbilityEffect_MagicDefenseUp` (9220-9233) sets living enemies' battle
MDEF from their own derived MDEF plus the caster's STR, wrapping a 16-bit word.
No resistance selector means no effect RNG (`Effect_SetupSkillParams`,
9576-9595). The captures show the range directly, and the Siren captures show
the subsequent raised-MDEF fallback. Browren's two calls at f26069 are ability
selection/re-roll, not effect calls.

The regular inventory is recomputed from all 83 rows: **45 implemented,
4 partial, 34 unsupported** (27 damage, 6 status/stat, 1 scripted/custom).
The current `zelan-kuran` derivation lists 24 abilities and **zero unsupported**.

## Zio's opening and Psycho Wand

Event 6 loads enemy 139. The cartridge opens this encounter as an enemy ambush,
so script round 1 is the first *player* command phase, battle round 2.
Zio acts before Alys: Magic Barrier at f24896, Nightmare at f25225, then her
Psycho Wand at f25473. The capture therefore observes the two scripted phases
and the transition into normal vulnerable-form attacks, through battle round 6.

`ItemObj_PsycoWand` (`ps4.asm:16305`) invokes the item effect and loads its
object. `loc_3CF60` (79203-79215) changes only the first Zio formation entry,
then `loc_7F22` (11908-11939) reloads the enemy records without running init.
The phase counter is retained. Replay compares `EnemyStatsReloaded` against
the decoded record and the observed cells, instead of requiring individual
`StatChanged` events. The capture revealed no new engine-rule divergence;
the missing comparison was in the replay harness.

## Reproduction and retained evidence

Local evidence root: `build/x86-evidence/`. Scripts and base RAM observations
are local evidence, not original-game source assets to stage.
Prepare the base state as shown in the README, then:

```sh
python3 -m oracle.force --formation 0x126 --durable --max-rounds 4 \
  --party-script build/x86-evidence/siren-script.json \
  --scout build/x86-evidence/base/scout.json --require-ability 0x1D \
  --out build/x86-evidence/siren-126
python3 -m oracle.force --formation 0x127 --durable --max-rounds 4 \
  --party-script build/x86-evidence/siren-three-script.json \
  --scout build/x86-evidence/base/scout.json --require-ability 0x1D \
  --out build/x86-evidence/siren-127
python3 -m oracle.force --formation 0x1AC --durable --max-rounds 6 \
  --party-script build/x86-evidence/defend-script.json \
  --scout build/x86-evidence/base/scout.json --require-ability 0x1D \
  --out build/x86-evidence/browren-1ac
python3 -m oracle.force --event 6 --durable --max-rounds 6 \
  --party-script build/x86-evidence/zio-script.json \
  --ram-patch 24795:FFFFF410:39 --scout build/x86-evidence/base/scout.json \
  --out build/x86-evidence/zio-psycho
python3 -m oracle.sweep.replay_pack
```

Siren scripts: Alys FOI `$01` at fighter 6 or 7 for the first two player
command rounds; Chaz/Hahn defend; all defend thereafter. Browren: all defend.
Zio: Alys ITEM `$39`, Chaz/Hahn defend in the first player command round;
all defend thereafter. These scripts repeat the last round.

Extract with `python3 -m oracle.fixture`, the report's exact `trace`, `log`,
`tape`, `battle_first` and `battle_last`, the capture's `script-map.json`,
`--hp-patch 999 --minified`, and the same `--max-rounds`. The command arrays
are retained beside the extraction logs. Each committed fixture records full
trace/log hashes and host/core/ROM provenance; each capture directory retains
`report.json`, the frozen tape, patches, pilot input decisions, capture and verify.

Failed first attempts are retained: changing inputs before the formation draw
selected the wrong formation; a mistyped extraction endpoint was rejected for
missing frames; an initial Zio comparison lacked reload-stat support. No
divergence exception or altered captured value was used to make a replay pass.

## Checks and remaining scope

The delivery receipt in `build/x86-evidence/` records actual commands, exits,
counts and timings. Negative controls cover unknown techniques rejected before
any frame runs; wrong queue-readback commands/ids/targets; disabled live lists;
invalid item cursor/copy observations; menu scratch and skipped no-RNG turns;
wrong BARRIER records/carriers; signed/word-wrap boundaries; and a one-point
captured MDEF error or substituted party command failing generic replay.

Correction ladder: **rung 2, gate-run guards with negative controls**. Internal
types cannot guarantee which command the external cartridge accepted; live
readback and generic replay validate that boundary.

Issue 72's Juza capture and unrelated deferred enemy carriers are outside X86.
The four captures do not certify other party abilities' object chains or
animation timing. No publishing, merge, issue mutation or owner-asset cleanup
is part of this delivery.
