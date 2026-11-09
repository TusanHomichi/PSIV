# Tape, log and RAM formats

[Oracle setup and index](README.md)

### Determinism

Tape A run twice from power-on produces **byte-identical** logs. `verify.sh`
prints the hash on every run and fails if the two differ; the hash itself is
deliberately not pinned in this document, because it changes whenever the RAM
map gains a column and a stale value here would look like a regression.

Core: **Genesis Plus GX v1.7.4**, libretro build, commit
`2d7131c5efa606f649d36e1685a8ca47c24f31b3`, region NTSC.

Core options are pinned explicitly in `host/core_options.c` and **validated
against the core's declared value list at startup, aborting on a mismatch**.
That guard exists because a wrong value is otherwise silent: Genesis Plus GX
parses its overclock option with `atoi()`, so an invalid `"1x"` became a 1%
clock divisor and produced a log full of plausible-looking zeroes rather than
an error. Anything not pinned falls through to the core's compiled-in default,
which is fixed for a fixed core build; `--dump-options` prints the full set.

## Tape format

Line-oriented, one step per line:

```
<frames> <buttons> [mark]
```

- `frames` — how many consecutive frames to hold this state, at least 1.
- `buttons` — `.` for nothing, otherwise letters from `UDLRABCS`
  (up/down/left/right, Genesis A, B, C, Start). `A`, `B` and `C` are the
  physical Genesis buttons; see the button mapping note below.
- `mark` — optional label, recorded in the log on the step's first frame.
  Marks are how findings get cited: every frame number in this document is a
  mark or an offset from one.

A run of steps repeats with a block (blocks do not nest):

```
repeat 336
4 C
12 .
end
```

Blank lines and `#` comments are ignored. **Playback always starts from
power-on**, so a tape plus a pinned core build fully determines the run; there
are no savestates in the loop.

### Button mapping (this bit is a trap)

The disassembly names buttons by *function*, and the mapping to physical
Genesis buttons is not the obvious one:

| Genesis button | joypad bit | disassembly name | field effect |
|---|---|---|---|
| B | 4 (`$10`) | `ButtonCancel` | nothing in field control |
| C | 5 (`$20`) | `ButtonSpeak`  | Talk — `FieldRoutine_Interaction` |
| A | 6 (`$40`) | `ButtonCamp`   | opens the camp menu |
| Start | 7 (`$80`) | `ButtonStart` | settings window |

(`ps4.constants.asm:1877-1892`; confirmed by pressing each in turn and
watching `Game_Mode_Routine` go to 8, 4, and $10 respectively.)

Genesis Plus GX maps libretro `Y`/`B`/`A` onto Mega Drive A/B/C, which the
host handles; tapes are written in Mega Drive letters.

## Log format

CSV with three provenance comment lines (four with `--rng-trace`, which adds
`# rng-trace=<name>`), then `frame,mark,buttons` and one column per enabled RAM
field. `frame` is 1-based and counts `retro_run()`
calls from power-on. Values are decimal, or fixed-width hex for fields flagged
`hex` in the map.

Every input is named by its file and not by the path the run was given:
`# rom=`, `# tape=` and `# rng-trace=` all carry a basename
(`oracle/host/provenance.h`), and a fixture stores them that way too
(`oracle/fixture/logs.py`'s `provenance_lines`). Two runs that differ only in
their directories - where the ROM or the tape sits, where the run wrote -
therefore produce byte-identical traces and byte-identical logs, which is what
lets a ledger pin a capture by sha256 and compare it against another run
(`tests/test_oracle_provenance.py`).

## RAM map

`ram_map.json` is the source of truth; **every address is transcribed from
`reference/ps4disasm/ps4.constants.asm` and carries the line it came from**,
except where a field's `source` names `ps4.asm` instead: an address the
constants file does not name, recovered from the operand the code itself uses.
`enemy_ability_index` (`$FFFFEEA8`) is the only such field so far - it is
`Enemy_Attack`'s ability re-roll word, read at `ps4.asm:19149` and written at
`ps4.asm:19151` - and the clears that reach it, the oracle measurements and what
the port does with it are in `docs/oracle/BATTLE_ORACLE_REPLAY.md`. Nothing was
inferred by watching memory. Struct fields are recorded as base + offset with
both citations so the arithmetic is auditable — for example `c1_facing` is
`Character_1` (`constants:2086`, `$FFFFC000`) plus `facing_dir`
(`constants:107`, `+6`).

`gen_ram_map.py` emits the flat `ram_map.tsv` the C host parses, validating
sizes, alignment and address range on the way. `verify.sh` regenerates and
diffs it, so the two cannot drift apart silently.

### Battle groups

The battle groups are all transcribed from the constants file with the
same citation discipline as the rest of the map:

| group | what it covers |
|---|---|
| `battle` | `Battle_Routine` ($FFFF4100), `Battle_Routine_2`, `Battle_Total_Comd_Input`, the cursor indices, `Battle_Priority` ($FFFFEE45: 0 normal / 1 surprise / $FF ambush), `Enemy_Count`, ambush and run chances, item drop rate, and `Battle_Turn_Order` ($FFFFEFB0) |
| `bhit` | `Fighters_Hit_Flags` ($FFFF4150; $00 normal, $01 critical, $FF miss or untargeted) and `Battle_Heal_Damage_List` ($FFFF415A), the per-fighter damage numbers |
| `enemy` | all four `Enemy_Stats` slots ($FFFF4200, $80 stride): id, level, HP/max HP, status, strength, agility and battle agility, dexterity, attack and defence |
| `chars` | Chaz and Alys from `Character_Stats` ($FFFFF500, $80 stride): level, EXP, HP/TP, status, the same stat block, plus `Current_Money` |

`Battle_Routine` values are decoded from `BattleRoutines` (`ps4.asm:7524`):
`$08` ProcessCOMD, `$0A` ProcessMACRO, `$0C` ProcessRUN, `$0E` **OrderTurns**,
`$16` **DoAttackEffect**. `Battle_OrderTurns` builds `Battle_Turn_Order` from
each living fighter's `agility_battle`, four bytes per entry (index word then
agility one byte in), sorted highest first — so turn order versus agility is
readable straight out of the log.

EXP is not accumulated in a battle-local total; `ps4.asm:4775` adds it directly
into each character's `exp` field, so the award is observed as a delta on
`chaz_exp` / `alys_exp`. Meseta accumulates in `$FFFF41D0` and is added to
`Current_Money` at `ps4.asm:4801`.

`python3 -m oracle.scripts.analyze_battle` reads a battle log and reports the
formation and stats, the routine timeline, the turn order, every HP change
with the hit flags, damage list and RNG seed at that frame, and the
EXP/meseta deltas.

### Work-RAM byte order

Genesis Plus GX builds with `-DLSB_FIRST`, which stores `work_ram` in
**host-native 16-bit word order**. The 68000 core reads bytes as
`READ_BYTE(base, addr) == base[addr^1]` and words as
`*(uint16 *)(base + addr)` (`core/macros.h`, `core/m68k/m68kcpu.h:854-882`).
So:

- byte at 68000 address `A` → `ram[A ^ 1]`
- word at even `A` → little-endian `uint16` load at `ram + A`
- long at even `A` → `word(A) << 16 | word(A + 2)`

Get this backwards and every log line is quietly wrong, so `verify.sh` proves
both paths against values the cartridge itself chose (see
[where the measurements live](README.md#where-the-measurements-live)).
