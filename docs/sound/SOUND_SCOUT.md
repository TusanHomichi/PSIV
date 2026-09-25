# PSIV sound architecture scout

Retail research recorded 2026-08-16. The native driver/core path is now
implemented in `psiv-sound`; see [sound extraction](SOUND_EXTRACTION.md) and
[sound integration](SOUND_INTEGRATION.md) for implementation and evidence.
This document retains cartridge research and comparison methods. The old
implementation estimates and alternative-product rankings have been removed.

## Retail architecture

PSIV uses a **modified SMPS 68k driver**, with a PSIV-specific command set and
channel/DAC behavior. It is not stock Sonic-style SMPS data that a generic
SMPS player can consume unchanged. The 68000 driver owns sequencing,
instruments, envelopes, YM2612 writes, PSG writes, channel stealing, and
sound-ID dispatch. A small embedded Z80 program exists primarily to stream
the DAC sample banks; it is not the music sequencer.

The practical 1:1 path is:

1. Keep the ROM-derived sound records byte-exact in the local, gitignored
   runtime pack, with explicit pointer and record metadata.
2. Transcribe the PSIV driver grammar and scheduler rather than converting the
   music to MIDI or assuming a stock SMPS format.
3. Drive a separately selected YM2612-compatible core and SN76489-compatible
   PSG core.
4. Use the existing emulator oracle to capture PCM and, after a small future
   instrumentation step, timestamped register writes.

That makes the recommended playback strategy **hybrid**: oracle WAV/VGM-like
traces for fixture baselines and debugging, plus a live native driver/core for
runtime playback. Rendered audio is useful local evidence; it is not a sound
system replacement and must not become distributed extracted content.

---

## 0. Evidence and citation method

### Retail image

The file named `Phantasy Star IV (USA).md` is the verified US retail ROM, not
a Markdown source file:

| Property | Value |
|---|---|
| Path | `Phantasy Star IV (USA).md` |
| Size | 3,145,728 bytes |
| SHA-256 | `511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a` |

