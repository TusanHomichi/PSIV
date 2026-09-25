_Alshline and Zema — part of the [native playability ledger](NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._

## Connected Alshline progression

The earlier native attempts resumed the Tonoe save, paid the 60-meseta inn,
and opened Gryz's storage door through ordinary dialogue/input. The first
physical-only attempt lost Chaz to the basement's BLOB formation. The next
attempt used Alys's learned FOI, won that fight, then wiped against four
TOADSTOOLs on B1. Investigation then found a runtime defect: level-ups had
discarded all learned techniques, learned skills and skill-use growth. The
saved level-four Chaz was missing TSU; Hahn was missing his level-three WAT.
`PROGRESSION.md` records the fix and explicit three-byte repair of a copy of
that campaign save. The original is preserved. Subsequent attempts
used the repaired copy and normal commands, including support techniques.
With the repaired save the party wins the first basement battle, heals
paralyzed Alys through normal RES, walks onward and wins the next B1 fight.
Alys and Hahn are dead after that second victory, so the unprepared route
stops. The receipt is retained under `build/native-alshline-under-equipped`.
That attempt was not a campaign collection/return proof. Earlier failed artifacts are retained under
`build/native-alshline-physical-attacks` and `build/native-alshline-spells`.

## Instant-death player commands

CRASH, BROSE, VOL and SAVOL now execute their original success/resistance
checks, consume the original resource, and grant normal enemy rewards on a
successful kill. See `../battle/INSTANT_DEATH.md` for source anchors, regression results
and the still-missing special animation work. This brings ordinary battle
techniques to 30 of 40 and implemented character battle skills to four of 54.

## Battle recovery techniques

ANTI, RIMPA, REVER and REGEN now work during battle as well as in the field.
Their isolated native command, victory, save and fresh-title Continue proof
is documented in `../battle/BATTLE_RECOVERY.md`. That brought the total to 34 of the 38
battle-usable techniques, plus both field-only travel techniques. Earlier
counts used all 40 techniques as their denominator.


## Party STATUS and RIMIT follow-through

`PARTY_STATUS.md` records the five-seat battle status repair, three-digit
numeric field corrections, eleven original STATUS portraits, original age
words and profession-name decoding. The native five-person battle, all five
STATUS selections and SAVE are verified; the earlier display-only fixture
also passed fresh title Continue with only the expected movement byte change.

`../battle/RIMIT.md` records effect 7, core/runtime proof, native casting/sleep/victory/
SAVE and fresh title Continue/movement/SAVE validation. Its FEEVE note corrects an apparent data interpretation: the US
routine writes a saved water-resistance byte, not live psychic resistance.


## Crawler THREAD correction

The next retreat attempt reached the final Tonoe basement floor and exposed
unsupported THREAD being replaced by physical attacks. Two party members
died; `build/native-alshline-thread-fallback` preserves that failed run.
`../battle/THREAD.md` records the original object dispatch and the correction: one
physical-resistance agility check, no HP damage or extra attack. Core and
real-formation runtime tests pass; the corrected campaign retry now completes.

The preceding `build/native-alshline-stair-avoidance` failure was confined to
the test player: it refused to cross another cell of its arrival staircase.
The game already suppresses a repeat warp between adjacent map-change tiles;
the path planner now allows leaving the trigger rectangle it starts inside.


The development launcher now checks for a running game or mapped extension
before building. A per-user launch lock prevents concurrent launch/build
attempts. `build/native-rimit/launcher-guard.json` records a real test with
two native Godot processes loading the extension: the launcher refused the
rebuild and the library hash and modification time stayed unchanged. On this
host `fuser` missed the mappings, so the guard reads `/proc/*/maps` directly.


## Connected Alshline completion

`build/native-alshline/route/route.json` completes 16 route legs and six
ordinary random encounters, all handled through RUN. The repaired Tonoe
source is copied unchanged; the 60-meseta inn restores the party, Gryz opens
the storage door, normal movement reaches the final basement chest, and
Alshline enters inventory as item 141. Its conversation completes and sets
flag 0x32. Crawler THREAD executes without the former physical fallback;
three ordinary RES casts keep the four party members alive.

At SAVE (tick 10828), the party is in Tonoe map 0x41 at (31,11), has 2065
meseta, Alshline, and Alys/Chaz/Hahn/Gryz HP 52/37/28/51 and TP 40/10/39/20.
Levels remain 7/4/4/6. The run produced no unsupported-ability diagnostic.
The opened chest, THREAD narration and saved-state captures were inspected.

The source for the next connected leg is
`build/native-alshline/saves/slot_1.sram`, SHA256
`2b0b4dd7cf2efa3b802b4dbdfd8b05d9f5aff4dcee8ac819a9624c0dc4ae154d`.
Fresh title Continue, one Down and SAVE to slot 2 pass in `continued/`: only
logical payload Y at 0x309 changes (176 to 192), with the source slot intact.
All original Tonoe and explicit progression-repair source bytes are preserved.
The library used for that Alshline proof was SHA256
`cfee377efe01420ab7fb55ffe22e3fa6cf670d719487d1cc2296f1347a900c64`;
882 workspace tests and strict Clippy pass. This proves the connected quest
through its return, not yet Zema's rescue or an end-to-end game playthrough.

## Connected Zema rescue and restart

`build/native-zema-return/route/route.json` continues the verified Alshline
save through a paid Tonoe rest, the mountain pass, all 50 rescue dialogue
pages, nine ordinary RUN escapes, and the Igglanova battle using normal
commands. Seven residents return, Alshline is consumed, flags 0x33 and 0x37
are set, and the party saves outside Zema at (99,83) with 3063 meseta.
Alys/Chaz/Hahn/Gryz have HP 53/39/33/66 and TP 31/1/31/20, levels 7/4/4/6.

That run used native library SHA256
`7c4e908f547de00e8a7612bd70a79b03d4e4679e07b4e5ff2df8e2e32d6b34ca`.
The saved slot 1 is SHA256
`75ce5fb3fb92660d9adfdb459059c516b00c0016fa078f5ed0428608d0916a40`.
Fresh title Continue, one Down, and SAVE 2 passed. The reusable validator
checks unchanged source bytes, valid slot/checksum headers and an otherwise
identical gameplay payload: logical byte 0x309 changes 48→64. Slot 2 is
`80dc8ef6f6fdc69985ee0500a3a02e19d52254d9ac5efd4f2a31a54baf594b47`.
Receipts and captures are in `continued/` beside the route.

The restored text, green Igglanova entrance, field dialogue without stale
comic panels, post-battle town and SAVE captures were inspected. The town
capture exposed a palette fault: residents were active but still rendered in
grey. The subsequent Zema equipment run below verifies consumption of their
flag-gated normal palette variants.
This is connected gameplay/restart proof, not a claim of visual or overall
original-game parity. See `../scenes/SCENE_DIALOGUE.md` and `BIOPLANT_NATIVE.md`.

## Connected Zema equipment and restart (2026-09-15)

`build/native-bioplant/outfit/route/route.json` continues the rescued-Zema
save, pays 80 meseta for a full inn recovery, buys seven items for 2470,
equips Alys with two slashers, Chaz with STEL-SWORD, Gryz with BROAD-AXE,
and Hahn with CRBN-SUIT, CRBNSHIELD and CIRCLET. The hand-selection cancel
check changes no inventory or equipment. The driver also checks all seven
residents' normal sprite sheets; their restored colours and the corrected
equipment window captures were inspected.

The saved party stands outside Zema at (99,83), with 513 meseta, full HP/TP
and the original levels. Slot 1 is SHA256
`6f257d112740505f92ad7163759da559e9d5d4f8bdb646ea1cbbb2155cf0871e`.
Fresh title Continue, one Down, and SAVE 2 pass in `outfit/continued/`:
only payload Y at 0x309 changes 48 to 64, with the source slot intact.
The original Zema rescue save is unchanged. This is the prepared source for
the connected BioPlant traversal; see `BIOPLANT_NATIVE.md`.

The prior driver's CIRCLET-on-Alys failure is retained under
`outfit/incompatible-circlet`; the game correctly enforced the cartridge's
usable-by mask. That test-player error did not justify weakening equipment
eligibility. `EQUIP_SCOUT.md` corrects the earlier mistaken claim that the
original game could not re-equip a left-hand weapon.

## Connected BioPlant elevator checkpoint (2026-09-15)

The equipped-Zema source advances through 64 ordinary training wins, a paid
inn, and two purchased CRBN-SUITs for Alys and Chaz. The earned party is
levels 7/6/6/7. The entrance, alarm and elevator scenes now execute through
normal interactions and movement, with live graphics and collision changes.
CROSSCUT, learned at level six, now executes its two independent damage hits.

`build/native-bioplant/campaign/route/saved-map-0A7.json` records the normal
SAVE at tick 3747, map `$A7`, cell `(68,66)`, 495 meseta and full HP/TP.
Its preserved SRAM copy has SHA256
`88f62eceab4d1b777e824f7dd7dc0577183a8bf8716de73b0583bfb2595d78b3`.
Fresh title CONTINUE, one Down and SAVE 2 pass in `continued-A7/`. Only
logical payload Y at `$309` changes from 32 to 48; all other gameplay bytes
and the source save remain intact, and slot/checksum headers validate.
The restored elevator and four-person party capture was inspected.

This checkpoint uses library SHA256
`651630d96cce6962da07976da67a7de7692a7688fa64c617c7eda4c17fcd2ebc`.
All 896 Rust workspace tests and strict Clippy pass. Detailed source
transcriptions, alarm pixel checks, failures and Rika fixture scope are in
`BIOPLANT_NATIVE.md`.

Connected Rika progression remains open. Both the onward run and a title
restart lose party members in Gicefalgue ambushes. The full-health `$A7` save
above is the current verified continuation source; the separate Rika fixture
does not replace that missing campaign proof.

### STATE/ORDER and the Gryz-leading checkpoint

STATE/ORDER now supports picks, undo, cancellation and automatic completion
with the last member. Runtime tests preserve character records and field
positions, verify the saved order and prove that enemy targeting uses it.
The full suite is now 898 passing Rust tests; strict Clippy passes.

`build/native-order/native-final/route/route.json` uses ordinary input to
make Gryz/Alys/Chaz/Hahn from the healthy `$A7` checkpoint and save it. The
cartridge emulator accepts that original native SRAM directly and produces
the same ordering. Four bounded ORDER-menu comparisons have RMSE 0 with no
masked pixels; broader camp/camera parity remains outside those checks.

The resulting source is `build/native-order/native-final/saves/slot_1.sram`,
SHA256 `f3e721e68d5af82fcbdc4f3b033f7d982f725b32902db119f6a682bf8a2819c5`.
Only the four party-order bytes differ from the old healthy source. Fresh
CONTINUE, one Down and SAVE 2 pass with no other gameplay changes. Source
details, receipts and limits are in `PARTY_ORDER.md` and `BIOPLANT_NATIVE.md`.

The onward trial in `build/native-order/campaign/` wins six encounters and
reaches `$A9`, but Hahn dies in the sixth fight and ordinary recovery needs
revival. That run is failed, unsaved evidence. The full-health ordered `$A7`
save remains the verified continuation source; Rika's connected join and
escape are still unfinished.

The follow-up bounded run in `build/native-order/recovery/` adds automatic
ANTI before field healing. One connected battle, two cures at exactly 2 TP
each, ordinary recovery, SAVE and fresh CONTINUE all pass. It preserves a
separate poison-free `$A7 (75,61)` save; full details and capture limits are
in `BIOPLANT_NATIVE.md`. The original full-health ordered source is untouched.
