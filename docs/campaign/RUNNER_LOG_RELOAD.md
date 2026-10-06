# Runner log: the cutscene-return reload (S8)

Runs of the 50-chapter route after `FieldRoutine_Cutscene`'s zero return became a
modelled field reload ([`FIELD_RELOAD.md`](../scenes/FIELD_RELOAD.md)). Evidence
files are under the git-ignored `build/s8-route/` and `build/s8-route-b/`;
the commands regenerate them. Base `1b01ee7` (P88 plus the wanderer fix), which
halted in `esper-mansion` with "vehicle is 2, expected 0".

## S8 runs

```
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/s8-route --tape build/s8-route/run.tape --report build/s8-route/report.json
```

Two full runs from New Game: exit 0, all 50 chapters, **3,852,511 frames**,
digest `29c81ae27664b15b`, tapes byte-identical (SHA-256 `a66c5830…0131d`);
`psiv-campaign replay build/s8-route/run.tape` replays 3,852,511 frames to the same
digest. `esper-mansion` passes with the Ice Digger parked at its door.

### Why the digest changed

Every cutscene that returns zero now spends the cartridge's frames before
control (`FIELD_RELOAD.md`: the principal's return alone is 55), and the cartridge's
random draws are one stream, so every later encounter moved. Three more edits
changed the route's own path; each is a fix, and none is a seed hunt:

| Change | Why |
|---|---|
| the reload (state, frames, flag spend) | the defect this lane exists for; `Cutscene_MeetingKyra`'s bit 0 is now spent by the load that reads it |
| `bioplant_survival` runs from random encounters; `train_with_inn` retreats once a member has fallen or one is under a third of their HP (`policy.rs`) | with the moved stream the party was wiped in `bioplant-rika` and, after that, in the `aiedo-training` patrol: both policies resolved to `default` and survived only on the old stream. Not a mechanism defect, a runner one (H45 below) |
| `Cutscene_ZioDefeated` sets `$66` where `Event_MotaSpaceportAppearing` does, before the second `RefreshMap` (`ps4.asm:144825-144826`) | the port set it after, so that load rebuilt Motavia without the spaceport's door (H46 below) |

### H45: wiped on a moved random stream (runner)

With the reload and nothing else the run halted in `bioplant-rika` (objective 0,
`lost_battle`; the party entered the chapter at 19/61, 0/53, 0/45) and, with
`run_unless_boss` forced on the three BioPlant chapters, in `aiedo-training`
(`patrol`, formation 35, a 4,700-frame fight). The base only passed them because
its stream happened to be kind. Class fix: the two policies above, a route that
survives on one stream being no route.

### H46: the spaceport's door missing after `Cutscene_ZioDefeated` (scene)

The run then halted in `mota-spaceport` objective 1 (`stuck`, the party on
Motavia (53,91), inside warp 22's footprint at (52..53, 90..91)): the walk had
no step to take. The pack's `$66` page hook gives those four cells collision
type 1; the scene's `WriteMapChunks` wrote the chunk, but its second `RefreshMap`
rebuilt the map with `$66` still clear, and the base had only been passing
because a random battle's reload rebuilt it after the flag was set. The
subroutine sets the flag as its last act, before that load. The scene
([`39_ZioDefeated.md`](../scenes/39_ZioDefeated.md)) now does.

### Chapters whose objective asserted the old state

None. No `expect` held only because the reload was missing.
