# Runner log

The durable record of campaign-runner runs: each run's command, base revision,
how far it got, and every halt with its diagnosis. The runner and its report
format are described in [the campaign runner plan](CAMPAIGN_RUNNER.md#the-runner);
this file holds what the runs found. Evidence files live under the git-ignored
`build/campaign/` of the checkout that ran them, so a fresh clone does not hold
them; the commands below regenerate them.

A run that halts on a port defect keeps its report here and a diagnosis with
`file:line` and the cartridge evidence. The orchestrator turns a diagnosis into
a fix lane or an issue; the runner never fixes a port defect and never adds a
shortcut, a state edit or a skip to get past one.

## Current state

**#70 isolated headless candidate (2026-10-03, base `82eed831`, gameplay
source `6fb1a70`):** Session skips the scripted enemy-only ambush command round.
After ordinary Krup training to level 23 and menu-selected existing cures,
the 29-chapter New Game route defeats Zio and settles
`Cutscene_ZioDefeated` on Motavia. Two frozen runs have identical tapes,
digests and pad-SAVE bytes; both tapes replay, and a fresh Session loads the final
slot with Chaz/Rika/Rune alive and `$65/$68/$66/$61` set
([receipt](#70-frozen-headless-acceptance)). The earlier winning
**chapter-resume** trial and losing runs below are preserved. The combined
Godot/native milestone, generic Godot tape replay and a Zio event-battle
oracle fixture are not claimed.

**F3 (2026-10-02, base `2272c75`): H22 is fixed and the route now fights Zio.** It halts in
`nurvus-zio` with `lost_battle`, a balance-and-session result, not a port fault
([F3 results](#f3-results-after-the-h22-fixes)).

**C3 (2026-10-02, base `5c62765`): the route has 29 chapters and halts in the
last, `nurvus-zio`, on a port defect with no legitimate alternative
([H22](#h22-zio-at-nurvus-resumes-a-dialogue-instead-of-running-entry-0b-and-the-zio-phase-counter-is-unmodelled)).**
Everything before it passes from New Game: `nurvus-descent` (Zio Fort to
Nurvus B4, six elevator doors, the B1 tunnel, B5 and the stairs) runs in 5,028 frames,
and the halt is at Zio's trigger, frame 1,730,441 (run C3-F1, below).

`routes/main.json` had 27 chapters before C3: New Game, the post-Rika north bank, Aiedo,
the Zio Fort's Juza room (C1) and, since lane C2, Juza's battle, the first
Motavia arc's Demi rescue, the Machine Center and the Land Rover, the Ladea
Tower with Rune and the Psycho Wand, and the walk to the Zio Fort's barrier
(scenes 31 to 37, `zio-fort-barrier` last). **The route completes.** Lane F1 fixed the four defects the runs below found
on the critical path — H17 (the two revision-gated `DialogueTree6` loads), H18
(`Event_ZioFortBarrier` `$30`) and H19 (live map tile writes) — and the 27
chapters now run end to end: **run F1-A, exit 0, 1,724,673 frames, digest
`6381311acb90c052`**, tape sha256
`eb12090d6400ae95a38977ba917f457ca46382845707f060fe08d84cc4d826df`. The
workarounds H19's report added to the route (Juza's east-door round trip, the
Krup detour before the Machine Center) are now redundant; they still work and
are left in place.

The engine these runs were committed against carried neither fix: H17 halted
the route at chapter 20 and, with a four-line experimental patch for H17, H18
halted it at `zio-fort-barrier`'s talk. Both are transcribed now, and F1-A
carries no patch.

Two more port defects were met and are not blockers here:
[H20](#h20-the-vehicle-is-not-parked-at-a-map-load-and-boarding-is-missing) (the
Land Rover) and [H21](#h21-evil-eye-ability-76-in-the-ladea-tower-58) (an
unsupported enemy ability in the tower, issue #58).
[H19](#h19-story-flags-and-scene-tile-writes-do-not-reach-the-live-maps-collision)
is fixed. The run is a traversal
proof, not a balance proof (see "Not claimed").

H16 (Juza's ZAN and FORCEFLASH) is resolved: lane a1-damage implemented them
and the route fights Juza. H15 still lists FUSION and DEBAN open; C2's walks
ran from every encounter on the way, as C1 did. The Aiedo inn defect
[#39](https://github.com/TusanHomichi/PSIV/issues/39) does not block the story:
[H13](#h13-the-aiedo-inn-returns-aiedoeventpending-39).

The one port defect the first runner lane found,
[H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile), is fixed
(issue #54).

## Runs

Base revision for every run: `016cdc7` (PR #53, the battle, shop and camp
Session modes), runner code of this lane on top. Pack: the local
`runtime-pack`. All runs are release builds unless said otherwise; a debug
build gives the same tape byte for byte (checked on chapter one).

| Run | Command | Result |
| --- | --- | --- |
| R1-1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full1` (route as R0 shipped it) | academy completed (30,822 frames, 2 battles); halted in `holt` objective 0, `TriggerUnsupported { trigger: 112 }` on arriving in Mile: [H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile). Report: `build/campaign/evidence/halt-1-mile-sandworm-trigger.json` |
| R1-2 | `... --from-chapter holt` while the route and the runner were repaired | `holt` completed once Mile was left out (H1) and the `expect` cell corrected (H2); `rune-dorin` halted on the Zema inn (H3), then lost a battle in the Valley Maze (H4) until the runner cured the party between battles |
| R1-3 | `... --from-chapter alshline` | storage door object settled (H5); lost a battle in the Alshline basement until `run_unless_boss` existed (H6); then chest (H7), equipment with every slot full (H8), the BioPlant elevator doors (H9) and the Rika scene's `go_to` (H10) |
| R1-4 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full4` | **completed**, exit 0. 175,497 frames, digest `52ccbf8d9a7de61a`; `psiv-campaign replay build/campaign/full4/run.tape` reproduces the digest. A second full run (`full3`, after H11) printed the same digest |
| R1-5 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c54-route --tape build/c54-route/run.tape --report build/c54-route/report.json` (release; issue #54: sand worm transcribed, Mile restored, `basement-training` added) | **completed**, exit 0. 168,843 frames, digest `5e80a50e50dad15c`, tape sha256 `24ad3ae31b58bc574cf76f9c2e727887c69c6575b32d6588f4f1da5f64ae67d5`; `psiv-campaign replay` reproduces the digest. Before the training chapter existed the same route halted in `alshline` objective 13 (`lost_battle`, [H12](#h12-the-alshline-basement-kills-an-under-levelled-party-after-the-walk-shifted)) |

R1-4 per chapter (frames, battles, party at the chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| academy | 32,308 | 4 | Alys L7 50/53, Chaz L2 31/31, Hahn L2 27/27 |
| holt | 9,958 | 6 | Alys L7 41/53, Chaz L3 28/34, Hahn L3 28/30 |
| rune-dorin | 29,215 | 9 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 27/33, Gryz L6 66/66 |
| alshline | 8,871 | 6 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 33/33, Gryz L6 66/66 |
| zema-rescue | 14,198 | 9 | Alys L7 52/53, Chaz L5 49/49, Hahn L5 33/38, Gryz L6 66/66 |
| zema-outfit | 1,282 | 0 | Alys L7 53/53, Chaz L5 49/49, Hahn L5 38/38, Gryz L6 66/66 |
| zema-training | 53,841 | 55 | Alys L7 53/53, Chaz L6 53/53, Hahn L6 45/45, Gryz L6 66/66 |
| zema-armour | 718 | 0 | the same, all full |
| bioplant-elevators | 4,887 | 3 | Alys L7 52/53, Chaz L6 53/53, Hahn L6 44/45, Gryz L6 65/66 |
| bioplant-order | 60 | 0 | Gryz leads: Gryz, Alys, Chaz, Hahn |
| bioplant-rika | 18,763 | 2 | Gryz L7 0/76, Alys L8 0/61, Chaz L6 53/53, Hahn L6 0/45, Rika L1 39/39 |
| north-bank | 850 | 0 | Gryz L7 76/76, Alys L8 61/61, Chaz L6 53/53, Hahn L6 45/45, Rika L1 39/39 |
| aiedo | 546 | 0 | the same; standing at Aiedo (47,83) |

Evidence of R1-4 (local, under `build/campaign/`): `evidence/r1-4-full-route.tape`
(sha256 `dd5ff60f0cd2ef06ff483ecf7f55ee398cc22bed85752a7181dc9291c2e5e6c5`),
the chapter saves `full4/NN-id/slot_1.sram` (north bank
`29b717fdba5bdd1d5047025b2f6f63c7448dedf792f609a273b25512bf1897dc`, Aiedo
`866c29cd343403cb3babe77f31abdde6da7620af08fecd0272929a0141c999a2`), and the
route's own saves in `full4/route/`.

Acceptance target: after `north-bank` the party stands on Motavia `$00` at
(84,64) with Gryz, Alys, Chaz, Hahn and Rika, all alive, flag `$35` set (the
chapter's `expect` and `closing` hold), and `aiedo` arrives at map `$54`
(47,83), one warp, as R0 planned.

## C1 runs: Aiedo to the Zio Fort

Base revision: `8c19769` (main after PR #57), lane C1 runner code on top.
Release builds. The branch is `worktree-agent-a03b13906c10bc1e3`. Evidence is
under the git-ignored `build/c1/` of the lane's worktree; the commands below
regenerate it.

| Run | Command | Result |
| --- | --- | --- |
| C1-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0 --tape build/c1/run-0/run.tape --report build/c1/run-0/report.json` (the route as C1 found it) | completed, exit 0, 168,843 frames, digest `5e80a50e50dad15c`: identical to R1-5 |
| C1-1 | the same route plus a scratch chapter at the Aiedo supermarket inn counter (50,32), `rest_inn`, `expect event:0x46` | halted at `rest_inn`: `the counter refused: "Aiedo rest event pending."` ([H13](#h13-the-aiedo-inn-returns-aiedoeventpending-39)); `build/c1/evidence/h-inn-39-report.json` |
| C1-2 | `--save-dir build/c1/s1`, scratch chapters: Chaz's house, then `go_to_map 130` with `attack_all` | halted at the passageway: `unsupported ability 18 for fighter 7` (map `$81`, formation 211, frame 171,935); re-recorded as `build/c1/evidence/h15-fusion-report.json` ([H15](#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)) |
| C1-3 | the same with `run_unless_boss` through the passageway and the fort, no training | the passageway passed; the fort's F1 killed a level 8 to 11 party (`lost_battle`, map `$84`), and a variant halted on ability 33 (`build/c1/evidence/h14-untrained-report.json`): [H14](#h14-the-fort-needs-a-trained-party-and-a-rest-that-is-not-the-aiedo-inn) |
| C1-4 | `aiedo-training` (patrol until level 10, then 12), passage and approach chapters with `attack_all` in the fort | halted on ability 33 (FIREBREATH, `build/c1/evidence/h16a-airslash-report.json`) and, in an earlier draw, on ability 45 ([H15](#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)); with `run_unless_boss` the same walk completes |
| C1-5 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-5 --tape build/c1/run-5/run.tape --report build/c1/run-5/report.json` | **completed**, exit 0. 457,356 frames, digest `96d2835a8633def8`, tape sha256 `53adf11f092bb99982c299aa2620b75a4078f6a4fe3ba69f0adae0a3d749bd57` |
| C1-6 | the same command with `run-6` | **completed**, exit 0, identical frames, digest and tape sha256 (and the same Zio Fort chapter save, `71e15b109ef788ed4f215fa76409d56fd43b4b4d370f32b8e85e9fbf127a20a3`); a third run (`run-7`) printed the same tape sha256. `psiv-campaign replay build/c1/run-5/run.tape` reproduces the digest |
| C1-7 | the full route plus a scratch chapter `talk npc 0` at Juza (`build/c1/run-4/`) | halted at the talk: `unsupported ability 71 for fighter 6` (Juza's ZAN), frame 458,343; `build/c1/run-4/halt-juza.json` ([H16](#h16-juzas-battle-rolls-zan-and-forceflash-which-the-engine-does-not-run)). The earlier draw halted on ability 86 |

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
[H17](#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree)
(four lines) and, where said, a stub scene for `$30`
([H18](#h18-event_ziofortbarrier-30-is-not-transcribed)). They exist to see what
lies behind a defect; they are not committed and no route step depends on them
except where stated.

| Run | Command (`--save-dir build/c1/run-N --tape build/c1/run-N/run.tape`) | Result |
| --- | --- | --- |
| C2-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json` (the route as C1 left it, run-1) | **completed**, exit 0, 457,356 frames, digest `96d2835a8633def8`: identical to C1-5 |
| C2-1 | a chapter `zio-fort-juza` (`talk npc 0` at Juza, `fight_scripted`), `--from-chapter zio-fort-juza` | halted at the talk: `lost_battle`. Event battle 3 killed a level 12 to 13 party in four rounds with the default policy (everyone attacks); after the third round every member was under half HP |
| C2-2 | the same after a shopping chapter at Aiedo's weapon shop (Crimson sword, knife, two Saber claws, three mails, a helm: 21,500 of 21,832 meseta) | still `lost_battle`: the party's damage is an estimated 330 a round against his 1,523 HP, and his group techniques take about 50 from everyone a round |
| C2-3 | the boss policy `fight_to_win` (strongest estimated action per member, a cure that covers the damage) on the same fight | won Juza at level 12 with Alys down and Gryz as low as 4 of 121 HP: no margin, and a `--from-chapter` run of the same fight (a different draw) lost; the training target was raised (below) |
| C2-4 | `--from-chapter zio-fort-demi` | the Demi rescue and event battle 4 passed; `Cutscene_AlysWounded` faulted: [H17](#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree) |
| C2-5 | experimental engine (H17 patch), `--from-chapter motavia-machine-center` and on | the Machine Center appears ($43), the Land Rover is won; `go_to_map 183` stuck on the door: [H19](#h19-story-flags-and-scene-tile-writes-do-not-reach-the-live-maps-collision) |
| C2-6 | `plan --from-map 0 --from-cell 109,153 --to-map 142` | no plan on foot (64 positions): the tower lies across the sand; `--vehicle 1` plans it (126 steps to the tower's door). The runner learned to drive: [Mounted walks](CAMPAIGN_RUNNER.md#the-runner) |
| C2-7 | `ladea-tower-psycho-wand` with the default policy, level 12 to 13 party plus Rune at level 18 | `lost_battle` to Gy-Laguiah (event battle 5): a member falls about every round |
| C2-8 | a `krup-training` patrol (foot ground south of Krup, the Krup inn as refuge) before the Machine Center, a cure that casts enough, and the boss policy | training levels 18, 19 and 20 pass the tower and Gy-Laguiah; levels 16, 17 and 21 halt on EVIL EYE in the tower ([H21](#h21-evil-eye-ability-76-in-the-ladea-tower-58)) before reaching him. Target 19 kept |
| C2-F1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-5 --tape build/c1/run-5/run.tape --report build/c1/run-5/report.json` (the committed engine) | **halted**, exit 2, chapter `zio-fort-demi` objective 2: `scene_fault: dialogue fault: scene resume has no saved cursor`. 1,023,474 frames, digest `49f5c47df7971428`, tape sha256 `83f96cf4287581717d22fc8272cb65248e1d4d2493e91b0b32fa6a0ccc3d75b6`; `build/c1/evidence/h17-halt-report.json` ([H17](#h17-cutscene_alyswounded-and-cutscene_psycowand-read-the-wrong-dialogue-tree)) |
| C2-F2 | the same with `run-6` | identical frames, digest and tape sha256. `psiv-campaign replay build/c1/run-6/run.tape` replays 1,023,474 frames and reproduces the digest |
| C2-E1 | experimental engine (H17 patch), the same command with `run-7`, `--report build/c1/evidence/h18-halt-report.json` | **halted**, chapter `zio-fort-barrier` objective 2 (`talk`): `scene_fault: dialogue event 0x30 has no transcribed scene`. 1,723,422 frames, digest `dd1a7757ab42acf6`, tape sha256 `3c3c8df72b58732a655be40fd15a4495459e93b1f553d3a76766c84507b1bcdd` ([H18](#h18-event_ziofortbarrier-30-is-not-transcribed)) |
| C2-E2 | the same with `run-8` | identical |
| C2-E3 | experimental engine (H17 patch and a stub for `$30` that runs dialogue `$44` and sets `$64`), `run-3` | completes, exit 0, 1,723,576 frames, digest `e77a7b9cf1f5fa37`, tape sha256 `e566844935aba65f75a0f6084f6d6e338636617dab336b40744c8073012b3c3a` |
| C2-E4 | the same with `run-4` | identical frames, digest and tape sha256; `psiv-campaign replay build/c1/run-4/run.tape` reproduces the digest |
| F1-A | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/f1/run --tape build/f1/run/run.tape --report build/f1/run/report.json` (lane F1: H17, H18 and H19 transcribed, no patch, no route edit) | **completed**, exit 0, 1,724,673 frames, digest `6381311acb90c052`, tape sha256 `eb12090d6400ae95a38977ba917f457ca46382845707f060fe08d84cc4d826df`; route file sha256 `234b50fc832f3abce6762ec8212673ecaa1c7e936f2cebd069ab2ff58a8716df`, unchanged from the base. 251 frames longer than C2-E3: the real barrier scene runs where the stub skipped it, and the route's two H19 workarounds (Juza's east-door round trip, the Krup detour) are still performed |

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
| C3-2 | a chapter `nurvus-zio` (`go_to_map 211`, `go_to` the corridor's row 30, `fight_scripted`), the committed engine, `--from-chapter nurvus-zio` | **halted** at the `go_to`, frame 739: `scene_fault: dialogue fault: scene resume has no saved cursor` ([H22](#h22-zio-at-nurvus-resumes-a-dialogue-instead-of-running-entry-0b-and-the-zio-phase-counter-is-unmodelled), part 1) |
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

## Halts and their diagnoses

H1 is a port defect. H2 to H11 are route claims or runner gaps, each fixed in
this lane; they are here so the next runner author does not rediscover them.

### H1: the Mile sand worm trigger halts every visit to Mile

**Fixed (issue #54, run R1-5).** `trigger_custom.rs` now transcribes `RunEvent_MileSandWorm` with the field RNG supplied through `TriggerContext` (one draw only when the flag and box pass), and the three other formerly unsupported custom triggers (`RidingElevator`, `EnterGrbkTwDoor`, `PenguFeedStolen`); the `Unsupported` variants are gone. The route visits Mile again, in `holt` and in `rune-dorin` (the Mile inn). Arrival at pixel y `$310` is outside the worm's box, so no RNG draw and no event. Event `$71`'s scene (`Event_MileSandWormBattle`, `ps4.asm:152222`) is not registered in `rust/psiv-core/src/scenes/mod.rs`; a player who walks into the box and rolls `& $1F == 0` meets `SceneMissing`. The same holds for `$37`/`$38` (Garuberk tower door) and `$96` (Pengu Feed stolen). The diagnosis below is kept as found.

- **Halt:** R1-1, chapter `holt`, objective 0 (`go_to_map 29 via_warp 1`), on the
  frame the party arrives in Mile (map `$1D`, cell (20,49)):
  `scene_fault: TriggerUnsupported { trigger: 112 }` (`$70`).
- **Cause (port):** `rust/psiv-core/src/trigger_custom.rs:43` answers
  `TriggerResult::Unsupported(.., Unsupported::Rng)` for `MileSandWorm`
  unconditionally, and `trigger_custom.rs:434` pins it (`the_four_undecidable_routines_say_so`).
  The trigger table binds `$5C..$70` to it (`rust/psiv-core/src/trigger_table.rs:407`).
  Most of the routine is decidable without any RNG.
- **Cartridge:** `RunEvent_MileSandWorm`, `reference/ps4disasm/ps4.asm:116449`:
  it tests `EventFlag_MileSandWorm` (`$1B`, `ps4.constants.asm:1505`) and
  returns NoEvent when set; returns NoEvent unless the leader's `curr_x_pos`
  is at most `$170` and `curr_y_pos` is within `$150..$2B0`; only then calls
  `UpdateRNGSeed` and fires event `$71` when `RNG_Seed & $1F == 0`. Arrival is
  at pixel y `$310`, so the cartridge answers NoEvent, takes no RNG draw, and
  the party walks on. The port halts where the cartridge does nothing.
- **Fix lane:** decide the flag test and the position box in `trigger_custom.rs`
  as the routine does, and raise `Unsupported::Rng` only inside the box with the
  flag clear (or give `TriggerContext` the shared RNG and run the draw and event
  `$71`, which is the real fix). Add the position cases as unit tests. Until
  then every visit to Mile halts a runner, and a player who walks into the
  worm's box meets the same gap.
- **Route alternative used:** Mile is not needed. The two Mile objectives of
  `holt` (a detour before Zema, flagged `verify` by R0) are gone; the route goes
  to Zema directly and every later claim still holds. The Mile inn at the start
  of `rune-dorin` is replaced by the Piata inn: the Zema inn is shut
  (`psiv-campaign plan --from-map 36 --from-cell 31,49 --to-map 39` with the
  flags the route holds finds no walk; with `event:0x33` and `event:0x37` it
  finds the 23-step one), so `go_to_map 25`, stand at (41,32), `rest_inn`,
  `go_to_map 0`. A player can choose that inn.

### H2: the Holt scene's landing cell (route claim)

`expect` after Professor Holt claimed Zema (60,21). The scene writes
`Map_Start = ($3C,$14)` in 8-pixel units (`docs/scenes/13_ProfHolt.md:26`):
cell (30,10), and the port stands the party one row south, (30,11), as at first
control. The route now says (30,11). No port defect: the claim halved the wrong
unit.

### H3: the Zema inn is shut early (not a defect)

`rune-dorin` first replaced the Mile inn with the Zema inn and halted with
`unreachable: no walk from (31,49) fires warp 4 of map 0x24`. The inn's door is
patched shut until the Igglanova rescue (static plan evidence under H1). The
story keeps the town sealed; the route uses the Piata inn.

### H4: a lost battle in the Valley Maze (runner gap)

The default policy only attacked, never cured between battles and never
revived; Chaz and Hahn fell in the first maze fights and Rune walked alone until
the party was wiped (`lost_battle`). The runner now cures the party through the
camp after each battle (`rust/psiv-campaign/src/recovery.rs`: the same TECH menu
a player uses, a healing technique from whoever has the TP, until nobody is
below 70%), and the in-battle threshold is 50%. With the cure in place R1-4
walks the maze with nobody down.

### H5: Gryz's storage door is object 1 (route `verify`)

`alshline` objective 7 named object 0; the press reaches object 1
(`talk` halts `wrong_object` naming both). Route fixed.

### H6: a lost battle in the Alshline basement (runner gap)

The chapters `alshline` and `zema-rescue` name `run_unless_boss`; the runner
resolved every name to the default policy and fought a basement formation it
cannot beat. `run_unless_boss` now runs from random encounters (RUN on the main
options until it works) and fights scripted battles.

### H7 to H11: runner gaps and route claims, fixed

- **H7** `open_chest` halted `wrong_object`: a chest press opens a chest
  window, not a dialogue. The talk controller reads the loot window as opened.
- **H8** `equip` for a member who wears something in every slot: the EQUIP
  menu opens its list only from an empty slot and takes off what a full slot
  holds (`rust/psiv-runtime/src/session/camp/equipment.rs`). The controller
  clears the item's own hand, or a head or body piece it puts back on after.
- **H9** the BioPlant elevator: after event `$13` the door cells are walkable
  map-change ground (collision 1) while the warp record is a ground trigger, so
  the stepping rule never "fires" it and R0's flood found no walk. The
  controller walks into the warp's trigger area and lets the game's elevator
  event do the rest. Confirms the route's `opens` claims.
- **H10** `bioplant-rika` objective 8: the Rika scene fires on the way and
  carries the party off the map, so the `go_to`'s target no longer exists. A
  scene that ends the walk off the target map now ends the objective; the next
  objective's `expect` asserts what the scene left.
- **H11** north bank: the `expect` claimed at least 1103 meseta, one native run's
  purse; a New Game run had 914. The purse is what the battles paid, not a rule,
  so the clause is gone.

An earlier gap, found on the first academy run: `Igglanova: talk` presses Speak
and the engine starts interaction area 0 of map 23 (event `$6B`), not an object's
dialogue; the controller accepts an area event as what the press opened.

### H12: the Alshline basement kills an under-levelled party after the walk shifted

Restoring Mile moved the frame at which every later random battle rolls (the
RNG stream is a function of the frame count and the pad history), so R1-5's
first attempt met a different basement sequence: formation 177 on map `$48` took
Chaz to 11/39 and Hahn to 8/30 in one round, `run_unless_boss` kept pressing RUN
(each failed run is another round), and the party was wiped inside five
encounters (`lost_battle`, chapter `alshline`, objective 13, frame 74,971).
R1-4's six-battle basement walk was a lucky draw, not a margin: Chaz was L4 and
Hahn L3 entering it. Not a port defect.

Fix, in `rust/psiv-campaign`: a new chapter `basement-training` between
`rune-dorin` and `alshline` rests at the Tonoe inn and then patrols the strip of
Motavia outside Tonoe's gate (the only ground reachable from it without a warp)
until the party is level 6. `patrol` gained an optional `refuge`: objectives run
when a member has fallen or a living member is below half HP after the camp
cure (`Driver::needs_refuge`), here an inn trip out and back, after which the
patrol resumes. Without it the first patrol attempt trained until the healers
ran dry and the party died on the strip. R1-5 trains in 22 battles with one
refuge and enters the basement at Alys L8, Chaz L6, Hahn L6, Gryz L7.

### H13: the Aiedo inn returns `AiedoEventPending` (#39)

**Open port defect (issue #39), not on the critical path.**

- **Halt:** C1-1, `rest_inn` at the supermarket counter (50,32) facing up:
  `unexpected_state -- the counter refused: "Aiedo rest event pending."`
  (`build/c1/evidence/h-inn-39-report.json`).
- **Cause (port):** `rust/psiv-runtime/src/shop.rs:126-127` returns
  `InnResult::AiedoEventPending` when selector 6 is rested at with `$42` and
  `$46` clear, after `RecoverStats` and before the bill; the shop window shows
  the placeholder at `rust/psiv-runtime/src/session/shop.rs:397`; nothing runs
  `Event_GirlsSneakingOut`, whose transcription is registered
  (`docs/scenes/27_GirlsSneakingOut.md`).
- **Cartridge:** `loc_66122`, `reference/ps4disasm/ps4.asm:136380`; after
  `RecoverStats` it tests selector 6, `EventFlag_Zio` and `EventFlag_GirlsCaught`
  and calls `Event_GirlsSneakingOut` (`ps4.asm:136406`, body `:146949`), which
  sets `$46`; the bill is taken after the scene (`loc_661B6`, `ps4.asm:136411`).
- **Smallest change:** make `shop_stay` hand the scene to the session: run
  event `$23` in the shop context (the scene tests already start it on map
  `$63`) and settle the bill when the scene ends.
- **Does it block the story?** No. `$46` is read only by Aiedo's own map data
  (`MapDataMan_AiedoSupermarket`, `MapDataMan_AiedoPrison`,
  `ps4.asm:109146-109178`, which show or hide the two girls) and by this inn.
  No trigger the next arc uses reads it (`RunEvent_SavingDemi` tests `$42`
  only, `docs/scenes/31_DemiRescue.md`). The route rests at Chaz's house
  instead, which is a free rest the cartridge offers (scenes 28 and 29, passing
  in `aiedo-chaz-house`). When #39 is fixed a chapter that rests at the inn
  should assert `$46` and the prison map should show the girls.

### H14: the fort needs a trained party and a rest that is not the Aiedo inn

Route and runner, fixed in this lane.

- C1-3 walked the fort's first corridors at level 8 to 11 with Rika at level 3:
  `lost_battle` on map `$84` (Zio Fort F1), the party wiped inside three
  encounters even with `run_unless_boss` (a failed RUN is another round of
  enemy attacks).
- Fix: a new chapter `aiedo-training` patrols the open ground east of Aiedo's
  gate (Motavia (40,56) to (50,56)) until every member is level 12, with a
  `refuge` that rests at Chaz's house (a free rest) when a member falls or the
  healers run dry, and rests there once more before leaving so the party enters
  the passageway whole. 171 battles, 281,447 frames. The first draft stopped at
  level 10 and arrived at Juza with three members down (C1-5 draft).
- Runner: `walk.rs` now ends `go_to_map` at once when a scene's yes/no prompt
  opens on the target map (Chaz's house asks on arrival), leaving the prompt
  for the next `answer` objective; any other prompt still halts as before.
  `tests/runner.rs::an_arrival_prompt_is_the_next_objectives_to_answer`
  (ignored, release) passes and halts when the `answer` is removed.
- A refuge bug found on the way: a `go_to_map` with `via_warp` names a warp of
  the *current* map. The refuge's first leg runs on Motavia, where warp 3 is
  not Aiedo's gate, so it walked the party across the continent into poison
  and a defeat (one draft run). The route names no `via_warp` there now.

### H15: passageway and Zio Fort encounters roll unsupported abilities

**Open port defects.** Random battles on the only road to the Zio Fort fault
the battle presentation when an enemy rolls an ability the engine does not run
(`BattleEvent::UnsupportedAbility`, emitted at
`rust/psiv-core/src/battle/engine.rs:745` and `:847`; the session turns it into
a fault at `rust/psiv-runtime/src/session/battle/mod.rs:386-391`).

| Ability | Carrier and where | Halt | Cartridge |
| --- | --- | --- | --- |
| 18 FUSION (`$12`) | 34 ZolSlug, formation 211 (three ZolSlugs), Passageway `$81`, the condition `EnemyAI_ZolSlugs` holds once exactly two remain (`rust/psiv-core/src/battle/enemy_ai.rs:106,463-466`) | `unsupported ability 18 for fighter 7`, frame 171,935 (C1-2); `build/c1/evidence/h15-fusion-report.json` | `ps4.asm:23017` (the condition); the routine is `EnemyAttack_Blob` (`ps4.asm:19241`, label `:23043`); ledger row `docs/battle/ENEMY_ABILITIES.md:276` (class unknown) |
| 33 FIREBREATH (`$21`) | 83 Ripper (regular list `[0,0,0,0,0,33,33,33]`), formations 219, 220 and 226 in the Zio Fort (`$82`, `$84`); 25 formations in all | `unsupported ability 33` (C1-3 variant, C1-4); `build/c1/evidence/h14-untrained-report.json`, `h16a-airslash-report.json` | `EnemyAttack_Ripper` `ps4.asm:21587` to `loc_23D0A` `ps4.asm:47507`; ledger row `ENEMY_ABILITIES.md:196` (damage, †) |
| 45 DEBAN (`$2D`) | 70 ShadowSabr, conditional slots `[45,45,44,44]` under conditions `[7,7,2,2]`, formation 226 (F1) | `unsupported ability 45 for fighter 7` (C1-4, first draft) | `EnemyAttack_ShadowSabr` `ps4.asm:21933`: arm `$2C` at `loc_F85E`, arm `$2D` at `loc_F89A`; the ledger lists only `$2C` (Airslash) for these carriers and `$2D` for 116 Radhin, so its conditional-ability table omits ShadowSabr's `$2D` |

- **Disposition:** the route runs from every encounter on the way
  (`run_unless_boss`, chapters `passage-to-zio-fort` and `zio-fort-approach`),
  which a player can choose. It is not a fix: with the draws of C1-3 and C1-4
  the same walk halts, and run C1-5 passes only because every fight in it
  ended by RUN before such an ability rolled. A change to the route anywhere
  upstream shifts the draws.
- **Smallest change:** one `DamageRoute` entry each in
  `rust/psiv-core/src/battle/enemy_damage.rs` (`DAMAGE_SKILL_ROUTES`, line 421)
  for FIREBREATH on 83 and its sibling carriers and for the two ShadowSabr arms,
  transcribed from the routines above; FUSION is not a damage ability (effect
  `$1F`) and needs its own transcription. FIREBREATH is the largest class: the
  ledger counts 54 formations (+3 boss) across the Zio Fort, Ladea Tower, Island
  Cave and later maps, so every dungeon on this arc meets it.

**C2 update.** FIREBREATH (83 Ripper and its siblings) is implemented, so the
fort's and the tower's FIREBREATH encounters are fought. FUSION and DEBAN are
still open: the passageway and fort walks run from every encounter, and C2's
final runs never rolled them. C2 met EVIL EYE (ability 76) in the Ladea Tower:
[H21](#h21-evil-eye-ability-76-in-the-ladea-tower-58).

### H16: Juza's battle rolls ZAN and FORCEFLASH, which the engine does not run

**Resolved by lane a1-damage (ZAN, FORCEFLASH, FIREBREATH and CORRSION are run
and oracle-verified); the route fights Juza from lane C2 on. The diagnosis
below is kept as it was written.**

- **Halt:** C1-7, chapter `zio-fort-juza` (scratch), objective 0 (`talk npc 0`
  at Juza, `$87` (32,19)): the dialogue fires event `$40`, event battle 3
  begins, and Juza's first action faults: `unsupported ability 71 for fighter 6`
  (frame 458,343; `build/c1/run-4/halt-juza.json`). An earlier draw halted on
  ability 86 for the same fighter.
- **Cause (port):** event battle 3 is one JUZA (enemy 114, 1,523 HP) with the
  regular list `[64,64,68,68,71,71,86,86]`. `DAMAGE_SKILL_ROUTES` carries 114's
  WAT `$40` (`rust/psiv-core/src/battle/enemy_damage.rs:626`) and FOI `$44`
  (`:654`) and nothing for ZAN `$47` or FORCEFLASH `$56`, which are half of
  his draws. A fight of more than a couple of rounds cannot avoid them.
- **Cartridge:** `EnemyAttack_Juza` `ps4.asm:20575`; ZAN `loc_2AF18`
  (`ps4.asm:56372`); FORCEFLASH `loc_2A300` (`ps4.asm:55527`); ledger rows
  `ENEMY_ABILITIES.md:228` and `:241` (both damage, unsupported; FORCEFLASH
  is also carried by 115 Greneris and 116 Radhin, ZAN by 100 TechMaster).
- **Why no alternative:** the stairs to F3 are patched shut until `$41` and
  `$48` are set (`ZioFortJuzaRoom` map effect entry `$38`: with both clear the
  four cells at (24,18) are written to collision 0), `$41` is set by talking to
  Juza, `$48` by the trigger after his battle (`docs/scenes/50_JuzaDefeated.md`),
  and F3 and F4 (the Demi rescue) are reachable only from that room (the
  planner finds no other chain to `$8A` or `$8B`).
- **Smallest change:** two `DamageRoute` entries for enemy 114 (ZAN, FORCEFLASH),
  traced to the routines above, with the regression test that fights event
  battle 3 to its end.
- **Forecast, not evidence:** the scripted battles ahead are event battle 4
  (enemy 152, ZIO, ability 112, "implemented" in the ledger), 5 (117
  GY-LAGUIAH, abilities 33 and `[0,0,0,0]`; FIREBREATH is unsupported) and 6
  (139 ZIO, ability 84 BLACK WAVE, the ledger lists it unsupported with 0
  formations); Ladea Tower's carriers include VOICE `$34` and FIREBREATH.
  The next lane should expect those halts after H16 is fixed. The party also
  arrives rich (21,832 meseta) with Hahn holding a shield and no weapon; shopping
  at Aiedo's weapon shop (`$5B`) and supermarket is a legitimate step before
  the fight, and Chaz's house is the rest.

### H17: Cutscene_AlysWounded and Cutscene_PsycoWand read the wrong dialogue tree

**Resolved by lane F1.** The four revision-gated loads are transcribed, and the
class is guarded: `psiv-core`'s
`every_scene_loads_the_trees_its_retail_bytes_load` holds the retail
`DialogueTreesToRAM` census and `tests/test_scene_trees.py` re-derives it from
the image, so a scene whose tree ops drift from its byte range fails a test.
The audit also found a fifth load the diagnosis had not named —
`Cutscene_ZioDefeated`'s `DialogueTree36` at `$075D84`, which is *not* under a
revision block — and fixed it in the same pass. The audit table is
[docs/scenes/REVISION_AUDIT.md](../scenes/REVISION_AUDIT.md).

**Open port defect on the critical path. It is the route's first halt:** run
C2-F1, chapter `zio-fort-demi` objective 2 (`go_to` Zio Fort F4), frame
1,023,474. The Demi rescue (`$8008`) and event battle 4 pass, trigger `$1D`
starts `Cutscene_AlysWounded` (`$8009`) as the next frame's scene, and it faults
(`build/c1/evidence/h17-halt-report.json`).

- **Halt:** `scene_fault: dialogue fault: scene resume has no saved cursor`,
  after the trace line `tree 5 entry 44 is empty; nothing to show`.
- **Cause (port):** in the Saya branch the scene loads Krup Inn F1 (`$3F`),
  whose map binds dialogue tree 5 (`MapRecord::dialogue_tree`, 1-based; pack
  `trees.json` index 4), then runs dialogue `$2C`
  (`rust/psiv-core/src/scenes/post_rika_cutscenes.rs:130`). Entries 44 to 46 of
  that tree are empty (`trees.json`, tree 5; its NPC lines are entries 41 to 43),
  so the window never opens, nothing is suspended, and the following
  `RunDialogueResume` has no cursor. The scene text lives in tree 6
  (`DialogueTree6`, pack `label: DialogueTree6`, `rom_offset 0x1E32A0`: entries
  44 to 46 are "Hahn! You've come home!", "Alys...I have no idea what these
  symptoms are...", "Chaz! Alys's condition has suddenly taken a turn for the
  worse!"). The same omission is in `$2D` (`:165`) and, in
  `Cutscene_PsycoWand`, in `$2E` (`:237`).
- **Cartridge:** `Cutscene_AlysWounded` runs `move.l #DialogueTree6, d0 /
  jsr DialogueTreesToRAM` before dialogue `$2C` (`ps4.asm:154305-154307`) and
  before `$2D` (`:154465`); `Cutscene_PsycoWand` before `$2E` (`:154633`) and
  before its late `popdlg` resume (`:154938`). All four sit under `if
  revision>0`, and this project builds `revision = 1` (English,
  `reference/ps4disasm/ps4.options.asm:4`), so they run. The scene docs record
  the end-of-scene `SetDialogueTree` of tree 5 only (`docs/scenes/32_AlysWounded.md`
  op 52 to 54, `37_PsycoWand.md` op 104 to 106).
- **Why no alternative:** trigger `$1D` fires on Zio F4 as soon as Zio `$42` is
  set and Demi Joined `$47` is clear; the rescue itself is the only way to set
  `$42`, and the rescue's scene hands the party straight to it.
- **Smallest change:** `const TREE_6: u32 = 0x001E_32A0;` and
  `SceneOp::SetDialogueTree { rom_addr: TREE_6 }` immediately before the
  `RunDialogue` ops for `$2C`, `$2D` and `$2E` in `post_rika_cutscenes.rs`,
  with the arc test asserting the Saya branch's dialogue opens.
- **Verified by experiment:** exactly those four added lines make
  `zio-fort-demi`, the Machine Center chapters, the tower and
  `ladea-tower-psycho-wand` pass in the route (runs C2-E1 to C2-E4); the scene
  ends in Krup `$3F` at (33,35) with `[Gryz, Chaz, Rika, Demi]` (Hahn and Alys
  removed from the party the route reordered earlier: the doc's
  `[Chaz, Gryz, Rika, Demi]` starts from the native order).

### H18: Event_ZioFortBarrier ($30) is not transcribed

**Resolved by lane F1.** `Event_ZioFortBarrier` is transcribed from the US
bytes as `ZIO_FORT_BARRIER` (35 ops, `EventPtrs[$30]`) — the `$63` test, both
dialogues, the five ring destinations in the cartridge's order with the
leader's closing walk to `(47,54)`, the eight-object clear and the `$64` write
— with its scene doc [90_ZioFortBarrier.md](../scenes/90_ZioFortBarrier.md),
the route's Ptrs $30 row updated in the census, and
`the_zio_fort_barrier_walks_its_ring_and_opens_the_courtyard` asserting the
ring, the flag, the despawns and the courtyard walk to Nurvus `$D7`.

**Open port defect on the critical path, no legitimate alternative: the route
stops at the barrier.** Seen on the experimental engine only (H17 patched): run
C2-E1, chapter `zio-fort-barrier` objective 2, frame 1,723,422
(`build/c1/evidence/h18-halt-report.json`).

- **Halt:** the party stands at Zio Fort `$82` (47,57) and talks to the
  InvisibleBlock object 9 at (47,55): `dialogue tree 13 entry 73 fires event
  0x30`, then `scene_fault: dialogue event 0x30 has no transcribed scene`.
- **Cause (port):** `docs/scenes/README.md:454` and
  `docs/scenes/12_ArcTriggerCensus.md:263` list `Event_ZioFortBarrier` (`$30`) as
  "direct, not transcribed"; no scene is registered for `EventIndex(0x30)`
  (`rust/psiv-core/src/scenes/mod.rs`).
- **Cartridge:** `Event_ZioFortBarrier`, `ps4.asm:148155-148251`: it tests After
  Alys Death `$63` (set after the Psycho Wand scene), runs dialogue `$44`, walks
  the party into a ring (Chaz, Rune, Gryz, Rika and Demi each to a pixel target,
  the leader to ($2F0,$350), standing cell (47,54)), runs dialogue `$45`, and ends
  with `EventFlags_Set` of `EventFlag_ZioFortBarrier` `$64` (`ps4.asm:148251`,
  `ps4.constants.asm:1551`: "Set after breaking the invisible barrier blocking
  the way to Nurvus"). The Zio Fort map's effect entry `$42` despawns objects 4
  to 9 (BarrierBeam1 to 4 and the InvisibleBlocks, the four `dialogue 73`
  blockers at (46..49,55)) when `$64` is set, which opens the central courtyard
  whose warp 8 at (47,49) leads to Nurvus `$D7` (`ZioFort warp 8`; no other map
  warps to Nurvus).
- **Why no alternative:** `$64` is written only by this event, the courtyard is
  walled on every other side (`flood`: 922 reachable cells, none inside), and
  Nurvus is reached only through it.
- **Smallest change:** transcribe `$06EC62..$06EE3F` as a scene (dialogues `$44`
  and `$45`, the five party moves, the final `SetFlag($64)`), register it as
  `EventPtrs[$30]`, and let the despawn take effect on the live map (H19).
- **Forecast, not evidence:** with a two-op stub (dialogue `$44`, `$64`) the
  barrier opens and the route reaches Nurvus (run C2-E3). Nurvus is not authored:
  the planner chain is Zio Fort `$82` (47,56), Nurvus `$D7`, Part2 `$CC`; Part2
  warp 2 at (44,19) arrives in the right corridor of Part3 `$CD` (warp 1 leads to
  a dead-end corridor with chest 0); the Part3 elevator door at (30..31,13) needs
  `interact` at (30,14) face up; B1 `$CE` arrives at (46,12); its west door
  (22..23,11) opens from (22,12); B3 `$D0` then splits into elevator-linked
  regions (doors at (14,11), (14,23), (70,11), (78,11), (78,23), (14,67)) and the
  stairs down to B4 `$D2` at (76,79) are not in the first region. The scenes
  ahead are `Event_ZioNurvus` (`$34`, trigger Nurvus B4 Part2 `$D3`, standing row
  31) and event battle 6: the arc test uses the Psycho Wand in the battle's first
  round (`rust/psiv-runtime/src/suites/next_arc.rs`, `use_psycho_wand`, enemy
  140), which the policy would have to do (an opening item), and the
  ledger lists BLACK WAVE (ability 84) as unsupported with no formation.

### H19: story flags and scene tile writes do not reach the live map's collision

**Resolved by lane F1.** The cartridge's mechanism is the first of the three
the diagnosis offered: both scenes write the layout themselves
(`GetMapLayoutOffset` + `RefreshPlaneBG`, `$06FB74` and `$06B5CE`), which is why
they are live without any map reload and why neither calls `RefreshMap`. The
port now transcribes those writes as state (`RestoreMapChunks` for Juza's own
base chunks, `WriteMapChunks` for the overworld chunk), and seven scenes were
audited for the same shape — `Cutscene_PsycoWand` and `Cutscene_ZioDefeated`
were dropping two more. The census is
[docs/scenes/LIVE_LAYOUT_WRITES.md](../scenes/LIVE_LAYOUT_WRITES.md), the tests
are `scene_map_tests`, and issue #59 (the camera-gate reload) is untouched:
the refresh path is not the mechanism either case uses.

**Port defect with a legitimate alternative; routed around (leave the map and
come back).** Two instances.

- **Juza's stairs.** After event battle 3 and `Event_JuzaDefeated` (`$41`) the
  stairs at (24..25,18..19) are open on a map built with `$41` and `$48` set, and
  shut on the live map the party stands in. Without the workaround
  `zio-fort-demi` objective 0 (`go_to_map 138`) halts `stuck`: the party stands in
  the stairs' warp rectangle `CellRect { x: 24, y: 18, width: 2, height: 2 }` and
  steps from one map-change cell to the next do not fire it
  (`build/c1/evidence/h19-juza-report.json`, a full run with the `go_to_map 136`
  and `go_to_map 135` objectives removed, frame 1,012,340).
- **The Machine Center's door.** After `Event_MachineCenterAppearing` (`$43`)
  the door cells (114..115,180..181) read collision 0 on the live overworld
  (`surroundings` of `build/c1/evidence/h19-machine-report.json`, the chapter
  without the Krup detour, `--from-chapter motavia-machine-center`, stuck at
  (115,180) with warp 21 listed). Draw-dependent: a random battle on the way
  rebuilds the map (`MapRefreshed`) and the door works, which is why only some
  draws showed it.
- **Cause (port):** the map's effects (flag-gated layout and collision writes)
  are evaluated when a map loads (`rust/psiv-runtime/src/map_change.rs:70`) and
  after a battle (`refresh_field_after_battle`, `:30`), not when a scene sets a
  flag. A scene can write chunks live (`SceneEffect::MapChunksWritten`,
  `rust/psiv-runtime/src/scene_runtime.rs:292`), but `Event_JuzaDefeated` records
  its four tile groups as "presentation-owned" (`docs/scenes/50_JuzaDefeated.md`
  ops 1 to 4) and `Event_MachineCenterAppearing` writes none (`34_...md`).
- **Cartridge:** the retail field reads its collision from the layout it just
  rewrote, so the stairs and the door are usable the moment the event ends
  (`ps4.asm:149162` onward for Juza; the building rises in
  `Event_MachineCenterAppearing`, `:115178` trigger, `$06B4B2..$06B6F3`).
- **Smallest change:** write the scene's tile groups as chunk writes, or
  re-evaluate the current map's effects when a scene ends after setting a flag
  (`refresh_field_after_battle`'s body without the battle).
- **Disposition:** `zio-fort-juza` leaves by the east door and re-enters
  (`go_to_map 136`, `go_to_map 135`); `motavia-machine-center` walks into Krup and
  out again. Both are things a player can do; when H19 is fixed they are
  redundant and harmless.

### H20: the vehicle is not parked at a map load, and boarding is missing

**Port defect, not blocking this route.** Found routing the Land Rover.

- **Halt (evidence run):** `use_item LAND-ROVER` on Motavia at Krup's gate:
  `camp item: LAND-ROVER NOT USABLE` (`build/c1/evidence/h20-land-rover-item-report.json`,
  a scratch chapter appended to the route).
- **Cartridge:** `GameMode_LoadFieldMap` does `clr.w (Vehicle_Index).w` unless
  `Map_Load_Flags` bit 0 or 2 is set (`ps4.asm:107517`; `RefreshMap` at
  `:121777` the same), so every ordinary warp leaves the party on foot with the
  machine parked. `ItemAction_LandRover` (`:123419-123431`) boards again from the
  ITEM menu, on Motavia, Dezolis or Rykros only and only when
  `Vehicle_Boarding_Flags` bit 0 is set (`VehicleBoardingFlags`,
  `ps4.asm:117131`), by writing Event `$09`; `Event_BoardingLandRover`
  (`:144950-145008`) builds the vehicle object and writes `Vehicle_Index`.
  `docs/field/VEHICLES.md` "Mount and dismount" records both.
- **Port:** `change_map_from` keeps `self.vehicle` across the load
  (`rust/psiv-runtime/src/map_change.rs:90-99`), so a mounted party walks into the
  Ladea Tower and must press Action to get off (the route has a `dismount`
  objective for it); and no scene is registered for events `$09`, `$0A`, `$0B`,
  and `camp.rs` has no item action table, so the Land Rover item cannot be used.
  Dismounting is therefore one way: the party in the route cannot take the
  machine across the sand again once it has left it.
- **Smallest change:** clear the vehicle in `change_map_from` unless the loading
  scene asked to keep it, transcribe events `$09` to `$0B`, and answer the three
  vehicle items from the camp.
- **Disposition:** not needed from Krup to the Zio Fort (the foot planner finds
  a seven-warp chain, 431 steps, from Krup's gate), so no route step depends on
  it. Whether the Land Rover is needed later in the arc is not claimed.

### H21: EVIL EYE (ability 76) in the Ladea Tower (#58)

**Open port defect (lane a2-status's scope), issue #58.** The tower's F1 to F3
encounters include a carrier of EVIL EYE (`abilities.json` enemy skill 76:
effect 7, psychic, power 64, `EvilEye`). When a RUN fails, or the policy fights,
the engine faults `unsupported ability 76 for fighter 6` (or 7) at
`rust/psiv-core/src/battle/engine.rs` (the same emission as H15). Seen at the
training targets 16, 17 and 21 of the C2 experiment (`ladea-tower-rune` objective
4, `go_to_map 143`), not at 18 to 20 or in the final route, whose passes depend on
draws. Per the lane brief it is recorded and not routed around.

### H22: Zio at Nurvus resumes a dialogue instead of running entry `$0B`, and the Zio phase counter is unmodelled

**Status: fixed in F3** (2026-10-02). Part 1 is the class fix in [the Dialogue2 audit](../scenes/DIALOGUE2_CALLERS.md) (`DialogueWindow::Retained`, a ROM-derived census); part 2 is `rust/psiv-core/src/battle/zio.rs` plus the Zio2 rows. The text below is the diagnosis as written before the fix.

**Open port defects on the critical path, no legitimate alternative: the route
stops at Zio's trigger.** Two defects, one behind the other, both met in chapter
`nurvus-zio` (Nurvus B4 Part2 `$D3`, crossing row 30). The committed engine halts
on the first; with the first patched (a probe, C3-3 and C3-4) the second halts it.

**Part 1: `Event_ZioNurvus` op 11.**

- **Halt (committed engine):** full run, frame 1,730,441,
  `scene_fault: dialogue event 0x0 has no transcribed scene`
  (`build/c1/evidence/c3-zio-nurvus-resume-report.json`); a `--from-chapter` run
  (a session loaded from a save) halts earlier, frame 739, `dialogue fault: scene
  resume has no saved cursor`.
- **Cause (port):** `rust/psiv-core/src/scenes/post_rika_events.rs:344` ends the
  scene's presentation with `SceneOp::RunDialogueResume`, which reopens the
  cursor `Saved_Dialogue_Addr` holds. In a played game that cursor is whatever
  dialogue ran last (the Zio Fort barrier's `$45`, so the resume opens the
  barrier tree's next entry, `$46`, whose text fires event 0) and after a load
  there is none. `docs/scenes/38_ZioNurvus.md` row 8-11 and the arc test
  (`rust/psiv-runtime/src/suites/next_arc.rs`, which dispatches the event with
  no dialogue pack, so a resume is silent) hide it.
- **Cartridge:** `Event_ZioNurvus`, `ps4.asm:148573-148629`: the call at
  `ps4.asm:148622-148623` is `moveq #$B, d0` / `jsr (Event_GetAndRunDialogue2)`,
  which is `GetDialogueByID` (`:119345`, entry `$0B` of the *current map's* tree:
  Nurvus B4 Part2's `dialogue_tree` is 36) followed by the window routine
  (`:121634`). It is not a resume. `Event_DarkForce1` (`ps4.asm:149134`) calls
  the same routine with entry 6 and the port transcribes that as
  `RunDialogue { Entry(6), Standard }` (`post_zio_cutscenes.rs:653`).
- **Smallest change:** replace the op with
  `SceneOp::RunDialogue { source: DialogueSource::Entry(DialogueId(0x0B)), window: DialogueWindow::Standard }`,
  correct row 8-11 of `38_ZioNurvus.md`, and give the arc test a Session-level
  assertion (a headless event with no dialogue pack cannot see this class).
  **Verified by experiment (C3-3):** that one op makes the scene open
  `SceneDialogue { entry: 11 }`, set `$65` and start event battle 6.
- **Class:** `Event_GetAndRunDialogue2` has 14 callers
  (`ps4.asm:148623, 149136, 149154, 150038, 150056, 150173, 150182, 150364,
  150750, 152563, 152871, 152977, 153841, 157969`: Zio Nurvus, DarkForce1, Juza,
  DarkForce2, XeAThoul, AirCastleFakeChest, AngerTowerAlys, FractOozeFound,
  KingRappy, DaughterTerminal, ProfoundDarkness and three local labels,
  `loc_70908`, `loc_70D2E`, `loc_7442E`). This lane audited none of the others
  for a `RunDialogueResume` transcription; a fix lane should.

**Part 2: the Zio phase counter.**

- **Halt (probe, part 1 patched):** `unsupported_ability: battle renderer: engine
  emitted unsupported ability 84 for fighter 6`, in round 1 of event battle 6,
  from a `--from-chapter` run (frame 1,441) and from the full run (frame
  1,731,142, `build/c1/evidence/c3-zio-black-wave-report.json`).
- **Cause (port):** the first form (enemy 139 `ZIO`, 16,383 HP, agility 255, so
  it acts before anyone) has `regular_ability_ids` `[84 x 8]`
  (`runtime-pack/battle/enemies.json`), and `engine::roll_enemy_ability`
  (`rust/psiv-core/src/battle/engine.rs:746`) dispatches the rolled id, which is
  BLACK WAVE, unsupported. Only enemy 152 (Zio3) has its phase counter
  (`engine.rs:774`, `zio_phase`).
- **Cartridge:** `EnemyAttack_Zio` (`ps4.asm:19483-19517`) ignores the roll and
  advances the counter `$FFFFEE98` (cleared by `EnemyInit_Zio`, `:17895`): phase 0
  writes `$6B` and `BattleObj_MagBarrir` (`:67319`), phase 1 writes `$53` and
  `BattleObj_NightmareFull` (`:66960`), later phases `$54` and
  `BattleObj_BlackWave3` (`:66897`). `EnemyAttack_Zio2` (`:19519`), the form the
  Psycho Wand reloads (enemy 140, 2,889 HP), shares the counter and rewrites
  `$24(a4)` from it: the same family as issue #62 (Zio2 CORRSION). The brief's
  `$54`/`$6C` BLACK WAVE are these two arms. The engine's doc
  (`docs/battle/ENEMY_ABILITIES.md:245`, `:261`) lists both as `scripted/custom`,
  unsupported.
- **Smallest change:** model `EnemyAttack_Zio` and `EnemyAttack_Zio2` as the
  engine models Zio3: a counter on the battle, reset at battle start, with the
  phase's ability and (for `$6B`) the barrier; this is #62's work extended to
  enemy 139's three arms.
- **No legitimate alternative:** the trigger is the corridor itself (`RunEvent_ZioNurvus`,
  `ps4.asm:115720`: Zio Nurvus `$65` clear and leader Y exactly `$1E0`), the only
  way north in Part2, and the fight is the story's.
- **What lies behind (not claimed):** C3-5, a probe that makes the engine run
  Zio as a plain attacker, lost to Zio2 at levels 19 to 24 with the boss policy
  after a round-1 Psycho Wand. Whether the party can win the fight the cartridge
  gives (CORRSION, HEWN, BLACK WAVE `$6C`) is open; the route has not been
  built past it, and `Cutscene_ZioDefeated` (`$800B`, Motavia) has not been run.

### F3 results after the H22 fixes

**Before:** the full run halted at Zio's trigger, frame 1,730,441, on a mistranscribed
op (C3-F1, above). **After:** the route reaches the fight, plays it, and loses it
(F3-0). Two things decide that loss, and only one is the route's.

1. **A session defect wastes the Psycho Wand (not the route, not this lane's tree).**
   `EnemyInit_Zio` raises `$FFFFEE87`, and `loc_B62A` (`ps4.asm:17448`) turns that into an
   enemy ambush (`scripted_flag.rs`). The cartridge's `Battle_ProcessCOMD`
   (`ps4.asm:7636`, `tst.b Battle_Priority / bmi`) then skips the party's command input
   for that round and `Battle_OrderTurns` queues the enemies alone, so round 1 is Zio's
   Magic Barrier and nothing else. `session/battle` (lane p1-battle) still opens the
   command menu in an ambush round: `psycho_wand_then_win` spends its opening
   PSYCO-WAND there, the round discards it, and Zio (139, 16,383 HP, the party's hits
   do 1) kills one member a turn from round 4 on (Black Wave, `loc_25048`). Reported to
   p1-battle. The route and the policy are unchanged.
2. **With the wand in round 2 the party still loses on balance.** A throwaway
   local edit of the policy (reverted, not committed) that waits for the second menu
   round: round 2 Zio casts Nightmare, the wand reloads Zio2 (2,889 HP), and the party
   (Gryz L23, Chaz L23, Rika L24, Demi L19, Rune L23, 137/195, 105/150, 164/164,
   144/144, 115/115 on entry) is defeated in the fifth round after the wand, having
   dealt about 1,100 of Zio2's 2,889 HP while CORRSION (78, 72, 76, 97, 48 in one
   round) and BLACK WAVE hit it. Report
   `build/f3/evidence/wand-round2-experiment-report.json`. Whether a better trained
   party, other equipment or other commands win is a **route question**, reported with
   the levels and rounds above and not changed. `Cutscene_ZioDefeated` has not run.

Evidence class: the cited chains (`zio.rs`, `routes/zio.rs`, `DIALOGUE2_CALLERS.md`),
unit and engine-driven tests, and these two runs. **No oracle capture** of the Zio
battle exists: `oracle.force` forces a *group* through a probe run that locates the
formation draw (`Battle_SetupEnemyData`'s `UpdateRNGSeed2`), but an event battle
(`Event_Battle_Index >= 0`, `ps4.asm:11813-11818`) takes the boss block with no draw,
so a `--ram-patch` of that byte would need the probe, durable-patch and fixture
phases (`oracle/force/phases.py`, `draw.py`, `durable.py`) reworked around a battle
that has no draw, plus an ITEM-menu policy (`oracle/force/tape.py` has `attack` and
`defend` only) and a Psycho Wand and party seeded into RAM. That is a separate lane,
not a patch to this one; no replay fixture was added and none changed.

### #70 and Zio route trials on the isolated candidate

The candidate starts at `82eed831af88234feec8ab8b9322a9d2624de91f` in
`/home/peter/.cache/codex/worktrees/PSIV/ambush-zio`. Its ignored
`runtime-pack` and `reference` entries link read-only local inputs; pack
manifest SHA256 is
`018df2227406af1f09412b9ec3550724a2f9b8688aa0400c1cd707f5b4d05650`.
The built release runner uses the isolated `rust/target` directory,
`CARGO_BUILD_JOBS=1`, and the shared heavy-job flock. No run writes the F3
source saves.

| Trial | Input and command | Result | Preserved evidence |
| --- | --- | --- | --- |
| #70 baseline after the Session gate, old training | F3 `27-nurvus-descent/slot_1.sram` copied byte-for-byte, SHA256 `c9417845688a5f45cf113271672f5cb82f6cd4d1ffac877c24a4af1bbb9d90af`; `psiv-campaign run routes/main.json --from-chapter nurvus-zio --save-dir build/ambush-zio/baseline --pack runtime-pack` | Exit 2, `lost_battle` at frame 4,868, digest `3cea66160c4728e1`. Trace chooses PSYCO-WAND at the first actual player-command menu after Zio's ambush, then loses with the 23/23/24/19/23 party. | `build/ambush-zio/baseline/{runner.log,halt-report.json,run.tape,timing.txt}`; tape SHA256 `c8bf88bf218bc51e0a37c370f6a3c015d686eb8f0b1bd4fb11edbca35f6691ab` |
| Level-23 Krup training, old one-cure boss policy | F3 `20-zio-fort-demi/slot_1.sram` copied, SHA256 `12ee2a8a2507ba01c8d02ba712175a3738e5a2ca70ba4fc12c308c0619abcece`; `--from-chapter krup-training --save-dir build/ambush-zio/train23` | Exit 2, `lost_battle`, frame 1,359,374, digest `2957bdc96a7704ec`. Training took 1,315,336 frames and 1,620 ordinary battles; Zio party reached 27/26/28/23/26. Higher levels alone did not close the fight. | `build/ambush-zio/train23/{runner.log,halt-report.json,run.tape,timing.txt}`; predecessor chapter save SHA256 `863cdcd43975cba7bc5308726841b8e7d0edcf17de847fa9c18730b5e11ff78d` |
| Same trained party, supported group cure | Copied the preceding trial's `27-nurvus-descent/slot_1.sram`, same SHA256 `863cdcd43975cba7bc5308726841b8e7d0edcf17de847fa9c18730b5e11ff78d`; `PSIV_CAMPAIGN_TRACE=1 psiv-campaign run routes/main.json --from-chapter nurvus-zio --save-dir build/ambush-zio/group23 --pack runtime-pack` | Exit 0, 9,271 frames, digest `278ef9a69eab6171`. The trace shows the wand via ITEM in the first player round and Rika's TECH 28 GISAR in later damaged rounds; the scene settles and the runner writes a **read-only chapter checkpoint** (this trial predates the final pad-SAVE objective). Fresh-Session `inspect` reads map `$00`, Chaz/Rika/Rune, and `$65/$68/$66/$61`; Rune is at 0/127 HP. | `build/ambush-zio/group23/{runner.log,run.tape,28-nurvus-zio/slot_1.sram,timing.txt}`; tape SHA256 `ab2039431e87cd5acb73b586eecdf54714ded86ef217cc741d65b3bc0cfce0d4`, checkpoint SHA256 `c7ceab137ae371a4a47ab839005651124789e407a93cfc43b1f6b3f529a34531` |
| Old-level party with group cure | Copied the same F3 pre-Zio save as the #70 baseline, SHA256 `c9417845688a5f45cf113271672f5cb82f6cd4d1ffac877c24a4af1bbb9d90af`; new group-heal policy and final pad-SAVE route objective, `--from-chapter nurvus-zio --save-dir build/ambush-zio/group19` | Exit 2, `lost_battle`, 5,862 frames, digest `f823f4e3972ac976`. Rika chooses earned SAR (technique 27) through the menu in four rounds; its healing extends the fight but does not win. | `build/ambush-zio/group19/{runner.log,report.json,run.tape,timing.txt}`; tape SHA256 `0974fa58f5a880821244c27e3fef1b4305c9dde078443f0d92a76022771bf5c4` |
| Level-22 training with group cure | Same F3 pre-training source save, SHA256 `12ee2a8a2507ba01c8d02ba712175a3738e5a2ca70ba4fc12c308c0619abcece`; an ignored local route copy changes only `krup-training`'s target to 22; `--from-chapter krup-training --save-dir build/ambush-zio/group22` | Exit 2, `lost_battle`, 1,162,849 frames, digest `57b8fca3f79b5c2d`. Training took 1,120,497 frames and 1,381 ordinary battles; Zio party 26/25/26/22/25 has SAR but not level-28 GISAR. The pre-Zio chapter checkpoint SHA256 is `deebc62edb3f1e4d4a3d57b1a352e3adc6df7dbb9149233e6bf7dd9b10f4ed37`. | `build/ambush-zio/group22/{route.json,runner.log,report.json,run.tape,timing.txt}`; tape SHA256 `fb091133a0764f90f7af565e576c8d06eaa53637269d1e899a166080ae887050` |
| Level-23 trained party with actual pad SAVE | Copied the same level-23 pre-Zio save SHA256 `863cdcd43975cba7bc5308726841b8e7d0edcf17de847fa9c18730b5e11ff78d`; final route now includes `save` after Zio's settled scene; `--from-chapter nurvus-zio --save-dir build/ambush-zio/group23-save` | Exit 0, 9,297 frames, digest `98edcf5b85d2ecb5`. The added 26 frames are ordinary camp STATE → SAVE input. `route/slot_1.sram` and the read-only chapter checkpoint have identical bytes, SHA256 `c7ceab137ae371a4a47ab839005651124789e407a93cfc43b1f6b3f529a34531`; the separate route slot is the persistence evidence. | `build/ambush-zio/group23-save/{runner.log,run.tape,route/slot_1.sram,28-nurvus-zio/slot_1.sram,timing.txt}`; tape SHA256 `f87edbfe8003cf4724026f496437bdc461de37882a108e9d23b361267c692850` |

### Full New Game probe after the first freeze

The first frozen local commit `c9647831868a2054cec13253064e2ef9e930da80`
and release runner SHA256 `d00bea217f698dd181dcc548fe27dc5419975773e77797de11f703348915d4b2`
used the same pack manifest as above. The full run exposed an earlier route
survival problem after the ambush command timing changed the encounter stream;
it did not reach Zio. Both outcomes below are preserved, not substituted for
the required final twice-replayed route.

| Probe | Result | Raw evidence |
| --- | --- | --- |
| Tracked route at `c964783`, New Game | Exit 2, `lost_battle` in `rune-dorin` objective 15 (Valley Maze), 67,937 frames, digest `9e1c25c1cfc5e4ee`. After 20 battles the party's Chaz TP is down to 2 and the last formation `$9D` defeats it. | `build/ambush-zio/full-a/{runner.log,report.json,run.tape,timing.txt}`; tape SHA256 `c1368124a2701f80609c5e57afccc8158bb3e1de211fe7e97fa64d0962aad74d` |
| Ignored local route copy changing only `rune-dorin` random encounters to `run_unless_boss`, New Game | Exit 0, all 29 chapters and the final camp SAVE, 2,430,197 frames, digest `bdbe45da995ad466`, 32.80 seconds. `rune-dorin` settles in 17,911 frames after 11 encounters; `nurvus-zio` finishes with Chaz L26, Rika L28 and Rune L26. The route save SHA256 is `e6a27534bf7e964800fbf29d6232c146cc677df766cb3637b5e8001559713e09`, byte-identical to the separate read-only chapter checkpoint. Fresh-Session `inspect` reads map `$00` (54,93), Chaz/Rika/Rune all alive and `$65/$68/$66/$61` set. This is a player RUN choice, not a battle-rule change. | `build/ambush-zio/full-rune-run/{route.json,runner.log,run.tape,route/slot_1.sram,28-nurvus-zio/slot_1.sram,inspect.log,timing.txt}`; route-copy SHA256 `a8cf92f36b439fea3ec0f4a2101589c6f75a492c3124b219a0244931cebc1c80`, tape SHA256 `6109cb681fd33b6027eea181428db1443126e91f431e91d853326db33801aa84` |

### #70 frozen headless acceptance

The tracked route's `rune-dorin` RUN choice was frozen at `da3df22` and
completed twice from New Game (`full-final-a/b`: 2,430,197 frames, digest
`bdbe45da995ad466`, tape SHA256 `6109cb681fd33b6027eea181428db1443126e91f431e91d853326db33801aa84`,
pad-SAVE SHA256 `e6a27534bf7e964800fbf29d6232c146cc677df766cb3637b5e8001559713e09`).
Those passes were **superseded** by a review correction to healing targets;
their bytes remain in the ignored evidence directories.

| Frozen source | New Game result | Evidence |
| --- | --- | --- |
| `64e4023`, shared core target lists, before proactive group cure | Exit 2 at Zio, 2,426,644 frames, digest `0460ec17c11b5a82`; the halted tape replays exactly (SHA256 `d66185710995eef97903d184c378eee44e6be1ad440585c0f910029b07806127`). After CORRSION, Chaz and Rika sit around 62% HP; the half-HP-only group rule skips GISAR, then Chaz dies. A chapter-resume probe from a hash-verified copy of the source-earned pre-Zio save wins under its reset RNG; it is diagnostic, not full-route acceptance. | `build/ambush-zio/full-target-a/{runner.log,report.json,run.tape,replay-halt.log,timing.txt}` and `full-target-trace/trace.log`; copied predecessor SHA256 `18ec910253f4597b3d6a62389f83bc55145c0a88ab5c3b7bb13e4b8cba47094b`, probe `zio-target-probe/` |
| `6fb1a70`, group cure below 70% for at least two eligible humans | **Completed twice**, all 29 chapters, 2,430,333 frames and digest `d4a124057f4439cd` each; full tapes byte-identical (SHA256 `9b3ebe16dbb9241f679ab15033d8cdf7ea478942f6c1c27b204c071e0916ff0c`). Both tapes replay to the digest. Both distinct `route/slot_1.sram` pad SAVEs and the read-only final chapter snapshots are byte-identical (SHA256 `8443a11e790bfb51a8ede3f2d515f452d99050eaf1db3548a71c4525eb8ce6bd`). Fresh-process `inspect` loads each route slot at Motavia `$00` (54,93), Chaz L26 123/174, Rika L28 179/180, Rune L26 127/127, with `$65/$68/$66/$61` set. | `build/ambush-zio/full-group-{a,b}/{source.txt,inputs.sha256,binary.sha256,runner.log,run.tape,replay.log,route/slot_1.sram,28-nurvus-zio/slot_1.sram,inspect.log,results.sha256,timing.txt}`; `full-group-b/comparison.txt` confirms source, inputs, binary, tape, pad SAVE, snapshot and inspection identical |

The accepted isolated runner is the worktree's own
`rust/target/release/psiv-campaign`, SHA256
`e025a578f99afd7eb97fea87869893545eaf50f7443eea30b8171d4ee1d26161`,
with `CARGO_TARGET_DIR` set to that worktree, `CARGO_BUILD_JOBS=1` and the
shared heavy-job flock. The route SHA256 is
`56faad360a5be1a2d8fa2a7d5f25d15b05ad8f490ece010f20cb1f094896747f`;
the pack manifest SHA256 remains
`018df2227406af1f09412b9ec3550724a2f9b8688aa0400c1cd707f5b4d05650`.
The final `save` objective presses camp STATE → SAVE after the scene settles;
every chapter also has a separate read-only checkpoint, but only explicit
`save` objectives exercise the ordinary SAVE input.

The review correction uses the [correction ladder](../AGENT_WORKFLOW.md#correction-ladder)
at rung 1: `DefaultPolicy` and `BossPolicy` consume the core's
`technique_targets`/`item_targets`, the same API used by the runtime menu.
The mixed Chaz/Demi/Rika negative case keeps the most injured android out
of RES and MONOMATE targets, and checks GISAR when two humans are hurt.
The 70% group threshold is a player
choice after observed all-party damage; single-target cures remain below
half HP. Neither policy change adds a technique or changes its retail effect.

The source decision for #70 is in
[battle-party.md](../source-notes/battle-party.md#2026-10-03--rounds-that-open-no-party-command-window):
`loc_52D6` skips options on negative priority and
`Battle_ProcessCOMD` skips all `$6E` actors; normal/preemptive priority still
offers commands. `Battle_OrderTurns` queues enemies only on ambush and clears
priority after the round. The read-only priority accessor changes no RNG or
turn order. The route's `psycho_wand_then_win` policy disarms only on an
actual menu choice. Rika learns GISAR (technique 28) at level 28 in the
retail level table; it is existing effect 18, all-human target nibble 5,
and the boss policy chooses it through the TECH menu when multiple humans
are hurt. It cannot heal Demi, an android. Neither group-heal policy nor
training edits battle balance or the core Zio phase model. SAR at level 20
was tried; the level-22 party still loses, while the level-23 training route
earns GISAR at Rika's level 28 and wins. There is **no
event-battle oracle fixture for Zio**, so these are source-backed headless
Session route results, not oracle parity.

## `verify` items resolved by playing

R0 left 12 objectives marked `"verify": true`. All are resolved and the flags
dropped; the run that settled each is in the route's `note`.

| Chapter and objective | Question | Answer |
| --- | --- | --- |
| academy 10 | Hahn: object 0 or 1 | object 0 |
| academy 16 | Igglanova: which object | none: the press starts interaction area 0 (event `$6B`); object 0 only places the party |
| holt 0 | the Mile detour | restored once the sand worm trigger was transcribed (H1); arrival is outside the worm's box |
| holt 7 | Zema cell after the Holt scene | (30,11), not (60,21) (H2) |
| rune-dorin 20 | Dorin: object 0, 5 or 8 | object 0 |
| alshline 7 | Gryz's storage door | object 1 (H5) |
| zema-training 3 | patrol until level 6 and 1,180 meseta | holds after 55 battles in R1-4; the runner cures the party between them |
| bioplant-elevators 4 | `temp:0` after the door | set |
| bioplant-elevators 8, 10 | `opens` cells of the elevator doors | walkable (collision 1) after the scene (H9) |
| bioplant-rika 0, 4 | the same | the same |

## Not claimed

- **C2's balance.** Everything the route fights from Juza on is won by a party
  trained to level 17 before the fort and level 19 on Krup's ground, with the
  boss policy; none of it is evidence that the cartridge's intended party wins
  at those levels. Gy-Laguiah is a close fight (level 16 loses; 18 to 20 win in
  the sampled draws). Juza is won with margin at level 17 (`zio-fort-juza`:
  Hahn on 76 of 100 HP at the end).
- **The boss policy's estimate.** It reads the live fighters' stats and the
  cartridge's formula at its mean roll. A player reads the same ranking from the
  damage numbers; the policy skips the experiment. Skills other than Crosscut
  and Vortex are not chosen because the engine does not run them.
- **Past Zio's defeat.** F3's losing fight remains the old-base observation
  ([F3 results](#f3-results-after-the-h22-fixes)). The isolated #70
  [full New Game receipt](#70-frozen-headless-acceptance) reaches the settled
  scene and an ordinary SAVE in headless `Session`. It does not establish
  combined Godot/native play, generic Godot tape replay, Zio event-battle
  oracle parity or any later arc.
- **Balance.** Members fall in `bioplant-rika` (ten command windows open with a
  member down; Gryz, Alys and Hahn stand at 0 HP when the Rika scene ends); the
  inn restores them (`north-bank` rests at Zema before the crossing). The policy
  does not revive and does not grind for safety, and the party reaches Aiedo at
  levels 6 to 8. A route that wants a healthier party names an inn or a
  `patrol`.
- **Seeds.** Frames and battles of a run are a function of the whole pad
  history. A `--from-chapter` run restarts the frame counter and RNG, so it is
  the same game but not the same frames; only the full run is evidence.
- **Unexercised objectives.** `sell`, `use_item` and an explicit
  `use_technique` are not in `routes/main.json`; `tests/runner.rs` plays them at
  the Piata item shop. `Session` field notices (a member falling to poison) have
  no pad path, so the runner halts on one (`unexpected_state`); none occurred.
- **The boundary.** Construction (`Runtime::new_game`, `enable_battles`,
  `start_event`, `from_save`) is the one thing the runner does outside
  `Session::frame` and views, in `src/start.rs`. Node S5 will make those
  crate-private; the runner then needs a `Session` constructor taking the pack
  and the battle files.
