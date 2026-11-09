# `Event_GetAndRunDialogue*` caller census

`Event_ZioNurvus` ended its presentation with a `RunDialogueResume`. The
cartridge's last op there is `moveq #$B, d0 / jsr Event_GetAndRunDialogue2`
(`ps4.asm:148622-148623`), entry `$0B` of the map's own tree. A resume reopens
`Saved_Dialogue_Addr`, which in play is whatever dialogue ran last and after a
load is nothing, so the engine halted the campaign runner at Zio's trigger.
The audit of that routine's callers found every one on the wrong window; the
census of its four siblings found eight more. This document is both audits and
the census that keeps them true.

## The routines and their windows

The five `Event_GetAndRunDialogue*` entry points disassembled from the image
(`$5AC66`, `$5ACDC`, `$5AD32`, `$5ADA2`, `$5ADF8`) start the same way: `jsr
GetDialogueByID` (`$59164`, entry `d0` of `Current_Dialogue_Tree`), create
window 1, run the text and (all but the fourth) wait for a button. What differs
is the exit, read from the image (`$5AC66..$5AE73`), and each exit is one
`DialogueWindow` variant (`rust/psiv-core/src/scene_types.rs`):

| Routine | Image | Exit | Port window |
|---|---|---|---|
| `Event_GetAndRunDialogue` | `$5AC66` | `Panel_DestroyAll` (`$5AE74`), destroy the second window if `$FFFFECA4` bit 0 is set, destroy window 1, reload the map chunks (`Map_LoadChunks`, `$51ADC`) | `Standard` |
| `Event_GetAndRunDialogue2` | `$5ACDC` | `moveq #0, d0` then `move.b d0, Panel_Num`, `move.b d0, Windows_Opened_Num`, `move.b d0, $FFFFECA4`; `rts`. No destroy and no chunk reload: the text window and every panel stay on screen, forgotten rather than closed | `Retained`, written `retained(entry)` |
| `Event_GetAndRunDialogue3` | `$5AD32` | as the first, with `Panel_DestroyAll` omitted: the windows close, the panels stay | `Cutscene3`, written `cutscene3(entry)` |
| `Event_GetAndRunDialogue4` | `$5ADA2` | `RunText3` (`$6AD00`) instead of `RunText`, no button wait, no `Panel_DestroyAll`; windows and chunks as the first | `Ending` |
| `Event_GetAndRunDialogue5` | `$5ADF8` | as the third, then `PalFadeOut_ClrSpriteTbl` (`$4223E`) and `Panel_DestroyAll` | `Cutscene` |

`Cutscene5` is `Event_RunDialogue5`'s resume (`RunDialogueResumeWithWindow`)
and `EndingIntro` an inline `RunText3` window; neither is a `Get` entry. The
Godot presenter destroys panels on close only for `Standard`, `Cutscene` and
`Cutscene5`, so `Retained` and `Cutscene3` leave them up as the cartridge does.

(The clone comments the `SFXID_Selection` store out of several of these; the
image has it in all but the fourth, `13FC 00F3 FFFF500A` at the second
instruction. The port is silent on dialogue open everywhere, which is a separate
gap.)

**The rule:** a scene runs a dialogue in the window of the routine its bytes
call, with the entry `d0` carries there. A `Dialogue2` call in particular is an
ordinary `RunDialogue` of a literal entry in the *current* tree followed by a
window that is never destroyed. It is not a resume, it does not read
`Saved_Dialogue_Addr`, and it never ends a scene on a clean map: every one of
its fourteen callers is followed by a battle, a map change or another scene.

## The census

The image has 155 `jsr`/`jmp` to `Event_GetAndRunDialogue` (134 `jsr`, 21
`jmp` tail calls), 14 to the second, 8 to the third, 3 to the fourth and 23 to
the fifth; and 24 inline `jsr GetDialogueByID` inside transcribed scenes' bytes
(the scene's own window code: `Cutscene_AlysWounded`, `Cutscene_ZioDefeated`,
`Cutscene_CrashLaanding`'s entry `$03`, `Cutscene_Ending`'s first window, the
title and ending text plates, the Hunter's Guild lines). Each caller is tied to
the scene whose documented range holds it ([scene README](README.md) and the
scene documents' `Retail bytes`, each range belonging to the section heading it
sits under).

The entry is every literal `d0` reaches the call with: the `moveq`/`move.b`/
`move.w #n, d0` just before it (or before a `move.b #1`/`clr.b` of
`Render_Sprites_In_Cutscenes`, the only instructions the image puts between),
and each such load followed at once by a `bra` that lands on the call.
`Event_RajaSick` reaches its `jmp` at `$070C72` with `$5D`, `$5E` and `$5F`,
`Event_Gyuna` and `Event_MeeseClinicSickWoman` with two each. A call with no
literal path is *computed*: `Event_AlysFound` reads `$14` of Alys's object,
`Event_CatchingRocky` its per-map row (`loc_72866`), `Event_HuntersGuild` the
job's line, `Event_PickingFifthCharacter` its row's `$5(a0)` (`1028 0005` at
`$071F30`, then `addq.b #2` / `#3`, `5400` / `5600`, before `$071F44` and
`$071F86`). The port unrolls those or computes them in a table-driven op
(`GuildOp::RunLine`, `FifthOp::RunLine`, `FifthOp::RunLeaverLine`, each on the
`Standard` window), so a computed row only asks that the scene run its
routine's window at least once.

**Checked:** for each scene and routine, the entries its rows give are exactly
the entries it runs in that routine's window, counted (a second
`retained(0x3A)` in `Event_DarkForce2` fails); an inline row's entry may run in
any window; no scene runs a window none of its callers asks for.

### Findings

