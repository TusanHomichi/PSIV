# Runner log: the Motavia arc (C1 to C3)

Runs C1 to C3, the Aiedo arrival to the Zio Fort and Nurvus
(chapters 14 to 29 of the route), moved out of the [runner log](RUNNER_LOG.md)
on 2026-10-04 so that file stays under the size rule. The halts these runs
met (H13 to H22) and the later receipts stay in the main log; links below point
there. The Zelan arc is [RUNNER_LOG_ZELAN.md](RUNNER_LOG_ZELAN.md).

## C1 runs: Aiedo to the Zio Fort

Base revision: `8c19769` (main after PR #57), lane C1 runner code on top.
Release builds. The branch is `worktree-agent-a03b13906c10bc1e3`. Evidence is
under the git-ignored `build/c1/` of the lane's worktree; the commands below
regenerate it.

| Run | Command | Result |
| --- | --- | --- |
| C1-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0 --tape build/c1/run-0/run.tape --report build/c1/run-0/report.json` (the route as C1 found it) | completed, exit 0, 168,843 frames, digest `5e80a50e50dad15c`: identical to R1-5 |
| C1-1 | the same route plus a scratch chapter at the Aiedo supermarket inn counter (50,32), `rest_inn`, `expect event:0x46` | halted at `rest_inn`: `the counter refused: "Aiedo rest event pending."` ([H13](RUNNER_LOG.md#h13-the-aiedo-inn-returns-aiedoeventpending-39)); `build/c1/evidence/h-inn-39-report.json` |
| C1-2 | `--save-dir build/c1/s1`, scratch chapters: Chaz's house, then `go_to_map 130` with `attack_all` | halted at the passageway: `unsupported ability 18 for fighter 7` (map `$81`, formation 211, frame 171,935); re-recorded as `build/c1/evidence/h15-fusion-report.json` ([H15](RUNNER_LOG.md#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)) |
| C1-3 | the same with `run_unless_boss` through the passageway and the fort, no training | the passageway passed; the fort's F1 killed a level 8 to 11 party (`lost_battle`, map `$84`), and a variant halted on ability 33 (`build/c1/evidence/h14-untrained-report.json`): [H14](RUNNER_LOG.md#h14-the-fort-needs-a-trained-party-and-a-rest-that-is-not-the-aiedo-inn) |
| C1-4 | `aiedo-training` (patrol until level 10, then 12), passage and approach chapters with `attack_all` in the fort | halted on ability 33 (FIREBREATH, `build/c1/evidence/h16a-airslash-report.json`) and, in an earlier draw, on ability 45 ([H15](RUNNER_LOG.md#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)); with `run_unless_boss` the same walk completes |
| C1-5 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-5 --tape build/c1/run-5/run.tape --report build/c1/run-5/report.json` | **completed**, exit 0. 457,356 frames, digest `96d2835a8633def8`, tape sha256 `53adf11f092bb99982c299aa2620b75a4078f6a4fe3ba69f0adae0a3d749bd57` |
| C1-6 | the same command with `run-6` | **completed**, exit 0, identical frames, digest and tape sha256 (and the same Zio Fort chapter save, `71e15b109ef788ed4f215fa76409d56fd43b4b4d370f32b8e85e9fbf127a20a3`); a third run (`run-7`) printed the same tape sha256. `psiv-campaign replay build/c1/run-5/run.tape` reproduces the digest |
| C1-7 | the full route plus a scratch chapter `talk npc 0` at Juza (`build/c1/run-4/`) | halted at the talk: `unsupported ability 71 for fighter 6` (Juza's ZAN), frame 458,343; `build/c1/run-4/halt-juza.json` ([H16](RUNNER_LOG.md#h16-juzas-battle-rolls-zan-and-forceflash-which-the-engine-does-not-run)). The earlier draw halted on ability 86 |

C1-5 per new chapter (frames, battles, party at the chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| aiedo-chaz-house | 1,130 | 0 | Gryz L8 84/84, Alys L8 61/61, Chaz L7 62/62, Hahn L7 51/51, Rika L3 47/47 |
| aiedo-training | 281,447 | 171 | Gryz L13 121/121, Alys L12 92/92, Chaz L12 93/93, Hahn L12 81/81, Rika L12 105/105 |
| passage-to-zio-fort | 3,144 | 6 | the same, all full |
| zio-fort-approach | 2,792 | 5 | the same, all full; standing at map `$87` (32,21), purse 21,832 meseta |

Not exercised by these chapters: scenes 27 (blocked by H13) and 31 to 39, which
all lie past Juza. Scenes 28 and 29 (Chaz's house) are exercised and assert
their temp flag `$18`.

## C2 runs: Juza to Zio

Base revision: `0c6ae8b` (main after PR #63, which carries lane a1-damage's
ZAN, FORCEFLASH, FIREBREATH and CORRSION), lane C2 runner code on top. Release
builds. The branch is `worktree-agent-a27a7d1adcde1d92a`. Evidence is under the
git-ignored `build/c1/` of the lane's worktree (`evidence/` holds the halt
reports); the commands regenerate it.

"Experimental engine" below means `rust/psiv-core` with the patches of
[H17](RUNNER_LOG.md#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree)
(four lines) and, where said, a stub scene for `$30`
([H18](RUNNER_LOG.md#h18-event_ziofortbarrier-30-is-not-transcribed)). They exist to see what
lies behind a defect; they are not committed and no route step depends on them
except where stated.

| Run | Command (`--save-dir build/c1/run-N --tape build/c1/run-N/run.tape`) | Result |
| --- | --- | --- |
| C2-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json` (the route as C1 left it, run-1) | **completed**, exit 0, 457,356 frames, digest `96d2835a8633def8`: identical to C1-5 |
| C2-1 | a chapter `zio-fort-juza` (`talk npc 0` at Juza, `fight_scripted`), `--from-chapter zio-fort-juza` | halted at the talk: `lost_battle`. Event battle 3 killed a level 12 to 13 party in four rounds with the default policy (everyone attacks); after the third round every member was under half HP |
| C2-2 | the same after a shopping chapter at Aiedo's weapon shop (Crimson sword, knife, two Saber claws, three mails, a helm: 21,500 of 21,832 meseta) | still `lost_battle`: the party's damage is an estimated 330 a round against his 1,523 HP, and his group techniques take about 50 from everyone a round |
| C2-3 | the boss policy `fight_to_win` (strongest estimated action per member, a cure that covers the damage) on the same fight | won Juza at level 12 with Alys down and Gryz as low as 4 of 121 HP: no margin, and a `--from-chapter` run of the same fight (a different draw) lost; the training target was raised (below) |
| C2-4 | `--from-chapter zio-fort-demi` | the Demi rescue and event battle 4 passed; `Cutscene_AlysWounded` faulted: [H17](RUNNER_LOG.md#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree) |
| C2-5 | experimental engine (H17 patch), `--from-chapter motavia-machine-center` and on | the Machine Center appears ($43), the Land Rover is won; `go_to_map 183` stuck on the door: [H19](RUNNER_LOG.md#h19-story-flags-and-scene-tile-writes-do-not-reach-the-live-maps-collision) |
| C2-6 | `plan --from-map 0 --from-cell 109,153 --to-map 142` | no plan on foot (64 positions): the tower lies across the sand; `--vehicle 1` plans it (126 steps to the tower's door). The runner learned to drive: [Mounted walks](CAMPAIGN_RUNNER.md#the-runner) |
| C2-7 | `ladea-tower-psycho-wand` with the default policy, level 12 to 13 party plus Rune at level 18 | `lost_battle` to Gy-Laguiah (event battle 5): a member falls about every round |
| C2-8 | a `krup-training` patrol (foot ground south of Krup, the Krup inn as refuge) before the Machine Center, a cure that casts enough, and the boss policy | training levels 18, 19 and 20 pass the tower and Gy-Laguiah; levels 16, 17 and 21 halt on EVIL EYE in the tower ([H21](RUNNER_LOG.md#h21-evil-eye-ability-76-in-the-ladea-tower-58)) before reaching him. Target 19 kept |
| C2-F1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-5 --tape build/c1/run-5/run.tape --report build/c1/run-5/report.json` (the committed engine) | **halted**, exit 2, chapter `zio-fort-demi` objective 2: `scene_fault: dialogue fault: scene resume has no saved cursor`. 1,023,474 frames, digest `49f5c47df7971428`, tape sha256 `83f96cf4287581717d22fc8272cb65248e1d4d2493e91b0b32fa6a0ccc3d75b6`; `build/c1/evidence/h17-halt-report.json` ([H17](RUNNER_LOG.md#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree)) |
| C2-F2 | the same with `run-6` | identical frames, digest and tape sha256. `psiv-campaign replay build/c1/run-6/run.tape` replays 1,023,474 frames and reproduces the digest |
| C2-E1 | experimental engine (H17 patch), the same command with `run-7`, `--report build/c1/evidence/h18-halt-report.json` | **halted**, chapter `zio-fort-barrier` objective 2 (`talk`): `scene_fault: dialogue event 0x30 has no transcribed scene`. 1,723,422 frames, digest `dd1a7757ab42acf6`, tape sha256 `3c3c8df72b58732a655be40fd15a4495459e93b1f553d3a76766c84507b1bcdd` ([H18](RUNNER_LOG.md#h18-event_ziofortbarrier-30-is-not-transcribed)) |
| C2-E2 | the same with `run-8` | identical |
| C2-E3 | experimental engine (H17 patch and a stub for `$30` that runs dialogue `$44` and sets `$64`), `run-3` | completes, exit 0, 1,723,576 frames, digest `e77a7b9cf1f5fa37`, tape sha256 `e566844935aba65f75a0f6084f6d6e338636617dab336b40744c8073012b3c3a` |
| C2-E4 | the same with `run-4` | identical frames, digest and tape sha256; `psiv-campaign replay build/c1/run-4/run.tape` reproduces the digest |
| F1-A | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/f1/run --tape build/f1/run/run.tape --report build/f1/run/report.json` (lane F1: H17, H18 and H19 transcribed, no patch, no route edit) | **completed**, exit 0, 1,724,673 frames, digest `6381311acb90c052`, tape sha256 `eb12090d6400ae95a38977ba917f457ca46382845707f060fe08d84cc4d826df`; route file sha256 `234b50fc832f3abce6762ec8212673ecaa1c7e936f2cebd069ab2ff58a8716df`, unchanged from the base. 251 frames longer than C2-E3: the real barrier scene runs where the stub skipped it, and the route's two H19 workarounds (Juza's east-door round trip, the Krup detour) are still performed |
| F2-A | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/f2/run --tape build/f2/run/run.tape --report build/f2/run/report.json` (lane F2 route edit: `dismount` removed; connected route uses initial Getting Land Rover scene, not Aiedo inn or ITEM reboarding) | **completed**, exit 0, 1,726,539 frames, digest `bfd4d40aa048623c`, tape sha256 `96acff7606db27823b39775419c4a961c7dedd8bc807f2719bb4f4e5479e5f8c`; route file sha256 `c51285678363da34431eb9035b754a478cd6a493c8bb77244fdd0ea7e86401c0` (one route edit: `ladea-tower-rune`'s `dismount` dropped). The old full-route baseline with that objective completed; the candidate route completes without it, while the explicit-dismount negative control halts because the map load already parked the vehicle. This is parking/load evidence, not reboarding evidence. |
| F2-R | `flock -x /home/peter/PSIV/build/continuation-heavy.lock` around release build, `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/f2-repair-20261003/route-final/saves --tape build/f2-repair-20261003/route-final/run.tape --report build/f2-repair-20261003/route-final/report.json`, then replay of that tape; source/test candidate in [F2-R receipt](RUNNER_LOG.md#f2-r-repair-receipt-2026-10-03) | **completed**, both commands exit 0; 27 chapters, 1,726,539 frames, digest `bfd4d40aa048623c`, tape SHA-256 `96acff7606db27823b39775419c4a961c7dedd8bc807f2719bb4f4e5479e5f8c`. Raw run/replay/report under ignored `build/f2-repair-20261003/route-final/`. Same caveat as F2-A: no special inn or ITEM reboarding in this connected route. |

Per new chapter on the experimental engine (C2-E3; frames, battles, party at the
chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| aiedo-training (level 17) | 832,603 | 615 | Gryz L18 160/160, Alys L17 129/129, Chaz L18 123/123, Hahn L17 100/100, Rika L19 143/143 |
| aiedo-shopping | 1,116 | 0 | the same; purse 332 meseta |
| passage-to-zio-fort | 2,888 | 4 | the same, all full |
| zio-fort-approach | 2,904 | 6 | the same, all full |
| zio-fort-juza | 3,134 | 2 | Gryz L19 146/168, Alys L17 120/129, Chaz L18 118/123, Hahn L17 76/100, Rika L19 135/143 |
| zio-fort-demi | 18,422 | 3 | Gryz L19 168/168, Chaz L18 123/123, Rika L19 143/143, Demi L12 96/96 (Krup Inn F1 `$3F` (33,35)) |
| krup-training (level 19) | 661,909 | 803 | Gryz L23 195/195, Chaz L22 146/146, Rika L24 164/164, Demi L19 144/144 |
| motavia-machine-center | 1,313 | 2 | the same |
| machine-center-control-key | 4,949 | 1 | the same; Motavia (114,177), Vehicle 1 |
| ladea-tower-rune | 5,452 | 6 | + Rune L23 115/115 |
| ladea-tower-psycho-wand | 14,415 | 3 | Gryz L23 195/195, Chaz L23 150/150, Rika L24 164/164, Demi L19 144/144, Rune L23 115/115; Krup `$39` (20,15), PSYCO-WAND in the pack |
| zio-fort-barrier (stub) | 4,498 | 6 | the same; Zio Fort `$82` (47,56), `$64` set |

Per chapter on the fixed engine (F1-A; the chapters C2-E3's table above already
lists are unchanged frame for frame):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| zio-fort-barrier | 5,595 | 6 | Gryz L23 195/195, Chaz L23 150/150, Rika L24 164/164, Demi L19 144/144, Rune L23 115/115; the courtyard open, `$64` set |

Draws matter. Training at levels 16, 17 and 21 halts in the tower on
EVIL EYE, and the Gy-Laguiah fight is close at level 16: the passes above are
those of the route as committed, and a change upstream shifts the draws (the
same as H15's note).

## C3 runs: the Zio Fort barrier to Zio

Base revision: `5c62765` (main after PR #69, lane F1: H17 to H19 fixed), lane C3
runner code on top. Release builds. Evidence is under the git-ignored
`build/c1/` of the lane's worktree (`evidence/` holds the halt reports); the
commands regenerate it. "Probe" below means a throwaway change to `rust/psiv-core`
made to see what lies behind a defect and reverted before the commit; no route
step depends on one.

| Run | Command | Result |
| --- | --- | --- |
| C3-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0 --tape build/c1/run-0/run.tape --report build/c1/run-0/report.json` (the route as F1 left it) | **completed**, exit 0, 1,724,673 frames, digest `6381311acb90c052`: identical to F1-A |
| C3-1 | `nurvus-descent` authored in four steps, each with `--from-chapter nurvus-descent --save-dir build/c1/iter` | four route claims, [N1 to N4](#n1-to-n4-route-claims-in-nurvus) below; the chapter then completes in 4,592 frames |
| C3-2 | a chapter `nurvus-zio` (`go_to_map 211`, `go_to` the corridor's row 30, `fight_scripted`), the committed engine, `--from-chapter nurvus-zio` | **halted** at the `go_to`, frame 739: `scene_fault: dialogue fault: scene resume has no saved cursor` ([H22](RUNNER_LOG.md#h22-zio-at-nurvus-resumes-a-dialogue-instead-of-running-entry-0b-and-the-zio-phase-counter-is-unmodelled), part 1) |
| C3-3 | probe: `Event_ZioNurvus`'s op 11 as `RunDialogue Entry(0x0B)` (one op), `--from-chapter nurvus-zio` | the scene plays and event battle 6 starts; the new `psycho_wand_then_win` policy opens round 1 with PSYCO-WAND (trace: `battle actor 1 Item { name: "PSYCO-WAND", target: None }`, the rest attack); **halted** at frame 1,441: `unsupported_ability: engine emitted unsupported ability 84 for fighter 6` (H22, part 2) |
| C3-4 | the same probe, the full route from New Game, `--save-dir build/c1/probe-d1` | **halted** the same way, frame 1,731,142, digest `42ab96d0933ea942`: `build/c1/evidence/c3-zio-black-wave-report.json` |
| C3-5 | probe on top of C3-3: enemies 139 and 140 forced to the plain attack (ability 0) so the engine runs them, `--from-chapter nurvus-zio` | Zio's fight runs 5,858 frames and the party (Gryz L23, Chaz L23, Rika L24, Demi L19, Rune L23) is **defeated**: `build/c1/evidence/c3-probe-zio-lost-battle-report.json`. Not a fidelity claim: ability 0 is not what Zio does, and the cartridge's Zio2 opens with a barrier and casts CORRSION, HEWN and BLACK WAVE |
| C3-F1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-1 --tape build/c1/run-1/run.tape --report build/c1/run-1/report.json` (the committed engine) | **halted**, exit 2, chapter `nurvus-zio` objective 1 (`go_to`), frame 1,730,441: `scene_fault: dialogue event 0x0 has no transcribed scene`; digest `6eb19b166ed24d2f`, tape sha256 `e3cff60eaf31dacac06bfeb8819d983ad3a848da042d37108a5c73cc9d121402`; report `build/c1/evidence/c3-zio-nurvus-resume-report.json` |
| C3-F2 | the same with `run-2` | identical frames, digest and tape sha256; `psiv-campaign replay build/c1/run-2/run.tape` replays 1,730,441 frames and reproduces the digest |

| F3-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/f3/run --tape build/f3/run/run.tape --report build/f3/run/report.json` (release, the F3 engine) | **halted**, chapter `nurvus-zio` objective 1 (`go_to`), frame 1,727,476, digest `1d7c651992e0f631`, tape sha256 `1d123c9bc754ff70633d0b8057b49bc566f616dd3af6d9bc55ecd4ec52b08df5`: `lost_battle -- the party was defeated`. `psiv-campaign replay` reproduces 1,727,476 frames and the digest. Event battle 6 starts (`Started { priority: Ambush }`), Zio acts, the party is wiped; report `build/f3/evidence/committed-route-report.json` |

The full run's halt differs from C3-2's because the full run has a dialogue
cursor to resume (the Zio Fort barrier's, from `tree 13` entry `$45`) and the
`--from-chapter` run, whose session was loaded from a save, has none: the resume
reads `tree 13` entry 70 (`$46`) and that entry "fires event 0x0". Both are the
same defect.

Per chapter, full run C3-F1 (the chapters before are unchanged frame for frame):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| nurvus-descent | 5,028 | 7 | Gryz L23 155/195, Chaz L23 150/150, Rika L24 164/164, Demi L19 144/144, Rune L23 115/115; Nurvus B4 `$D2` |

### N1 to N4: route claims in Nurvus

Four runner or route problems, fixed in the route while authoring
`nurvus-descent`. The H18 forecast was right about the elevator doors and the
stairs and wrong about the route between them.

- **N1.** `go_to_map 206` from the Part3 elevator, after `interact`: `unreachable`.
  The door cells open as collision 1 and the planner only crosses a warp it is
  told to take, so the objective names it (`via_warp 2`), as every BioPlant
  elevator does.
- **N2.** The H18 forecast has B3 `$D0`'s stairs in a region the elevators do not
  reach "first". Flooding each map's collision (doors treated as walls until
  opened) gives the whole chain, and B1's ground floor does not join its east door
  to the arrival: arrival (46,12) reaches only B1's west half. The route is B1
  west door (22..23,11) to B3 region 1; B3's door (14..15,67) back to B1's
  south-west room; B1 warp 4 (28,73) into the B1 tunnel `$D1`; the tunnel's east
  warp (48,32) to B1's south-east room; up that room's corridor (72..73,34..57) to
  the east door (70..71,11); B3 region 2; the door (78..79,11) to B5 `$D5`; B5's
  other elevator (30..31,36) to B3 (78,23), which is the stairs' region. Every
  hop is a warp of the pack's own records and every door an `interact` on its
  type-2 interaction area.
- **N3.** The tunnel's two warps both return to B1: `go_to_map 206` from the
  tunnel takes warp 0 (the way it came); the route names `via_warp 1`.
- **N4.** B3's stairs (76..77,79) are a closed door like the others: `interact`
  at (76,80) first, or `go_to_map 210` halts `unreachable: no walk from (78,23)
  reaches warp 5`.
