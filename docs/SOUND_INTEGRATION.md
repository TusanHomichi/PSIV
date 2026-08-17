# Sound integration

Implemented 2026-08-16. This closes the runtime half of the sound slice; the
pack remains generated and gitignored under `runtime-pack/`.

## Runtime path

`psiv-data` loads `runtime-pack/sound/` as typed records. `psiv-godot` resolves
the records into `psiv_sound::SoundBank`, preserving the complete raw record
and its record-relative track start offset. `psiv-sound` then runs the PSIV
interpreter against:

- the extracted FM voices and exact volume-envelope byte streams;
- FM, PSG, and shared-subroutine track bytes;
- the extracted DAC bank/sample bytes;
- the Nuked-OPN2 YM2612 and SN76489 PSG cores.

DAC playback is live. `EE` sample commands select the extracted sample bank;
the driver renders PCM through YM `$2A`, enables DAC mode through `$2B`, and
applies the extracted direction, loop, pan, volume, and pitch controls. The
register log includes the DAC control/data writes, not just synthesized FM/PSG
events.

## Binding provenance

No sound binding was invented and no `psiv_tools` change was needed. The map
pack already emits `music.id`, `music.symbol`, and `music.changes_music` from
the ROM-derived map record. Piata is map `$013`; its pack binding is music
`$84`, `MotabiaTown`.

The music ID table is the retail `MusicPtrs` table at `$D1C40`, with IDs
`$81..$B4`. The corresponding symbols and dispatch rules are documented in
`docs/SOUND_SCOUT.md` and sourced from:

- `reference/ps4disasm/sound/ps4.sound_driver.asm` for pointer records and
  `PlaySoundID` dispatch;
- `reference/ps4disasm/ps4.constants.asm` for named music/SFX IDs;
- `reference/ps4disasm/ps4.asm` for map/event/battle call sites.

## Trigger coverage

| Scout dispatch | Runtime route | Status |
|---|---|---|
| Map arrival `music.id`; zero means keep current | `Field::ready` and `MapChanged` call `play_map_music` | Wired |
| Normal field battle `$8F` `MeetThemHeadOn` | `EncounterRolled`; debug random battle uses the same ID | Wired |
| Event battle `EventBattleMusicData` | `SceneBattleStarted`, all 27 ROM table entries preserved | Wired |
| Victory `$8B` `Winners` | `service_battle_finish` on `Outcome::Victory` | Wired |
| Cursor `$F2` `MovingCursor` | Modal Godot directional actions | Wired for current field/battle/dialogue/shop/camp UI |
| Selection `$F3` `Selection` | Modal Godot accept/cancel actions | Wired for current field/battle/dialogue/shop/camp UI |
| Vehicle battle `$96` `CyberneticCarnival` | No vehicle-battle runtime event exists yet | Silent |
| Attack/effect/cutscene SFX call sites | No complete runtime dispatch surface exists yet | Silent outside the UI routes above |

The current UI hooks intentionally cover the live Godot input surfaces, not
every retail `Sound_Index` write in the scout. SFX sequence-internal `EB`
triggers and the retail battle animation/effect SFX still need their owning
runtime events before they can be wired honestly.

`PSIV_DEBUG_TRACK=<id>` starts any extracted record through the Godot output
path. `PSIV_DEBUG_BATTLE=0x88` starts the oracle battle after boot; when both
selectors are set, the debug track starts first and the battle dispatch then
selects its event music, matching the driver's single music queue rather than
mixing two BGM records.

## Verification

The final local checks were:

```text
cargo fmt --all -- --check
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --workspace
PYTHONPATH=. python -m unittest discover -s tests -v
python -m psiv_tools pack "Phantasy Star IV (USA).md" <tmp>
python tools/pack_diff.py runtime-pack <tmp> --quiet
```

Results: Rust workspace green, clippy clean, 892 Python tests green, and the
full generated pack comparison was `2946/2946` identical with zero differing
files. The real Piata track's first three driver ticks have a deterministic
register log and include YM DAC `$2A` data plus `$2B` enable writes.

Headless Godot boots exit successfully and demonstrate normal Piata routing,
debug-track routing, and combined debug-battle routing. Godot 4.7.1 reports
one engine-owned `AudioStreamGeneratorPlayback` ObjectDB warning during
process teardown; the safe `stop`/stream-detach path is in place, but the
headless engine does not retire that playback before its final leak audit.