- **Fourteen of fourteen `Dialogue2` callers were wrong** (the audit below):
  one resume, nine standard windows, and four with no scene; one control-flow
  mismatch on the way (`Event_SavingKyra`'s on-foot branch).
- **Seven of eight `Dialogue3` callers were on the wrong window:** six on
  the `Dialogue5` one (`Cutscene`) and `Cutscene_AeroPrism` (`$077F8C`) on the
  standard one; `Cutscene_Rykros` (`$0781D8`) was the only `Cutscene3`. All
  seven now run `cutscene3(entry)`:
  `Cutscene_DemiRescue` `$074A92` `$42`,
  `Cutscene_SpaceshipSabotage` `$07643C` `$02`,
  `Cutscene_MeetingKyra` `$0777D8` `$28`,
  `Cutscene_LashiecDefeated` `$077A8C` `$38` (whose explicit
  `PanelDestroyAll` after the line the `Cutscene` window had pre-empted),
  `Cutscene_DarkForce2Defeated` `$077C70` `$3B`,
  `Cutscene_AeroPrism` `$077F8C` `$01`,
  `Cutscene_Elsydeon` `$0785B2` `$31`.
- **One `Dialogue` caller was on the `Dialogue5` window:**
  `Cutscene_MeetingKyra` `$0777A6` `$27`, now `standard(0x27)`.
- **`Dialogue4` and `Dialogue5` agree:** 3 and 23 callers, each scene on
  `Ending` and `Cutscene` with the image's entries.
- **`Dialogue` otherwise agrees:** all 155 of its callers have scenes and
  each scene runs exactly their entries on `Standard` (the last six were
  transcribed with #283's events after the audit found them without one).

## Per-scene census

Routine names: D1 to D5 are `Event_GetAndRunDialogue` to
`Event_GetAndRunDialogue5`; *inline* is `jsr GetDialogueByID` in the scene's
own window code. This table and `DIALOGUE_CALLERS` in
`rust/psiv-core/src/scenes/dialogue_callers.rs` are generated from the same
image read; the census fails on any address missing here.

| Scene | Routine (window) | Callers: `jsr`/`jmp` address, entry |
|---|---|---|
| `Event_AlysFound` | D1 (`Standard`) | `$06B32C` computed |
| `Event_MeetingHahn` | D1 (`Standard`) | `$06B438` `$11` |
| `Event_MachineCenterAppearing` | D1 (`Standard`) | `$06B564` `$0B` |
| `Event_BasementContainers` | D1 (`Standard`) | `$06BC46` `$0F` |
| `Event_MeetingSaya` | D1 (`Standard`) | `$06BC6C` `$0E` |
| `Event_AiedoManCrushOnAlys` | D1 (`Standard`) | `$06BE90` `$3C` |
| `Event_SuspicionOnPrincipal` | D1 (`Standard`) | `$06BEEA` `$6E` |
| `Event_BioPlantAlarm` | D1 (`Standard`) | `$06C29E` `$0A` |
| `Event_AiedoManSavings` | D1 (`Standard`) | `$06D46A` `$3D` |
| `Event_GirlsSneakingOut` | D1 (`Standard`) | `$06D53C` `$3E` |
| `Event_FaintingPriest` | D1 (`Standard`) | `$06D62E` `$45` |
| `Event_AfterIgglanova` | D1 (`Standard`) | `$06D6CA` `$12` |
| `Event_PrincipalConfession` | D1 (`Standard`) | `$06D7A4` `$13` |
| `Event_RuneFlaeli` | D1 (`Standard`) | `$06D830` `$26` |
| `Event_AlshlineFound` | D1 (`Standard`) | `$06DBDA` `$27` |
| `Event_WreckageEngine` | D1 (`Standard`) | `$06DBF4` `$48`, `$06DBFC` `$47` |
| `Event_CancellerReminder` | D1 (`Standard`) | `$06DEB0` `$0C` |
| `Event_GettingLandRover` | D1 (`Standard`) | `$06DEC0` `$09`, `$06E0A8` `$0C` |
| `Event_MonsenEarthquake` | D1 (`Standard`) | `$06E102` `$40` |
| `Event_PlateSysEngine` | D1 (`Standard`) | `$06E828` `$41` |
| `Event_RuneLadaeTower` | D1 (`Standard`) | `$06E9E0` `$06` |
| `Event_PsycoWandChest` | D1 (`Standard`) | `$06EA18` `$07`, `$06EACC` `$08` |
| `Event_ZioFortBarrier` | D1 (`Standard`) | `$06EC70` `$44`, `$06EDC4` `$45`, `$06EE10` `$4A` |
| `Event_ZioFanatic` | D1 (`Standard`) | `$06EE58` `$49`, `$06EEB8` `$1F` |
| `Event_MeetingDorin` | D1 (`Standard`) | `$06EEEE` `$18` |
| `Event_TonoeBasementDoor` | D1 (`Standard`) | `$06F1A6` `$24` |
| `Event_ZioNurvus` | D2 (`Retained`) | `$06F3D4` `$0B` |
| `Event_ChazHouse` | INLINE (inline) | `$06FA42` `$31` |
| `Event_ChazHouse` | D1 (`Standard`) | `$06FA50` `$31` |
| `Event_KuranArrival` | D1 (`Standard`) | `$06FAD6` `$04` |
| `Event_NearDarkForce1` | D1 (`Standard`) | `$06FAE8` `$05` |
| `Event_DarkForce1` | D2 (`Retained`) | `$06FAFA` `$06` |
| `Event_Juza` | D2 (`Retained`) | `$06FB42` `$48` |
| `Event_OutsideRajaTemple` | D1 (`Standard`) | `$06FC84` `$06` |
| `Event_TylerGraveOpening` | D1 (`Standard`) | `$06FCB0` `$1E`, `$06FE16` `$22` |
| `Event_PersistentEsperGuards` | D1 (`Standard`) | `$06FE2A` `$2E` |
| `Event_EclipseTorchStolen` | D1 (`Standard`) | `$06FE8A` `$32` |
| `Event_EclipseTorchUsed` | D1 (`Standard`) | `$070460` `$2A` |
| `Event_MeetingLeRoof` | D1 (`Standard`) | `$070566` `$00` |
| `Event_MuskCatsGuarding` | D1 (`Standard`) | `$0706A0` `$1C` |
| `Event_MuskCatElder` | D1 (`Standard`) | `$0706DC` `$2E`, `$0706E4` `$2D` |
| `Event_PenguinOwner` | D1 (`Standard`) | `$07073A` `$12` |
| `Event_CarnivorousTrees` | D1 (`Standard`) | `$070820` `$34` |
| `Event_SavingKyra` | D1 (`Standard`) | `$070902` `$34` |
| `Event_SavingKyra` | D2 (`Retained`) | `$070936` `$26` |
| `Event_DarkForce2` | D2 (`Retained`) | `$070978` `$3A` |
| `Event_EsperGuardPermission` | D1 (`Standard`) | `$0709E8` `$00`, `$0709F0` `$02`, `$070A0A` `$01` |
| `Event_DarkForce3Defeated` | D1 (`Standard`) | `$070A2C` `$02` |
| `Event_InnerSanctGuard` | D1 (`Standard`) | `$070A66` `$35` |
| `Event_InnerSanctGuardBeforeElsydeon` | D1 (`Standard`) | `$070A8E` `$36` |
| `Event_ClmCenterAfterBattle` | D1 (`Standard`) | `$070ADC` `$31` |
| `Event_DElmLars` | D1 (`Standard`) | `$070AEE` `$33` |
| `Event_AfterDElmLarsBattle` | D1 (`Standard`) | `$070B0E` `$34` |
| `Event_AirCastleArrival` | D1 (`Standard`) | `$070B20` `$35` |
| `Event_XeAThoulBeforeBattle` | D2 (`Retained`) | `$070B32` `$36` |
| `Event_AirCastleFakeChest` | D2 (`Retained`) | `$070B58` `$3C` |
| `Event_RajaSick` | D1 (`Standard`) | `$070C72` `$5D`/`$5E`/`$5F`, `$070C7C` `$60` |
| `Event_Gyuna` | D1 (`Standard`) | `$070C94` `$31`/`$32` |
| `Event_LashiecAppearance` | D2 (`Retained`) | `$070E1E` `$37` |
| `Event_DeVars` | D1 (`Standard`) | `$07144C` `$03` |
| `Event_SaLews` | D1 (`Standard`) | `$07146C` `$01` |
| `Event_AngerTowerAlys` | D1 (`Standard`) | `$07148C` `$07` |
| `Event_AngerTowerAlys` | D2 (`Retained`) | `$071538` `$08` |
| `Event_ReFaze` | D1 (`Standard`) | `$07157C` `$09` |
| `Event_DeVarsDefeated` | D1 (`Standard`) | `$07175C` `$04` |
| `Event_SaLewsDefeated` | D1 (`Standard`) | `$071824` `$02` |
| `Event_HuntersGuild` | D1 (`Standard`) | `$071946` `$12`, `$07196A` computed, `$07199C` computed, `$0719E8` computed |
| `Event_HuntersGuild` | INLINE (inline) | `$071A24` `$0F`, `$071B4E` `$10`, `$071BAC` `$13`, `$071C38` computed, `$071C88` `$12`, `$071CCA` computed, `$071CFA` `$11` |
| `Event_RuneHealingChaz` | D1 (`Standard`) | `$071E58` `$2B` |
| `Event_PickingFifthCharacter` | D1 (`Standard`) | `$071F34` computed, `$071F44` computed, `$071F86` computed |
| `Event_Phonon` | D1 (`Standard`) | `$072320` `$4E`, `$072406` `$4F` |
| `Event_Hijammer` | D1 (`Standard`) | `$072446` `$0A` |
| `Event_Burstroc` | D1 (`Standard`) | `$0724C2` `$08` |
| `Event_PosiBolt` | D1 (`Standard`) | `$07253E` `$06` |
| `Event_RanchOwner` | D1 (`Standard`) | `$0725DE` `$20` |
| `Event_RanchOwnerAfterBattle` | D1 (`Standard`) | `$072690` `$21` |
| `Event_RockyFound` | D1 (`Standard`) | `$0726B6` `$2C` |
| `Event_CatchingRocky` | D1 (`Standard`) | `$07273A` computed, `$0727E8` computed, `$07285E` `$46` |
| `Event_RockyOwner` | D1 (`Standard`) | `$07288A` `$5E` |
| `Event_PiataDormOwner` | D1 (`Standard`) | `$0728B0` `$6C` |
| `Event_MissingStudentFound` | D1 (`Standard`) | `$0728E0` `$52`, `$0728E8` `$4C`, `$072964` `$4D`, `$0729DE` `$53` |
| `Event_OldManNearMissingStudent` | D1 (`Standard`) | `$072A10` `$4F`, `$072A20` `$53` |
| `Event_MissingStudentInBed` | D1 (`Standard`) | `$072A48` `$50`, `$072A50` `$54`, `$072ACA` `$5B` |
| `Event_TallasMom` | D1 (`Standard`) | `$072AE8` `$2D` |
| `Event_TallasMomAfterRescue` | D1 (`Standard`) | `$072B0E` `$2E` |
| `Event_InsideMonsenHole` | D1 (`Standard`) | `$072B1E` `$24` |
| `Event_FractOozeFound` | D2 (`Retained`) | `$072B2E` `$2F` |
| `Event_TallasRescued` | D1 (`Standard`) | `$072B54` `$27` |
| `Event_LostGirlsMother` | D1 (`Standard`) | `$072C20` `$13` |
| `Event_GirlPrison` | D1 (`Standard`) | `$072C46` `$5F` |
| `Event_GirlsBail` | D1 (`Standard`) | `$072C6C` `$50` |
| `Event_PayingGirlsBail` | D1 (`Standard`) | `$072CAE` `$51` |
| `Event_GirlsBailedOut` | D1 (`Standard`) | `$072CDE` `$61`, `$072D3E` `$0A` |
| `Event_DyingBoyFather` | D1 (`Standard`) | `$072D5C` `$39` |
| `Event_FatherAfterBoyRecovery` | D1 (`Standard`) | `$072D84` `$3A` |
| `Event_DyingBoy` | D1 (`Standard`) | `$072DDE` `$33`, `$072FA6` `$3B`, `$072FAE` `$3E` |
| `Event_MeetingSekreas` | D1 (`Standard`) | `$072FCC` `$3C` |
| `Event_KingRappy` | D2 (`Retained`) | `$072FEA` `$36` |
| `Event_KingRappy` | D1 (`Standard`) | `$073012` `$35` |
| `Event_KingRappyDefeated` | D1 (`Standard`) | `$07302E` `$37`, `$0730EC` `$38` |
| `Event_ZemaOldMan` | D1 (`Standard`) | `$073156` `$6C` |
| `Event_ZemaOldManAfterMission` | D1 (`Standard`) | `$07317E` `$6D` |
| `Event_VahalFortEntrance` | D1 (`Standard`) | `$073190` `$00` |
| `Event_VahalFortMidway` | D1 (`Standard`) | `$0731A2` `$03` |
| `Event_DaughterTerminal` | D2 (`Retained`) | `$0731B4` `$04` |
| `Event_VahalFortBarrier` | D1 (`Standard`) | `$0731E8` `$01`, `$0731FA` `$02` |
| `Event_DominatorsDefeated` | D1 (`Standard`) | `$0732C8` `$05` |
| `Event_WeaponPlantArrival` | D1 (`Standard`) | `$0733BC` `$07` |
| `Event_MeeseClinicSickWoman` | D1 (`Standard`) | `$07341A` `$3C`/`$3D` |
| `Event_PenguinFeedStolen` | D1 (`Standard`) | `$0736B4` `$10` |
| `Event_Pennant` | D1 (`Standard`) | `$0736EC` `$64` |
| `Event_WoodCarving` | D1 (`Standard`) | `$073756` `$66` |
| `Event_EnterAngerTower` | D1 (`Standard`) | `$0737CE` `$05`, `$073822` `$06` |
| `Event_SoldiersTempleCaveDialogue1` | D1 (`Standard`) | `$073882` `$0E` |
| `Event_SoldiersTempleCaveDialogue2` | D1 (`Standard`) | `$073894` `$0F` |
| `Event_SoldiersTempleReached` | D1 (`Standard`) | `$0738A6` `$10` |
| `Event_AeroPrismFound` | D1 (`Standard`) | `$0738B8` `$11` |
| `Event_PiataGuardsReprimand` | D1 (`Standard`) | `$073932` `$10` |
| `Event_GameStart` | D1 (`Standard`) | `$073960` `$36`, `$073A78` `$37`, `$073AF8` `$38`, `$073BD6` `$39` |
| `Event_GameStart` | INLINE (inline) | `$073CA8` `$3A`, `$073CC8` `$3B`, `$073CE8` `$3C`, `$073D08` `$3D`, `$073D5C` `$3E`, `$073D7C` `$3F`, `$073D9C` `$40`, `$073DBC` `$41` |
| `Event_PiataChazAlone` | D1 (`Standard`) | `$073ED0` `$6D` |
| `Cutscene_PiataPrincipal` | D5 (`Cutscene`) | `$073F04` `$17` |
| `Cutscene_ProfHolt` | D5 (`Cutscene`) | `$073F3E` `$66` |
| `Cutscene_MeetingRune` | D5 (`Cutscene`) | `$073FCE` `$03` |
| `Cutscene_Dorin` | D5 (`Cutscene`) | `$074048` `$25` |
| `Cutscene_Alshline` | D5 (`Cutscene`) | `$074362` `$67` |
| `Cutscene_Alshline` | D2 (`Retained`) | `$074502` `$68` |
| `Cutscene_ZemaIgglanovaDefeated` | D5 (`Cutscene`) | `$07458E` `$69` |
| `Cutscene_MeetingRika` | D5 (`Cutscene`) | `$074618` `$00`, `$0749C4` `$03`, `$074A28` `$04` |
| `Cutscene_MeetingRika` | D1 (`Standard`) | `$0746FC` `$01`, `$07480E` `$02` |
| `Cutscene_DemiRescue` | D3 (`Cutscene3`) | `$074A92` `$42` |
| `Cutscene_AlysWounded` | INLINE (inline) | `$074B86` `$43` |
| `Cutscene_AlysWounded` | D1 (`Standard`) | `$074D6C` `$2C`, `$075052` `$2D` |
| `Cutscene_PsycoWand` | D5 (`Cutscene`) | `$075214` `$05` |
| `Cutscene_PsycoWand` | D1 (`Standard`) | `$07534E` `$2E` |
| `Cutscene_ZioDefeated` | INLINE (inline) | `$075A76` `$00` |
| `Cutscene_MeetingWren` | D5 (`Cutscene`) | `$07600C` `$01` |
| `Cutscene_SpaceshipSabotage` | D3 (`Cutscene3`) | `$07643C` `$02` |
| `Cutscene_CrashLaanding` | INLINE (inline) | `$076622` `$03` |
| `Cutscene_CrashLaanding` | D5 (`Cutscene`) | `$076774` `$07`, `$076B40` `$08` |
| `Cutscene_Landale` | D5 (`Cutscene`) | `$07717E` `$24` |
| `Cutscene_DarkForce1Defeated` | D5 (`Cutscene`) | `$0771F6` `$07`, `$0772D0` `$08` |
| `Cutscene_RajaSick` | D1 (`Standard`) | `$077364` `$59`, `$0774AE` `$62`, `$07769E` `$63` |
| `Cutscene_MeetingKyra` | D1 (`Standard`) | `$0777A6` `$27` |
| `Cutscene_MeetingKyra` | D3 (`Cutscene3`) | `$0777D8` `$28` |
| `Cutscene_LutzRevelation` | D1 (`Standard`) | `$077898` `$2A`, `$0779B0` `$2F` |
| `Cutscene_FindingAirCastle` | D5 (`Cutscene`) | `$077A52` `$18` |
| `Cutscene_LashiecDefeated` | D3 (`Cutscene3`) | `$077A8C` `$38` |
| `Cutscene_LashiecDefeated` | D1 (`Standard`) | `$077BC4` `$39` |
| `Cutscene_DarkForce2Defeated` | D3 (`Cutscene3`) | `$077C70` `$3B` |
| `Cutscene_GumbiousBishop` | D1 (`Standard`) | `$077DC8` `$33`, `$077E68` `$17` |
| `Cutscene_MeetingSeth` | D5 (`Cutscene`) | `$077EDC` `$00` |
| `Cutscene_AeroPrism` | D3 (`Cutscene3`) | `$077F8C` `$01` |
| `Cutscene_Rykros` | D3 (`Cutscene3`) | `$0781D8` `$16` |
| `Cutscene_LeRoofAgain` | D1 (`Standard`) | `$078424` `$0B` |
| `Cutscene_LeRoofAgain` | D5 (`Cutscene`) | `$078496` `$0C` |
| `Cutscene_BeforeElsydeonCave` | D5 (`Cutscene`) | `$078520` `$30` |
| `Cutscene_Elsydeon` | D3 (`Cutscene3`) | `$0785B2` `$31` |
| `Cutscene_Elsydeon` | D5 (`Cutscene`) | `$078A1E` `$32` |
| `Cutscene_Reunion` | D5 (`Cutscene`) | `$078ADC` `$00`, `$078C02` `$01` |
| `Cutscene_ProfoundDarkness` | D2 (`Retained`) | `$078E9C` `$03` |
| `Cutscene_Ending` | INLINE (inline) | `$078F70` `$09`, `$079C18` `$12`, `$079C36` `$13`, `$079C54` `$14`, `$079C72` `$15` |
| `Cutscene_Ending` | D4 (`Ending`) | `$079084` `$0A`, `$079114` `$0B`, `$0798AC` `$0C` |

## The `Dialogue2` audit

The image has exactly fourteen `jsr $5ACDC.l`. The first column is the
`ps4.asm` line of the `jsr` (the clone's labels, used for navigation only); the
second is the address in the US image, which is the authority. "Tree" is what
`GetDialogueByID` indexes at the call: the tree the scene loads itself, or else
the tree bound to the map the scene runs on (`runtime-pack/maps/*.json`
`dialogue_tree`; see the
[generated selection census](DIALOGUE_TREE_SELECTION.md#census-and-issue-42)).
Entry text is the first line in
`runtime-pack/dialogue/trees.json` for that tree and entry.

| # | `ps4.asm` | Image `jsr` | Scene (event) | Entry | Tree at the call | Entry text | Port before | Port after |
|---:|---:|---|---|---:|---|---|---|---|
| 1 | 148623 | `$06F3D4` | `Event_ZioNurvus` (`$34`) | `$0B` | map: Nurvus B4 Part2 (`$D3`), tree 36 | "Well, well... You've made it here" | **`RunDialogueResume`** (mismatch) | `retained(0x0B)` |
| 2 | 149136 | `$06FAFA` | `Event_DarkForce1` (`$3F`) | `$06` | map: Kuran F3 (`$198`), tree 35 | "What on earth? This feeling of oppression" | `RunDialogue` entry 6, `Standard` window | `retained(6)` |
| 3 | 149154 | `$06FB42` | `Event_Juza` (`$40`) | `$48` | map: Zio Fort Juza room (`$87`), tree 13 | "You are... I am Juza." | `RunDialogue` entry `$48`, `Standard` window | `retained(0x48)` |
| 4 | 150038 | `$070936` | `Event_SavingKyra` (`$4D`) | `$26` | scene: `DialogueTree37` loaded at `$070928` | "Look! Someone is being attacked!" | `Standard` window; **on-foot branch landed on op 8, replaying dismount dialogue `$34`** (mismatch) | `retained(0x26)`; on-foot branch to op 9 (`beq.w loc_70908`, `ps4.asm:149987-149988`) |
| 5 | 150056 | `$070978` | `Event_DarkForce2` (`$4E`) | `$3A` | map: Garuberk Tower Part7 (`$19F`), tree 22 | "What's that? Is that Dark Force?!" | `Standard` window | `retained(0x3A)` |
| 6 | 150173 | `$070B32` | `Event_XeAThoulBeforeBattle` (`$59`) | `$36` | map: Air Castle Xe A Thoul room (`$184`), tree 22 | "Ha, ha, ha, ha! You've come!" | `Standard` window | `retained(0x36)` |
| 7 | 150182 | `$070B58` | `Event_AirCastleFakeChest` (`$5A`) | `$3C` | map: Air Castle Inner B1 Part3 (`$187`), tree 22 | "Oh no! This torch is a fake!" | `Standard` window | `retained(0x3C)` |
| 8 | 150364 | `$070E1E` | `Event_LashiecAppearance` (`$5D`) | `$37` | map: Air Castle Inner B1 Part3 (`$187`), tree 22 | "You've finally come... Lutz!" | `Standard` window | `retained(0x37)` |
| 9 | 153841 | `$074502` | `Cutscene_Alshline` (`$8005`) | `$68` | scene: `DialogueTree3`, reloaded at `$0744EE` before the call | "How are we going to get in with that creature blocking the way" | `Standard` window | `retained(0x68)` |
| 10 | 157969 | `$078E9C` | `Cutscene_ProfoundDarkness` (`$8020`) | `$03` | map: The Edge Part9 (`$108`), tree 42 | "...Is this the shape The Profound Darkness has taken?" | `Standard` window | `retained(3)` |
| 11 | 150750 | `$071538` | `Event_AngerTowerAlys` (`$62`) | `$08` | map: Anger Tower F2 (`$FE`), tree 41 | | no scene | `retained(8)`; scene [114](114_AngerTowerAlys.md) |
| 12 | 152563 | `$072B2E` | `Event_FractOozeFound` (`$7D`) | `$2F` | current tree of its dialogue trigger | | no scene | `retained(0x2F)`; scene [95](95_FractOozeFound.md) |
| 13 | 152871 | `$072FEA` | `Event_KingRappy` (`$88`) | `$36` | current tree of its dialogue trigger | | no scene | `retained(0x36)`; scene [96](96_KingRappy.md) |
| 14 | 152977 | `$0731B4` | `Event_DaughterTerminal` (`$8F`) | `$04` | current tree of its dialogue trigger | | no scene | `retained(0x04)`; scene [97](97_DaughterTerminal.md) |

Findings:

- **One resume mismatch (the Zio-at-Nurvus defect above):** row 1. The census test
  `no_scene_resumes_before_it_opens_a_dialogue` extends that to every
  registered scene: none opens with a resume now.
- **One control-flow mismatch the audit surfaced (row 4):**
  `BranchIfVehicle { if_on_foot: 8 }` jumped onto the dismount dialogue `$34`,
  so a party on foot replayed "It won't work. Even with this..." before the
  attack. The cartridge's `beq.w loc_70908` skips to the red-alert music.
- **Ten of ten window-variant mismatches (rows 1 to 10):** nine transcribed
  callers used the standard window, which destroys the panels the cartridge
  leaves up, and the tenth was the resume of row 1.
- **Every caller has a scene.** Rows 11 to 14 are transcribed
  (row 11, `Event_AngerTowerAlys`, is [114](114_AngerTowerAlys.md)).

### The `Dialogue2` callers' spans

The local image matches the [accepted US ROM hash](../DEVELOPMENT.md#prepare-the-local-pack).
`psiv_tools/newgame.py::event_ptrs` resolves `EventPtrs` to `$05A2B4` from the
retail dispatcher bytes. The pointer slots and adjacent entries establish the
following spans; labels and fork build-address comments supply no authority.
All four now have scenes (rows 11 to 14).

| Event | Pointer slot -> routine | EventPtrs-derived span (inclusive) | Entry load and `jsr $5ACDC` bytes |
|---|---|---|---|
| AngerTowerAlys `$62` | `$05A43C` -> `$07148A` | `$07148A..$071579` (includes direction data) | `$071536`: `7008 4EB9 0005 ACDC` |
| FractOozeFound `$7D` | `$05A4A8` -> `$072B2C` | `$072B2C..$072B51` | `$072B2C`: `702F 4EB9 0005 ACDC` |
| KingRappy `$88` | `$05A4D4` -> `$072FDC` | `$072FDC..$073017` | `$072FE8`: `7036 4EB9 0005 ACDC` |
| DaughterTerminal `$8F` | `$05A4F0` -> `$0731B2` | `$0731B2..$0731D9` | `$0731B2`: `7004 4EB9 0005 ACDC` |

## The resume census

The five `Event_RunDialogue*` entries (`$5AC6C`, `$5ACE2`, `$5AD38`, `$5ADA8`,
`$5ADFE`; R1 to R5) are their `Get` routines' bodies after the opening `jsr
GetDialogueByID`, six bytes in: they run the text already in `a0`, which the
`popdlg` before every one of their 120 calls reloads from
`Saved_Dialogue_Addr`, and they exit exactly as their routine does. R5's
window is the port's `Cutscene5`, the `Get5` exit reached through the resume.
Six more `popdlg`s open the scene's own window code instead (`jsr RunText`
before any resume call): inline resumes, in `Cutscene_PsycoWand` and
`Cutscene_Elsydeon`. Four `popdlg`s run neither and only peek at the saved
address (`Event_MeetingDorin`'s `cmpi.b #$FF, -1(a0)` and three like it).

**Checked:** each scene runs, per window, as many resumes as its bytes call
the resume entry whose exit that window is; the extra resumes it runs beyond
those are exactly its inline resumes. A bare `RunDialogueResume` counts in the
window the scene's last dialogue left (the runner keeps it); a resume that
must change window names it (`RunDialogueResumeWithWindow`). A window whose
`Get` caller has a computed entry may run more resumes than the bytes call,
since the port unrolls that call into a branch per entry (`Event_CatchingRocky`).
Resume calls a transcription skips are named in `UNTRANSCRIBED_RESUMES` with
the issue that transcribes them.

### Resume findings

- **Thirteen resumes were in the wrong window**, each a bare resume that kept
  the window before it. Now named: `Cutscene_Alshline` `$0743EA`,
  `Cutscene_MeetingRika` `$074750`, `Cutscene_AlysWounded` `$074C48`,
  `Cutscene_MeetingKyra` `$077810`, `Cutscene_LutzRevelation` `$0779EA`,
  `Cutscene_LashiecDefeated` `$077B28`, `Cutscene_DarkForce2Defeated`
  `$077CD6` and `Cutscene_ZioDefeated` `$075DF8` on `Cutscene5`;
  `Cutscene_DemiRescue` `$074B10`, `Event_PsycoWandChest` `$06EB6C`,
  `Cutscene_SpaceshipSabotage` `$0764C0` and `Cutscene_AeroPrism` `$0780A4` on
  `Retained`; `Cutscene_LeRoofAgain` `$078454` on `Cutscene3`; and
  `Cutscene_CrashLaanding`'s five (`$076C26` on) back on `Standard`.
- **Pages the transcriptions skipped,** so each later resume showed the page
  before the cartridge's: `Event_MeetingSaya` ran two of six (now all six,
  with `SFXID_Res` and `SFXID_HealTechCast`); `Cutscene_AlysWounded` two of
  the three after `$2C` (now three, with `SFXID_HealTechCast`);
  `Cutscene_PsycoWand` one of its last two; `Cutscene_DarkForce2Defeated`
  lacked its last page with the screen before it (panels `$116` and `$A8`,
  `MusicID_Explosion`, `MusicID_RedAlert`). The actors' moves and facings
  between those pages are still not drawn.
- **Still skipped,** with
  [#414](https://github.com/TusanHomichi/PSIV/issues/414):
  `Cutscene_ZioDefeated`'s Demi sequence (`$075B10..$075D72`, four pages)
  and `Cutscene_Elsydeon`'s tail after panel `$130` (one inline page).
  Several of these scenes' panels are not in the pack yet
  ([#413](https://github.com/TusanHomichi/PSIV/issues/413)).

| Scene | Routine (window) | Resumes: `jsr`/`jmp` or inline `popdlg` address |
|---|---|---|
| `Event_MeetingSaya` | R1 (`Standard`) | `$06BCE2`, `$06BD0C`, `$06BD6E`, `$06BD8A`, `$06BDA6`, `$06BDEC` |
| `Event_AiedoManCrushOnAlys` | R1 (`Standard`) | `$06BED0` |
| `Event_AiedoManSavings` | R1 (`Standard`) | `$06D48E` |
| `Event_FaintingPriest` | R1 (`Standard`) | `$06D6A6` |
| `Event_AfterIgglanova` | R1 (`Standard`) | `$06D6EE`, `$06D748` |
| `Event_RuneFlaeli` | R1 (`Standard`) | `$06D8FA`, `$06D9B2` |
| `Event_WreckageEngine` | R1 (`Standard`) | `$06DCD8`, `$06DDA4` |
| `Event_GettingLandRover` | R1 (`Standard`) | `$06DF1E`, `$06DFA6` |
| `Event_MonsenEarthquake` | R1 (`Standard`) | `$06E2CE`, `$06E318`, `$06E342`, `$06E378`, `$06E39A`, `$06E408`, `$06E47E`, `$06E4D0`, `$06E58C` |
| `Event_PlateSysEngine` | R1 (`Standard`) | `$06E8B6` |
| `Event_PsycoWandChest` | R2 (`Retained`) | `$06EB6C` |
| `Event_MeetingDorin` | R1 (`Standard`) | `$06F032`, `$06F04E`, `$06F0E8`, `$06F132` |
| `Event_TonoeBasementDoor` | R1 (`Standard`) | `$06F1C0`, `$06F224`, `$06F2DA` |
| `Event_TylerGraveOpening` | R1 (`Standard`) | `$06FE02` |
| `Event_EclipseTorchStolen` | R1 (`Standard`) | `$06FFC2`, `$0700AA`, `$070156` |
| `Event_ReFaze` | R1 (`Standard`) | `$07166E` |
| `Event_Phonon` | R1 (`Standard`) | `$0723F2` |
| `Event_Hijammer` | R1 (`Standard`) | `$07248C` |
| `Event_Burstroc` | R1 (`Standard`) | `$072508` |
| `Event_PosiBolt` | R1 (`Standard`) | `$072584` |
| `Event_CatchingRocky` | R1 (`Standard`) | `$0727B4` |
| `Event_DyingBoy` | R1 (`Standard`) | `$072EC0`, `$072F4C`, `$072F76` |
| `Event_KingRappyDefeated` | R1 (`Standard`) | `$07304E`, `$0730D0` |
| `Event_DominatorsDefeated` | R1 (`Standard`) | `$07334A` |
| `Event_EnterAngerTower` | R1 (`Standard`) | `$073870` |
| `Cutscene_Alshline` | R5 (`Cutscene5`) | `$0743EA` |
| `Cutscene_MeetingRika` | R5 (`Cutscene5`) | `$074750` |
| `Cutscene_DemiRescue` | R2 (`Retained`) | `$074B10` |
| `Cutscene_AlysWounded` | R5 (`Cutscene5`) | `$074C48` |
| `Cutscene_AlysWounded` | R1 (`Standard`) | `$074D96`, `$074E26`, `$074E40`, `$07506C`, `$07508A`, `$0750B4`, `$0750CE`, `$0750F8`, `$075116`, `$075134`, `$075162`, `$075180`, `$07519E` |
| `Cutscene_PsycoWand` | INLINE (`inline`) | `$0753B6`, `$075484`, `$07563E`, `$075706` |
| `Cutscene_PsycoWand` | R1 (`Standard`) | `$0758FE`, `$0759A8` |
| `Cutscene_ZioDefeated` | R1 (`Standard`) | `$075BA0` (#414), `$075C56` (#414), `$075C94` (#414) |
| `Cutscene_ZioDefeated` | R5 (`Cutscene5`) | `$075CD4` (#414), `$075DF8` |
| `Cutscene_SpaceshipSabotage` | R2 (`Retained`) | `$0764C0` |
| `Cutscene_CrashLaanding` | R1 (`Standard`) | `$076C26`, `$076C68`, `$076C9A`, `$076D50`, `$076D9A` |
| `Cutscene_RajaSick` | R1 (`Standard`) | `$07761C`, `$077700` |
| `Cutscene_MeetingKyra` | R5 (`Cutscene5`) | `$077810` |
| `Cutscene_LutzRevelation` | R5 (`Cutscene5`) | `$0779EA` |
| `Cutscene_LashiecDefeated` | R5 (`Cutscene5`) | `$077B28` |
| `Cutscene_DarkForce2Defeated` | R3 (`Cutscene3`) | `$077CA6` |
| `Cutscene_DarkForce2Defeated` | R5 (`Cutscene5`) | `$077CD6`, `$077D42` |
| `Cutscene_AeroPrism` | R3 (`Cutscene3`) | `$077FB8`, `$07803C` |
| `Cutscene_AeroPrism` | R2 (`Retained`) | `$0780A4` |
| `Cutscene_Rykros` | R3 (`Cutscene3`) | `$078240`, `$07826A` |
| `Cutscene_Rykros` | R5 (`Cutscene5`) | `$078282` |
| `Cutscene_LeRoofAgain` | R3 (`Cutscene3`) | `$078454` |
| `Cutscene_Elsydeon` | R3 (`Cutscene3`) | `$0786AC` |
| `Cutscene_Elsydeon` | INLINE (`inline`) | `$078740`, `$07892A` (#414) |
| `Cutscene_Ending` | R4 (`Ending`) | `$07914C`, `$079170`, `$079226`, `$079244`, `$0792F2`, `$079384`, `$0793D6`, `$0793F4`, `$07945A`, `$0794BE`, `$0794F6`, `$07958A`, `$079660`, `$079728`, `$0797A6`, `$079830`, `$07992C`, `$0799AC`, `$079A2C`, `$079AAC`, `$079B32`, `$079BB2` |

## Callers with no scene

None. The three events that had callers no scene held (`Event_FaintingPriest`
`$24`, `Event_ZioFanatic` `$31`, `Event_DyingBoy` `$86`; #283) have scenes now,
and their rows are in the table above. `UNTRANSCRIBED`
(`rust/psiv-core/src/scenes/dialogue_callers_image.rs`) is empty. A caller the
image gains outside every documented range fails the census until it is listed
there, with its entries and owning issue, and in a table here that gives the
`EventPtrs`-derived span and the caller's loader bytes; a lane that
transcribes it moves its rows into `DIALOGUE_CALLERS` and runs them on
`Standard`, and the census names the row to add.

## The party move in `Event_CarnivorousTrees` and `Event_SavingKyra`

The US image at `$070826..$070847` ([CarnivorousTrees](53_CarnivorousTrees.md))
and `$07093C..$07095D` ([SavingKyra](54_SavingKyra.md)) loads the leader's Y
(`lea Character_1, a4` `49F8 C000`, `move.w $34(a4), d0` `302C 0034`), adds
`$20` as a word (`0640 0020`), calls `CalcPartyNumber` (`$05A6A4`, the count of
`Current_Party_Slots` up to the first empty one), then writes that same Y to
each counted party object: `3941 0034` (`move.w d1, $34(a4)`) at `$07083C` and
`$070952`, with `$40` object strides and `dbf`. They are immediate position
writes; X is untouched, and the event goes straight on to its battle request.

**Resolved:** both scenes ran a leader-only `MoveActorOffset { dy: $20, wait:
true }`, a walk the followers trailed. No existing op writes every party
object's Y from the leader's (`PlaceActor` is one actor at an absolute
position, `OverlapCharacters` copies X too), so the scenes now run
`SceneOp::PlacePartyY { dy: 0x20 }` (`scene_runner/mechanics.rs`): every
counted member's Y becomes the leader's `curr_y_pos` word plus `$20`, wrapping,
X and facing kept, nothing walks and nothing waits. The cartridge writes
`curr_y_pos` alone and leaves `dest_y_pos` (`$3A`) behind; the port's resting
actor has no separate destination, so the members are at rest at the new row
([source note](../source-notes/README.md)).

Guarded by `the_tree_attack_scenes_write_the_leaders_y_plus_0x20_to_every_member`
(`rust/psiv-core/tests/scene_outcomes.rs`: both events, mounted and on foot,
one and five members on distinct rows and columns, the battle requested on the
tick the last dialogue closes) with its control
`control_a_leader_walk_fails_the_party_move_check`, and the op's own cases in
`rust/psiv-core/tests/events/scenes.rs` (the count stops at the first empty
slot, the word wraps, a counted member missing from the cast faults).

The runtime takes the new positions: when a scene finishes,
`resume_party_from_scene` (`rust/psiv-runtime/src/scene_runtime.rs`) hands
each `PartyMember` actor's cell, facing and step to the field party through
`Party::resume_scripted` (`rust/psiv-core/src/party.rs`); for both tree scenes
the finish comes after the battle. `resume_scripted` does not copy the actor's
sub-cell `slide`, so a sub-cell Y would be lost, but only when the leader was
mid-slide at the write. These are US-image, registry and core-runner results:
no runtime or native test covers the hand-back for these scenes, and they earn
no ordinary-native-input, persistence or connected-route evidence.

## Guards

- `rust/psiv-core/src/scenes/dialogue_callers_image.rs`:
  re-derives every caller and its entries from the US image, reads each
  routine's exit from its bytes and checks `ROUTINE_WINDOWS` against it, ties
  each caller to a scene through the documented ranges (read by
  `scenes/documented_ranges.rs`, each range given to the nearest scene heading
  above it), and compares `DIALOGUE_CALLERS`,
  `UNTRANSCRIBED` and this document, with negative controls for a new image
  caller, a caller retargeted to another routine, a changed entry, a lost
  branch entry, a dropped or misfiled table row, a caller claimed by two
  scenes, overlapping ranges, a missing row here, a stale, transcribed or
  uncalled `UNTRANSCRIBED` entry, a changed exit and a changed, unknown or
  missing window mapping. Without the local ROM its image tests print a note
  and pass.
  `cargo test --manifest-path rust/Cargo.toml -p psiv-core --lib scenes::dialogue_callers_image::`.
- `rust/psiv-core/src/scenes/dialogue_callers.rs`:
  `every_dialogue_entry_caller_runs_its_window` holds the registry to
  `DIALOGUE_CALLERS`, with controls for a scene back on resume, a wrong
  variant, a wrong entry, a duplicated op, a dropped row, a caller claimed by
  two scenes, a row for an unregistered scene, a computed caller with no window,
  a table-driven line dropped, and an inline entry excusing another; and
  `no_scene_resumes_before_it_opens_a_dialogue`.
  `cargo test --manifest-path rust/Cargo.toml -p psiv-core --lib scenes::dialogue_callers::`.

- The resume census (above) is the same two tests' second half:
  `each_resume_entry_exits_as_its_routine` and the resume rows of
  `DIALOGUE_CALLERS`, with controls for a resume back on the last window, a
  dropped resume, an extra resume, a missing inline resume, a stale
  `UNTRANSCRIBED_RESUMES` entry and a resume entry given another routine's
  window.