The load-bearing addresses in this report are **ROM byte offsets**, verified
against that image. The checked-out public orientation source is
[`alechenninger/ps4disasm`](https://github.com/alechenninger/ps4disasm),
currently at clone commit `5d8dcb56943d2d9430b702950daab4436fb109e8`.
The clone supplies labels, structure, and prose; it is not independently
trusted as the retail byte source.

The following short signatures were checked in the retail image. They are
structural citations only; this repository does not ship a ROM or large ROM
extract.

| Retail offset | Short retail signature | What it anchors | Orientation citation |
|---|---|---|---|
| `0xD0008` | `4D F9 00 FF 50 00 42 2E 00 0E 4A 2E 00 07 66 00` | `UpdateSound` entry and `$FF5000` sound RAM base | `sound/ps4.sound_driver.asm:1-17`, label `UpdateSound` |
| `0xD0632` | `4E 75 7E 00 1E 2E 00 09 67 00 05 DE 1D 7C 00 80` | `DoSoundQueue` boundary and `PlaySoundID` dispatch | `sound/ps4.sound_driver.asm:577-633` |
| `0xD1A60` | `00 0D 1D 10 00 0D E5 C2 00 0D 1C 40 00 0D E4 B6` | General sound pointer block | `sound/Pointers.txt:1-10`, label `loc_D1A60` |
| `0xD1C40` | `00 0D 1D 8E 00 0D 23 86 00 0D 24 94 00 0D 29 4E` | First entries of the music pointer table | `sound/Pointers.txt:2`, `sound/ps4.sound_driver.asm:2139-2192` |
| `0xD1D8E` | `05 AC 06 03 02 09 05 34 00 00 00 41 F4 16 01 A5` | First music header and relative voice-table pointer | label `Music_TonoeDePon`, `sound/ps4.sound_driver.asm:2211-2218` |
| `0xD233A` | `21 31 01 71 01 9B 1F 15 5F 07 0A 09 0C 08 08 05` | First music record's local FM voice table | label `loc_D233A`, `sound/ps4.sound_driver.asm:2353-2354` |
| `0xDE4B6` | `00 0D E5 CE 00 0D E5 F8 00 0D E6 20 00 0D E6 4E` | First entries of the SFX pointer table | `sound/Pointers.txt:3`, `sound/ps4.sound_driver.asm:8907-8981` |
| `0xDE5CE` | `00 11 01 01 80 05 00 0A 00 04 EF 00 84 03 89 04` | First SFX header, relative track pointer, and control bytes | label `SFX_Rod`, `sound/ps4.sound_driver.asm:8985-8993` |
| `0xD153E` | `F3 F3 31 00 01 0E 00 06 00 10 FE 0D 20 F9 C3 00` | Embedded Z80 DAC program | `sound/ps4.sound_driver.asm:1981-1987`, label `loc_D153E` |
| `0xD1A3E` | `00 00 00 01 00 02 00 03 00 04 00 05 01 00 00 07` | Z80 DAC sound-to-bank lookup | `sound/Pointers.txt:6-10`, `sound/ps4.dac_driver.asm:509-526` |
| `0xE0000` | `50 80 50 0C 50 80 50 0C A0 8C A0 04 A0 8C A0 04` | First DAC bank's 8-byte sample records | `sound/Notes.txt`, `sound/ps4.sound_driver.asm` bank section |

`docs/source-notes/disassembly-discrepancies.md` and the existing battle scout establish the clone-drift
rule: `loc_XXXXX` labels are retail addresses when their bytes match the
cartridge; generated inline comments such as `;0x0 (0x0000...)` are fork
build addresses and can drift. This report cites labels and retail offsets,
never those generated comment offsets. Source line numbers are orientation
only and may move when the clone changes.

---

## 1. The driver

### 1.1 What PSIV actually runs

The source's own definition says both things that matter:

- [`DefDrv.txt`](../../reference/ps4disasm/sound/DefDrv.txt) is a “SMPS 68k
  Driver Definition” with `DrumChnMode = PS4`, PSIV frequency tables, and
  PSIV envelope commands.
- [`DefCFlag.txt`](../../reference/ps4disasm/sound/DefCFlag.txt) explicitly says
  “Phantasy Star IV SMPS Command Definition” and “modified SMPS 68k”.

So the honest classification is **PSIV's custom/modified SMPS-family 68k
driver**. The SMPS ancestry is real and useful for orientation, but a stock
SMPS extractor or player is not the format specification.

The driver is included into the retail image at the sound-driver boundary
(`ps4.asm:183553-183564`). `UpdateSound` runs from the 68k side and services:

- six FM channels;
- three PSG channels;
- DAC tracks and the special FM3/DAC paths;
- a separate SFX channel allocation that can mute or steal BGM channels;
- special vehicle/transport SFX channels.

The frame/update routine starts at retail `0xD0008` and walks the channel
state in `$FF5000` RAM. It calls `DoSoundQueue`, dispatches a sound ID, then
updates DAC, FM, and PSG tracks in phases
(`ps4.sound_driver.asm:1-72`). The YM writes are synchronized through the
YM ports at `$A04000`; PSG writes go to `$C00011`
(`ps4.sound_driver.asm:1180-1257`, `1312-1478`).

### 1.2 The Z80 is the DAC player, not the sequencer

`InitSoundDriver` takes the embedded blob at `loc_D153E`, copies it to Z80
address `$A00000`, resets/releases the Z80, and then stops all sound
(`ps4.sound_driver.asm:1145-1174`). The blob is the Z80 program in
[`ps4.dac_driver.asm`](../../reference/ps4disasm/sound/ps4.dac_driver.asm),
whose `zInitDriver` clears `$1F00..$1FFF` and waits for DAC requests.

The Z80 program:

- selects a bank and sample using its table at Z80 offset `$0500`;
- reads 8-byte little-endian sample records;
- writes YM2612 register `$2A` for DAC data;
- controls DAC enable/stereo through YM registers `$2B` and `$B6`;
- supports sample direction, looping, and volume control.

The sound driver's custom commands `EE`, `FA`, `E3`, `E4`, `FC`, and `FD`
write the corresponding Z80 control state. That is why “the Z80 sound
driver” is an incomplete answer: it handles PCM/DAC transport, while the
68k driver interprets music and SFX bytecode and writes FM/PSG state.

### 1.3 Where the data lives

The useful ROM regions are:

| Content | Retail location | Notes |
|---|---:|---|
| 68k driver entry | `0xD0008` | `UpdateSound`; driver code continues through the sound-driver block |
| General sound pointers | `0xD1A60` | Priorities, special SFX, music, SFX, modulation/volume tables, `$B5` base, update pointer |
| DAC lookup table | `0xD1A3E` | 17 `(bank, sound-in-bank)` pairs; the Z80 copy is at `$0500` |
| Pan-animation list | `0xD0530` | Used by the `FF/00` meta path |
| Modulation pointers/data | `0xD1A80` onward | Eight pointer entries, followed by modulation envelopes |
| PSG/volume pointers/data | `0xD1B44` onward | Ten volume-envelope pointer entries and data |
| Music pointer table | `0xD1C40` | 4-byte absolute pointers, IDs `$81..$B4` |
| First music target | `0xD1D8E` | Target of the first pointer, `TonoeDePon`; records use relative pointers |
| SFX pointer table | `0xDE4B6` | 4-byte absolute pointers, IDs `$B5..$F7` |
| Special SFX pointer table | `0xDE5C2` | 4-byte pointers for IDs `$F8..$FA` |
| First SFX record | `0xDE5CE` | `SFX_Rod`; track stream `loc_DE5D8`, local FM voice `loc_DE5DF` |
| DAC sample banks | `0xE0000`, `0xE8000`, `0xF0000`, `0xF8000` | Table plus uncompressed sample bytes; counts are 10, 2, 4, and 1 = 17 sounds |

The music and SFX records are not just note streams. A music header points to
a local FM voice table, declares FM/PSG channel counts and timing bytes, then
provides relative track pointers. `PlayMusic` reads the FM count at header
`+2`, PSG count at `+3`, resolves the first word as a relative local voice
table pointer, and populates the six FM / three PSG channel states
(`ps4.sound_driver.asm:635-744`). The first FM and PSG channel selectors are
the driver's `FMInitBytes` and `PSGInitBytes` tables.

An FM voice record is 25 bytes in the driver path: one algorithm byte, 20
operator register values, and four operator volume values. The driver walks
records in 25-byte strides and writes the YM2612 algorithm, operator, and
volume registers. Pan is held in channel state and written separately to
`B4` (`ps4.sound_driver.asm:1657-1708`, `1756-1761`).
This is a local voice table per music/SFX record, not a safe assumption of one
global instrument bank.

Track bytes are interpreted by the PSIV command dispatcher. Values below
`$E0` are note/rest/delay data; `$E0..$FE` dispatch through `cfHandler`, and
`$FF` enters the metadata path (`ps4.sound_driver.asm:1324-1345`,
`1480-1524`). The important custom command families are:

| Commands | Meaning in PSIV |
|---|---|
| `E0-E2` | Pan/AMS, detune, communication state |
| `E3-E4`, `ED`, `EE`, `FA`, `FC`, `FD` | DAC volume/loop/pan/sample/reverse/volume/track mode |
| `E5-E6`, `EC` | FM/PSG volume changes |
| `E7-EA` | Hold, note stop, LFO, tempo |
| `EB` | Trigger a sound command from a track |
| `EF` | Select FM instrument |
| `F0-F1`, `F4` | Modulation setup and envelope/type |
| `F2-F9` | End, noise, goto, loop, subroutine, return |
| `FB` | Transpose |
| `FE` | Special FM3 mode |
| `FF/00` | Pan-animation metadata |

The command names and lengths are from the driver's own definition file;
their execution is in `cfJumpTable` and its handlers. In particular,
`EB` means SFX can be reached from sequence data as well as from game code.

### 1.4 Sound-ID dispatch and music mapping

The shared queue is four byte slots beginning at `$FFFF500A` (`Sound_Index`),
with the latched ID at `$FF5009` in the driver's `$FF5000` state block.
`DoSoundQueue` drains the slots, applies `SndPriorities`, and latches the
winning request (`ps4.sound_driver.asm:577-600`). `PlaySoundID` then applies
these ranges (`ps4.sound_driver.asm:602-633`):

| ID range | Action |
|---|---|
| `$00..$7F` | Stop-all path for values below `$80` |
| `$80` | No-op/ignored request |
| `$81..$B4` | Music; index into `MusicPtrs` after subtracting `$81` |
| `$B5..$F7` | Regular SFX; index into `SFXPtrs` after subtracting `$B5` |
| `$F8..$FA` | Special SFX; index into `SpcSFXPtrs` after subtracting `$F8` |
| `$FB` | Fade out |
| `$FC` | Stop regular SFX |
| `$FD` | Stop special SFX |
| `$FE` | Stop all sound |

The complete music ID mapping is:

| ID | Symbol | ID | Symbol | ID | Symbol |
|---:|---|---:|---|---:|---|
| `$81` | `TonoeDePon` | `$92` | `Fal` | `$A3` | `EnemyAppearance` |
| `$82` | `Inn` | `$93` | `TempleNgangbius` | `$A4` | `HerLastBreath` |
| `$83` | `MotabiaVillage` | `$94` | `Thray` | `$A5` | `Pain` |
| `$84` | `MotabiaTown` | `$95` | `DefeatAtABlow` | `$A6` | `JijyNoRag` |
| `$85` | `OrganicBeat` | `$96` | `CyberneticCarnival` | `$A7` | `DungeonArrange2Cont` |
| `$86` | `DezorisTown1` | `$97` | `TerribleSight` | `$A8` | `TheBlackBlood` |
| `$87` | `NowOnSale` | `$98` | `EdgeOfDarkness` | `$A9` | `RedAlert` |
| `$88` | `BehindTheCircuit` | `$99` | `DezorisField1` | `$AA` | `Laughter` |
| `$89` | `MachineCenter` | `$9A` | `Tower` | `$AB` | `Mystery` |
| `$8A` | `InTheCave` | `$9B` | `TakeOffLandeel` | `$AC` | `EndOfTheMillennium` |
| `$8B` | `Winners` | `$9C` | `DezorisTown2` | `$AD` | `Explosion` |
| `$8C` | `FieldMotabia` | `$9D` | `DezorisField2` | `$AE` | `StaffRoll` |
| `$8D` | `LandMaster` | `$9E` | `AHappySettlement` | `$AF` | `ThePromisingFuture1` |
| `$8E` | `RequiemForLutz` | `$9F` | `Suspicion` | `$B0` | `PaoPao` |
| `$8F` | `MeetThemHeadOn` | `$A0` | `TheKingOfTerrors` | `$B1` | `DungeonArrange2` |
| `$90` | `RyucrossField` | `$A1` | `TheAgeOfFables` | `$B2` | `ThePromisingFuture2` |
| `$91` | `DungeonArrange1` | `$A2` | `Abyss` | `$B3` | `DezorisDeDon` |
|  |  |  |  | `$B4` | `Ooze` |

This table is duplicated as symbols in `ps4.constants.asm:918-969` and as
the pointer labels in `ps4.sound_driver.asm:2141-2192`. The pointer table is
the actual ID-to-record mapping: entry `n` is a 4-byte pointer, and each
record contains its own relative track/voice references.

Regular SFX are the 67 IDs `$B5..$F7`, named by the pointer table and constants
at `ps4.sound_driver.asm:8907-8975` and `ps4.constants.asm:973-1053`.
Special SFX are `$F8` `SpaceshipRadar`, `$F9` `LandRover`, and `$FA`
`Hydrofoil` (`ps4.sound_driver.asm:8977-8981`). The first regular SFX record
is `SFX_Rod` at retail `0xDE5CE`; its local track stream and FM voice are at
`0xDE5D8` and `0xDE5DF`. The final special record is the labeled `loc_DF772`,
immediately before the sound-bank section.

### 1.5 How game code triggers music and SFX

All three gameplay surfaces converge on the same queue. They do not call a
different “field audio” or “battle audio” engine.

**Field and map arrival.** The runtime pack already carries each map's
`music.id`, symbol, and `changes_music` flag
(`psiv_tools/pack.py:538-542`; `rust/psiv-data/src/map/settings.rs` (`Music`)). ID `0`
means keep the current track. A nonzero arrival ID is the equivalent of a
retail `move.b #MusicID_..., (Sound_Index).l` request. The map data is
therefore the correct input to the ID table above, not a second sequence
mapping to invent.

**Field scenes and cutscenes.** Scene code writes the same `$FFFF500A` queue
slot and optionally `$FFFFECEC` (`Saved_Sound_Index`) for later restoration.
Byte-verified examples in the existing scene reports:

- Game start writes `$84` `MotabiaTown` and saves `$84`
  ([`scenes/01_GameStart.md`](../scenes/01_GameStart.md):395-396).
- Basement containers writes `$AB` `Mystery`, waits through `VInt_Prepare`,
  then writes `$8A` `InTheCave`
  ([`scenes/07_BasementContainers.md`](../scenes/07_BasementContainers.md):29-39).
- Principal's confession writes `$9F` `Suspicion`, then `$84`
  `MotabiaTown` ([`scenes/10_PrincipalConfession.md`](../scenes/10_PrincipalConfession.md):40-52).
- The Piata guards scene writes `$FB` to stop music, then `$84` and saves it
  ([`scenes/11_PiataGuardsReprimand.md`](../scenes/11_PiataGuardsReprimand.md):28-42).

The explicit waits matter. A request is consumed by the sound update at a
frame boundary, and scenes use `VInt_Prepare`/`DoMapUpdateLoop` when the exact
ordering is visible or load-bearing.

**Battle entry.** `RunEventBattle` first writes `Sound_StopAll`, burns a
`VInt_Prepare`, indexes `EventBattleMusicData` with `Event_Battle_Index`, and
writes the selected music ID to `Sound_Index`
(`ps4.asm:120803-120835`, retail routine label `RunEventBattle`). The table
maps event indices to `DefeatAtABlow`, `MeetThemHeadOn`, `Laughter`, or
`TheKingOfTerrors`. On victory/battle exit, the battle code can restore
`Saved_Sound_Index` or stop music (`ps4.asm` around the battle result sound
handling at `6410-6422`).

**Battle and scene SFX.** Battle menus, attack animation objects, enemy
effects, and cutscenes write regular SFX IDs to the same `Sound_Index` slot.
For example the battle command UI writes `SFXID_Selection` and
`SFXID_MovingCursor` (`ps4.asm:1928-1930`, `1584-1593`), while attack object
code writes IDs such as `SFXID_EnemyKilled`, `SFXID_LaserAttack`, and
`SFXID_Megid`. The driver then steals/mutes the required BGM channels using
`BGMChnPtrs`/`SFXChnPtrs` (`ps4.sound_driver.asm:757-844`). Track command `EB`
is an additional sequence-internal SFX route.

---

## 2. The extraction path

### 2.1 Is there an existing documented format?

There is useful community knowledge, but not a safe PSIV drop-in format.

- Broad Genesis SMPS research and released Sega sound documentation cover
  common 68k/Z80 layouts, note timing, instruments, envelopes, and common
  command idioms. See the [Mega Drive/Genesis sound-driver list](https://vgmpf.com/Wiki/index.php?title=Mega_Drive%2FGenesis_Sound_Driver_List)
  and the discussion of Sega of Japan's released [SMPS source and sound
  documents](https://forums.sonicretro.org/index.php?threads%2Fsega-of-japan-sound-documents-and-smps-source-code.39425%2F=).
- PSIV's own `DefCFlag.txt` says “modified SMPS 68k” and adds PSIV-specific DAC,
  FM3, envelope, and command behavior. That is direct evidence that generic
  SMPS assumptions stop being authoritative at the command-dispatch level.
- Community VGM output exists, including a [PSIV VGMRips
  pack](https://vgmrips.net/packs/pack/phantasy-star-iv-mega-drive-genesis),
  but a rendered/logged result is evidence of playback, not a verified
  extraction specification or a replacement for the retail driver.

SMPS research is therefore applicable as **a vocabulary and starting point**:
it can explain relative pointers, channel records, common envelope markers,
and why instruments are arranged as they are. It is not applicable as a
generic parser/player without a PSIV-specific command table and tests.

### 2.2 What can be extracted

Yes. The records are structured enough to extract into a documented local
format:

```text
manifest: retail_rom_sha256, region, driver/source revision
music[id]: raw header, resolved FM voice slice, FM track slices,
           PSG track slices, envelope references, pan/meta references
sfx[id]:   raw header, local FM voice slice, track/channel descriptors
special[id]: same, plus special FM3/PSG routing
dac:       ID -> bank/sample table, 8-byte sample records, raw sample slices
```

The format should preserve both the raw bytes and the resolved spans. A
consumer can inspect the exact bytes while the interpreter uses the resolved
relative pointers. The manifest must record the ROM SHA and refuse a pack
from another retail build.

### 2.3 Pragmatic extraction recommendation

Use **raw sound records plus a transcribed PSIV interpreter** as the canonical
runtime input.

Do not convert the music to MIDI, a generic SMPS dialect, or a pre-rendered
“note format” first. Those transformations lose or obscure:

- tick/tempo and loop timing;
- local instrument selection and algorithm-dependent volume handling;
- modulation and PSG volume envelopes;
- DAC note pitch, reverse, loop, and volume control;
- SFX channel stealing and BGM restoration;
- `EB` sequence-triggered sounds and special FM3 behavior.

The raw-plus-interpreter approach is also the cleanest audit surface: every
emitted chip write can be traced back to a retail byte and a driver handler.
An eventual normalized analysis format is still worthwhile, but it should be
a view over byte-exact records, not a lossy replacement.

The existing runtime rule applies here: the pack contains Sega-derived data
and is never committed (see [runtime architecture](../RUNTIME_DESIGN.md)). “Gitignored” means
local build output, not permission to publish extracted sequences, voices,
DAC samples, VGM logs, or rendered audio.

---

## 3. Selected playback path

The game uses the live Rust driver and chip cores. Extracted records remain
in the local pack, and captured audio/register traces remain local verification
artifacts. Current integration and remaining SFX coverage are recorded in
[SOUND_INTEGRATION.md](SOUND_INTEGRATION.md). Original game content and
third-party components retain their own rights and licenses; the project MIT
license does not relicense them.

## 4. The oracle angle

Audio correctness needs more than a waveform that sounds plausible. Use three
levels of evidence.

### 4.1 Primary: register-write and event traces

Instrument the oracle at the sound-chip write boundary and record, for every
event:

```text
frame / emulated cycle
chip: YM2612, PSG, or DAC path
YM port + register/data, or PSG data byte
driver sound ID and queue event when known
```

For YM, record both address/data port writes and the cycle passed to GPGX's
`fm_write`. For PSG, record the cycle and byte passed to `psg_write`. DAC
events must include the YM `$2A` sample writes and the `$2B`/`$B6` control
writes, not just the eventual PCM.

Compare the candidate's normalized event stream to the oracle's stream in
order. First divergence is the useful failure. The comparison should pin:

- ROM SHA, region, emulator/core commit, YM mode, PSG mode, and audio options;
- reset and initial RAM state;
- VBlank/frame number and cycle/tick conversion;
- queue writes, priority winner, music start, fade/stop, and SFX steal;
- track loops, subroutine returns, envelope markers, and DAC state.

If the candidate uses the same driver grammar but a different chip core, the
register stream can still match exactly while PCM differs. That is desirable:
it isolates a core/configuration problem from a driver/transcription problem.

### 4.2 Secondary: canonical PCM hashes

Capture the libretro batch PCM using a pinned format: stereo signed 16-bit,
44,100 Hz, fixed core options, fixed frame window, and fixed lead-in/loop/tail
length. Hash the canonical interleaved sample bytes with SHA-256. Store the
hash with:

```text
rom_sha256
oracle core name/version/commit
region and timing mode
YM2612/YM3438 and PSG options
filter/preamp/resampler options
fixture tape or input hash
start frame and captured frame count
register-trace hash, if available
PCM/WAV hash
```

Exact PCM hashes are excellent regression tests when all those inputs are
fixed. They are not portable truth across cores, resamplers, filters, or
platforms. A waveform hash alone is also insufficient: different register
streams can converge to similar audio, and the same register stream can
produce different PCM under different chip models.

### 4.3 Scenario fixtures, not just isolated songs

The minimum useful fixture set should include:

1. Every music ID `$81..$B4`, from reset and from an already-running field
   state, with a fixed lead-in and at least one loop boundary.
2. Every regular SFX `$B5..$F7` and special SFX `$F8..$FA`, including BGM
   overlap and channel restoration.
3. DAC samples in forward, reverse, loop, and volume-controlled modes.
4. Map arrival with a changing music ID and with map ID `0` (“keep current”).
5. Game start, Basement containers, Principal confession, and the Piata guards
   scene, because their waits and music transitions are already documented.
6. Event battle entry through `EventBattleMusicData`, battle-menu SFX, attack
   animation SFX, victory, and saved-music restoration.

Acceptance should be layered:

1. queue/event timing agrees;
2. YM2612/PSG/DAC register streams agree;
3. canonical PCM hashes agree for pinned fixtures;
4. mixed scene/battle runs remain aligned after loops, SFX steals, and fades.

That ordering prevents “the WAV sounds close” from masking a broken driver.

---

## Source and external references

### Repository authority

- [`docs/source-notes/README.md`](../source-notes/README.md) — retail SHA, clone provenance, and
  byte/signature policy.
- [`docs/battle/BATTLE_SCOUT.md`](../battle/BATTLE_SCOUT.md) — explicit label-versus-generated-
  comment drift warning.
- [`docs/RUNTIME_DESIGN.md`](../RUNTIME_DESIGN.md) — runtime-pack provenance and
  never-commit rule.
- [`reference/ps4disasm/sound/ps4.sound_driver.asm`](../../reference/ps4disasm/sound/ps4.sound_driver.asm)
  — 68k driver, parser, dispatch, pointers, music, and SFX records.
- [`reference/ps4disasm/sound/ps4.dac_driver.asm`](../../reference/ps4disasm/sound/ps4.dac_driver.asm)
  — embedded Z80 DAC program and sample table.
- [`reference/ps4disasm/sound/DefDrv.txt`](../../reference/ps4disasm/sound/DefDrv.txt),
  [`DefCFlag.txt`](../../reference/ps4disasm/sound/DefCFlag.txt),
  [`Pointers.txt`](../../reference/ps4disasm/sound/Pointers.txt), and
  [`Notes.txt`](../../reference/ps4disasm/sound/Notes.txt) — driver definition,
  pointer map, and DAC record notes.

### External orientation and tooling references

- [Genesis Plus GX libretro documentation](https://docs.libretro.com/library/genesis_plus_gx/)
  and [Genesis Plus GX source](https://github.com/ekeeke/Genesis-Plus-GX).
- [Libretro core-development audio callbacks](https://docs.libretro.com/development/cores/developing-cores/).
- [Nuked OPN2](https://github.com/nukeykt/Nuked-OPN2) — accuracy-oriented
  YM3438/OPN2 reference core.
- [VGM specification](https://vgmrips.net/wiki/VGM_Specification) — timed
  register-write/log format.
- [SMPS driver research index](https://vgmpf.com/Wiki/index.php?title=Mega_Drive%2FGenesis_Sound_Driver_List)
  and [Sega sound-document discussion](https://forums.sonicretro.org/index.php?threads%2Fsega-of-japan-sound-documents-and-smps-source-code.39425%2F=).
- Rust/tooling survey: [`renuked`](https://docs.rs/renuked/latest/renuked/),
  [`game-music-emu`](https://docs.rs/game-music-emu/0.3.0/features),
  [`megadrive-sys`](https://docs.rs/megadrive-sys/latest/megadrive_sys/fm/index.html),
  [`soundlog`](https://docs.rs/soundlog/latest/soundlog/), and
  [Moa](https://github.com/transistorfet/moa).
