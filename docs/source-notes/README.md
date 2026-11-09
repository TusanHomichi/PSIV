# Source / provenance notes

The extractor is grounded against the US retail ROM (SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`) and the
public `ps4disasm` work. What the repository holds from the game, and what stays
local, is listed in the [README](../../README.md#license-and-third-party-code).

## Topic files

The detailed records live in the topic files below: the ledger index's pointers
to "the record below" mean the topic file whose scope covers it. Add a new note
to the topic file whose scope fits, and add a topic file when none fits; this
index lists every topic file.

| File | Scope | Where new notes go |
| --- | --- | --- |
| [formats.md](formats.md) | The two font encodings, Kosinski/Nemesis/Enigma decompression, palettes, plane mappings, map records, layouts and collision | A new format rule, or a correction to a decoder's proven behaviour |
| [oracle-methodology.md](oracle-methodology.md) | How the graphics and battle-formation decoders are pinned against the ROM | A new oracle technique or a changed acceptance criterion |
| [disassembly-discrepancies.md](disassembly-discrepancies.md) | Where the fork's annotations and the retail bytes disagree, and the dated retail-bug chronology | A dated record, correction or retraction |
| [battle-enemy-abilities.md](battle-enemy-abilities.md) | Enemy damage skills, their targeting arms and measured routes | A dated enemy-ability record |
| [battle-party.md](battle-party.md) | Party actions, items, rewards, battle entry/exit and the attack passes | A dated party or battle-flow record |
| [saved-char-id-mem.md](saved-char-id-mem.md) | The `Saved_Char_ID_Mem` party bridge: retail's work-RAM window, the boot clear, and the native save field that fixes it | A new observation or a changed deviation |

## Retail bug ledger index

The numbered index follows the ledger numbering established in the commit
history. It has **13 entries** as of 2026-09-12. The detailed records below are
deliberately chronological and retain their corrections, retractions, and
superseded interpretations; this index is only a map, not a replacement for
that evidence. Classifications are kept explicit: a retail finding is not
quietly promoted to a confirmed cartridge bug, and a candidate is not counted
as one.

1. **RETAIL FINDING / ROM bug — formation `0x177`.** The record declares four
   enemies but contains three enemy/position pairs; the retail source carries
   the same inconsistency.
2. **RETAIL CARTRIDGE BUG — `GetChunkAndCollision`.** The routine uses the
   opposite plane's row stride; the defect is dormant while retail plane widths
   agree.
3. **RETAIL CARTRIDGE BUG — `Battle_EnemyFormationIndexes`.** The table is 416
   bytes for a 417-map space; `AirCastleSpace` reads past it, with the related
   unused `ValleyMazeUnused` encounter footgun recorded in the same entry.
4. **RETAIL FINDING / stray build data — `InnerSanctuary_B1`.** One BG cell
   names chunk `$FF` although the map loads only 128 chunks.
5. **RETAIL CARTRIDGE BUG — `ClimCenter_F2`.** Its BG layout points at the
   wrong, smaller map buffer, leaving 1,280 cells dependent on stale layout RAM.
6. **RETAIL CARTRIDGE BUG — world-map viewer.** The viewer starts 64 bytes
   before the planet page data and renders the pointer table as terrain.
7. **RETAIL CARTRIDGE BUG — `Battle_ProcessRUN`.** It passes an uninitialised
   critical threshold to `Battle_CalculateChances`; the sign-only caller makes
   the result dormant in retail.
8. **RETAIL CARTRIDGE BUG — BLACK WAVE effect `$2C`.** The effect is beyond the
   dispatch table; the later Zio3 correction leaves whether the scripted fight
   reaches the crash path unsettled.
9. **RETAIL CARTRIDGE BUG — Wren Charge pose.** Decompression writes 42 words
   into a 36-word plane buffer and spills six into the next party slot.
10. **RETAIL CARTRIDGE BUG — `Battle_BackgroundIndexes`.** A second 416-byte
    table serves the 417-map id space and reads past its end at `0x1A0`.
11. **RETAIL CARTRIDGE BUG — basement repeatable un-looter.** The historical
    consequence is marked retracted by the final flag model; the measured
    set/clear/respawn cycle remains in the detailed record.
12. **RETAIL CARTRIDGE BUG — `Battle_OrderTurns`.** The max-agility scan reads
    ten words from a nine-entry table and consumes one word past its end.
13. **RETAIL CARTRIDGE BUG — VISION reads the caster's name as its power.**
    Stat selector zero addresses the first encoded name byte. The native
    implementation preserves Hahn's normal +8 dexterity without depending
    on his name; see the measured record below.

### Other classified records in the chronology

- **RETAIL FINDING — FLAG MODEL FINAL.** Retail has four flag banks; `$F120`
  is the shared chest/extended-event bank, `$F140` is temp, and `$F156` is
  fiction. This is the current flag model and supersedes the earlier chain.
- **CANDIDATE RETAIL BUG — `TempEveFlag_BioPlantAlarm`.** The earlier alias
  interpretation is superseded by FLAG MODEL FINAL; the historical candidate
  remains visible below.
- **CANDIDATE RETAIL BUG — invisible Academy Basement blockers.** The port
  treats invisible no-dialogue blockers as solid and silent; hardware
  confirmation remains pending.
- **DEVIATION — `Event_AngerTowerAlys` (`$62`) op 8, the leader's facing.**
  The cartridge writes Chaz's `facing_dir` raw (`move.w #0, $6(a4)` at
  `$071558`) and does not call `Event_UpdateObjFacing`, so his sprite is not
  refreshed; the port records it as `SceneOp::Face` down, which also turns
  the sprite. Nothing shows it: battle `$18` starts on the same frame and the
  field is rebuilt after it. Record [114](../scenes/114_AngerTowerAlys.md).

## Deliberate fixes to retail battle bugs

The owner's battle ruling (2026-08-15) fixes bugs that are plainly
unintended, under the rule in
[RUNTIME_DESIGN.md](../RUNTIME_DESIGN.md#fidelity-and-original-bugs). The
retail behaviour of each is in the `bugfixes=1` table of
[BATTLE_RULES.md](../battle/BATTLE_RULES.md#fork-contamination)
(`ps4.asm` line numbers there); the port takes the fixed branch:

- the level-99 pointer desync in the results loop (`ps4.asm:5926`);
- the shield-element read: a shield's element is applied as a weapon's
  (`ps4.asm:3822`, `3913`);
- the stat lag after a level-up: `UpdateCharModStats` is not re-run
  (`ps4.asm:6119`);
- the Defend and physical-resistance armour clobber of `physical_prop`
  (`ps4.asm:6368`, `9835`, `11287`);
- Telepipe and Escapipe consumed in battle (`ps4.asm:16109`);
- an ability effect id past the end of `AbilityEffectsOffs` (Zio3's `$2C`),
  rejected when the pack loads instead of dispatched.

The stat lag and the clobber are measured on hardware (tapes 10 and 14): Chaz
levels to 2, strength goes 8 to 9 and `atk_pow` stays 18 across 600 frames;
Defend rewrites `physical_prop`'s high byte 2 to 1 for the round. Quirks that
read as design stay faithful: the cure spells that also restore
agility and dexterity, and enemy paralysis clearing each turn.

## Other recorded deviations

- **Zoran Bult strike-only choreography (enemy `$0A`).** Ordinary runtime
  attacks and the certification fixture use the same limited strike lifecycle:
  entering the sixteen-frame attack immediately selects overlay piece 0's
  frame 2, retaining its current idle timer, rather than waiting for that frame
  as USA `BattleObj_ZoranBultAtk` does (`$01528A..$0152C2`). The port releases
  that freeze and restores the displaced target to its party seat when the
  visible attack expires; retail retains the freeze through the target's
  damage wait and restores the target through its reaction/recovery routine.
  This is a deliberate incomplete presentation path, not full-sequence parity.
  The [strike explanation](../battle/BATTLE_COMMAND_UI.md#mid-round-attack-52)
  and [#421](https://github.com/TusanHomichi/PSIV/issues/421) own the remaining
  windup, freeze-release and target displacement/restoration work.

- **Scene fades write only their copy to the CRAM model (#162).** `Pal_FadeIn`,
  `PalFadeOut_ClrSpriteTbl` and `InitVRAMAndCRAM`'s fade (`ps4.asm:85897-85985`)
  each copy `Palette_Table_Buffer` to `Palette_Table_Buffer_2` and then ramp the
  live buffer over frames the scene runner does not spend; the shell's cover
  draws the ramp. The runtime's CRAM model (`rust/psiv-runtime/src/cram.rs`)
  makes the copy and leaves the live words at the pre-fade palette, so after a
  `FadeOut` the words read the old palette instead of black, and a line that no
  op rewrites before the next `FadeIn` comes back with its old words where the
  cartridge would bring it back black. The variable fades, tone loops, flashes
  and cycles are stepped frame by frame
  ([SCENE_PRESENTATION.md](../scenes/SCENE_PRESENTATION.md#cram-state)).
- **The teleport's teardown drops the native ITEM and TECH windows at once.**
  `DestroyAllWindows` closes every window on the camp's stack in turn
  (`ps4.asm:117527-117543`), and the port closes each window it draws at the
  cartridge's position on that window's own frames. The ITEM and TECH pages
  still draw native windows where the cartridge has the pack's pages,
  USE/LOOK/DSCD, the caster list and the technique pages, on every path that
  reaches the teleport message (TELEPIPE, ESCAPIPE, RYUKA, HINAS). Those native
  windows vanish on the teardown's first step, and the closing rectangles of
  the cartridge records they stand for are not drawn. The frame count is the
  cartridge's either way: the teardown runs on the whole stack. It ends when
  the pages draw those windows from their records
  ([#403](https://github.com/TusanHomichi/PSIV/issues/403)). Recorded in
  [TRAVEL.md](../field/TRAVEL.md#teleport-windows-and-effect).

- **The tree-attack party move leaves no stale destination.**
  `Event_CarnivorousTrees` (`$070826..$070847`) and `Event_SavingKyra`
  (`$07093C..$07095D`) write the leader's Y plus `$20` into each counted party
  object's `curr_y_pos` word (`$34`) and leave `dest_y_pos` (`$3A`) and the
  fraction word (`$36`) as they were, then request the battle. The port's
  `SceneOp::PlacePartyY` puts each member at rest on the new row: a scene actor
  has no destination separate from a walk in progress. Recorded in
  [DIALOGUE_CALLERS.md](../scenes/DIALOGUE_CALLERS.md#the-party-move-in-event_carnivoroustrees-and-event_savingkyra).
- **The Menu-button credits skip is not ported.** `Cutscene_Ending`
  (`ps4.asm:158029-158033`) lets a player holding Menu at the scene's start jump
  to the credits; the port plays the whole ending. Recorded in
  [REVISION_AUDIT.md](../scenes/REVISION_AUDIT.md#recorded-deviation-the-menu-button-credits-skip).
- **The 16 oracle-receipt attack bodies ship no attack frames (#214).** The
  pack used to copy the indexed PNGs rendered from the receipts in
  `oracle/fixtures/battle_animation_remainder.json` as these bodies' attack
  frames. Those frames are not the routines' own art. Compared by pixel index
  (palette ignored), eight bodies (`$D526`, `$D998`, `$DCD2`, `$EFB2`,
  `$F088`, `$FCCA`, `$10C7C`, `$10E7A`) capture one identical frame sequence,
  three (`$E394`, `$F53C`, `$10AB4`) share a second, and `$E2BE` is the first
  sequence with one frame repeated. The shared frames show a humanoid figure
  with slash marks and tiles still missing mid-upload. That figure is most
  likely the targeted party member reacting, not the enemy's attack. The routines
  agree: `EnemyAttack_Prophallus` (`ps4.asm:19613`) only writes three palette
  words and an object routine and jumps to `loc_D200` (`ps4.asm:19395`), and
  after the `EnemyAttackOffs` dispatch the caller starts the target fighter's
  own presentation (`ps4.asm:19174-19184`, `move.w #8, $2(a0)`). The four
  unique sets (`$DE22`, `$E0EC`, `$ECDE`, `$10E38`) come from effect-object
  state machines that stream art and place themselves by target and by the
  ability chosen (`EnemyAttack_ProtectBit`, `ps4.asm:23637`, and
  `BattleObj_ProtectBitAtk`, `ps4.asm:30592`), which a static render does not
  reproduce. All 16 bodies therefore stay `deferred` in
  `battle/art/enemy_attacks.json` with no frames, and Godot keeps its
  body-flash fallback for them. Affected enemy ids: 4, 7, 14-16, 27-29, 56-58,
  78, 79, 91, 92, 96-101, 114-119, 121, 127, 128, 132, 151. The receipts and
  PNGs stay as oracle evidence only. `battle/enemy_animations.json` is
  unchanged.

- `Event_RanchOwner`'s alignment walk (`$07259E..$0725CF`,
  [record](../scenes/142_RanchOwner.md#event_ranchowner)). The cartridge loops a frame at a time,
  calling `FieldObj_NPCType7` (`ps4.asm:95230`) and its `loc_49C86` gate (within `$10` of the
  leader's Y, step along X toward him) until the owner's X word equals `Character_1`'s. The port
  runs `MoveActorToActorAxis` on X instead: the scene runner's walk at the party's step pace
  (normally 8 frames a cell), where `NPCType7` steps at `Selector1`'s 16, and without the routine's
  own movement state or its Y gate (the talk range keeps the leader inside it). Both horizontal
  directions take the same path, since the destination is the leader's live X either side. An
  owner already level with the leader spends no frame in either: the cartridge compares the X words
  before its first step, and the port's `BranchIfAligned` skips the walk (a walk to his own cell
  would block the runner for a tick). An owner caught mid-step at the talk starts from the cell he
  was stepping into (the map holds a stepping object at its destination), where the cartridge
  finishes that step inside the loop's `NPCType7` calls. A vertical step leaves his X word alone,
  so the cartridge's compare sees the same column the port does. A horizontal step into the
  leader's column is aligned for the port, which skips the walk, while the cartridge's X word only
  matches once the step completes, so it spends the rest of that step's frames (up to 15 at
  `NPCType7`'s pace) on the loop, each a `VInt_Prepare` that runs `UpdateRNGSeed` and the frame
  counter. Any other horizontal step leaves the port a one-cell walk from his destination, where the
  cartridge also finishes the step first. He ends on the leader's column in every case, the walk is
  at most a cell, and the dialogue that follows is the same; the frames between the press and the
  text, and so the shared RNG stream and palette phase, run up to 8 frames a cell ahead
  of the cartridge, and up to 15 more when the mid-step owner was already heading into the column.
- `Event_HuntersGuild`'s teardown (`.terminateguildwindows`, `ps4.asm:151470`,
  [record](../scenes/124_HuntersGuild.md)). The cartridge runs three `Window_Destroy`s, then
  `jmp Map_LoadChunks`; the port takes the windows down on the press and ends the scene on that tick.
  The list window and the text window each close with render bit 1: `loc_68888` restores two columns
  a pass and `bchg #2` runs `DMAPlane_A_VInt` on alternate passes (`loc_688A8`). The portrait window,
  there only if `$FFFFECA4` bit 0 was set, closes with render bit 0: one restore and no VInt. A window
  `W` cells wide (its `WinGroup_Event` width byte plus one) spends `ceil(ceil(W / 2) / 2) + 1` VInts,
  derived from the code and not measured on the oracle. Those frames read no pad and run no
  `RunMapUpdates`, but each VInt also runs `UpdateRNGSeed` and increments `Main_Frame_Count`
  (`ps4.asm:617,623`). So the field gets the pad back that many frames sooner, the windows do not
  animate shut, and the shared RNG stream and absolute palette phase leave the guild that many frames
  behind the cartridge.

- `Event_PickingFifthCharacter` (`$071E66`, `ps4.asm:151681`,
  [record](../scenes/181_PickingFifthCharacter.md#deviations)), four differences:
  - **The routine words.** Each `loc_72182` row's eighth byte and `loc_721AA` build the objects' routine
    words (`$82BC` plus the byte for the friend restored at their stand, `$8000` plus it for the new party
    object). The port draws the restored object from its map record and a party member from its roster id,
    so neither is carried: the pack's table has no art byte. The restored object keeps the party object's
    facing, as the struct copy leaves it.
  - **The two table walks.** Finding no row whose stand is the cell ahead of the leader, the cartridge
    runs on past the table with `d6 = $140` (object `$FFFFC580`, the row after the last). Finding no table
    character in the party (`loc_72156`), it indexes slot 5, past `Current_Party_Slots`. Neither is
    reachable: the friends are static on their stands, and no story state after `Cutscene_Reunion` puts a
    full party together without one of them. The port faults (`SceneFault::NoFifthRow`) in both cases
    instead of inventing a row.
  - **Macro traps.** `Event_RemoveCharFromMacros` (`ps4.asm:133433`) jumps to `ErrorTrap` on a macro in use
    that lacks the leaver, and `Event_AddMacro` (`ps4.asm:133467`) does on one with no free command. The port
    leaves such a macro alone (`GameState::remove_from_macros`, `add_to_macros`). The port's other scenes do
    not write the macros (`PresentationOp::AddMacro` prints and writes nothing), so a port save can hold a
    macro the cartridge never would, and the trap would end the game.
  - **The trail.** A party object whose destination is more than a step away walks there
    (`FieldObj_GetAutoInput`, `ps4.asm:93232`); the port's trail otherwise snaps a follower that is not
    adjacent. `Party::walk_in` gives that walk to the members the routine seats off the trail, and to those
    behind them, and keeps it across a scene handoff. Other followers keep the snap.
- **The Pengu Feed thief is not drawn (`$96`, #241).** `Event_PenguinFeedStolen`
  (`$0735F6`, `ps4.asm:153259-153328`, [record](../scenes/161_PenguinFeedStolen.md)) writes
  field-object slot 2 (`$FFFFC380`) with routine `$368` (`loc_4B30C`) and art tile `$342`, then
  runs `Field_LoadSprites` and `Field_BuildSprites`, so on the cartridge the thief is on screen
  while it runs in, spins and runs off. The port runs the slot's state (`ObjectOp::Spawn`, its
  walks and facings, the `clr.w` that removes it, the map load that drops it), but the pack has no
  sprite sheet for tile `$342`: the runtime places the object with no sprite, Godot's presentation
  consumer ignores the spawn record, and Godot builds its NPC sprite nodes only when a map loads, so nothing is drawn for
  slot 2. The scene's timing, the take and the flags are unaffected. Drawing
  it needs the art extracted and a sprite node that follows the slot's position and facing and goes
  on despawn or map load; that is the same gap as the strip club's dancers, deferred to
  [#241](https://github.com/TusanHomichi/PSIV/issues/241). The thief's walk speed, eight frames a
  cell, is the cast's step rate and is not measured against `loc_4B30C`.
- `Event_MonsenEarthquake`'s facing frames (`$2C`, `ps4.asm:147499-147814`,
  [record](../scenes/143_MonsenEarthquake.md)). Retail's `Event_UpdateObjFacing`
  (`$05A936`) ends in a `VInt_Prepare` in the US build. Outside Gryz's spin, which keeps it, the
  scene calls it 15 times, 16 with Rune, and twice through `loc_6E670`, which calls it once for each
  party slot that is filled (4, or 5 with Rune). The port's `Face`, `FaceOppositeOf` and `FaceParty`
  take no frame, so the scene is 23 frames shorter without Rune and 26 with him: the informant's two
  turns, the party's four or five, Gryz's two and Chaz's and Rika's after the spin, the second
  villager's own turn, Chaz's four as he looks about, and the two `loc_6E670` passes. Each lost
  frame runs no `Field_UpdateObjects`, no `RunMapUpdates` and reads no pad, so no object moves on it in either build, and every walk and wait lines up with the
  cartridge's. What drifts is the VInt's own work: `UpdateRNGSeed` and `Main_Frame_Count`
  (`ps4.asm:617,623`), so the shared RNG stream and absolute frame phase leave the scene that many
  frames behind the cartridge, and the turns show a frame sooner. The villagers' creation is not a
  deviation: the `dest` Y the scene writes from the X word is overwritten by
  `FieldObj_NPCScriptMove`'s setup before the object's one pass, so nothing moves on that frame.
- **The vehicle refusal opens in the dialogue window.** `Event_GettingOffVehicle`
  (`ps4.asm:145397-145413`) draws `CannotGetOffString` into window 0 with window tiles and holds
  it until `Joypad_Pressed` has a direction, A, B or C. The port opens the same string (from the
  pack) in the dialogue window, closed by the dialogue's own press, and then reloads the chunks as
  the cartridge does. The party, the selector and the music are the cartridge's. Recorded in
  [207](../scenes/207_GettingOffVehicle.md).

- Kadary's `Event_FaintingPriest` (`$24`) and `Event_ZioFanatic` (`$31`)
  (`ps4.asm:147047-147092`, `:148262-148298`, [record](../scenes/164_KadaryPriestAndBeliever.md)):
  - **Facing frames.** Each turns its speaker with one `Event_UpdateObjFacing` (`$05A936`, a
    `VInt_Prepare` at its end in the US build) before its first line. The port's facing ops take
    none ([#164](https://github.com/TusanHomichi/PSIV/issues/164)), so each opens its text one frame
    sooner. Nothing moves on that frame; the RNG seed and `Main_Frame_Count` (`ps4.asm:617,623`) fall
    one frame behind.
  - **The faint is not drawn.** `$24` gives the priest routine `$180` (`FieldObj_FaintedPriest`) and
    holds 46 frames for its script. The port records the routine and the frames, but the renderer
    draws each map object from the sheet its record names when the map loads, so the priest stays
    standing on screen through the faint and afterwards. His state (the routine, his `$38`, the block
    in his second cell) matches the cartridge. Drawing a scene-changed routine belongs with
    [#329](https://github.com/TusanHomichi/PSIV/issues/329).
  - **The writhing believer takes a cell, not 8-pixel-rounded pixels.** `$31` copies object 4's
    live `$30`/`$34` into object 9 and rounds each down to 8 pixels (`lsr.w #3` / `lsl.w #3`,
    `$06EE5E..$06EE81`). The port's `ObjectOp::SpawnAt` gives object 9 object 4's cell, the one the
    map holds for him, with no sub-cell offset. Standing still, object 4 is on a 16-pixel cell on both
    axes (his record is (`$140`,`$1F0`), and a wander step is one cell), so the rounding changes
    nothing and the two agree exactly. They differ only when he is talked to mid-step, which the
    cartridge allows (`Interaction_ChkObjects` finds a stepping object at its destination; see
    [NPC_WANDER](../field/NPC_WANDER.md#interaction-while-moving)): the port's map has already moved
    him to the destination cell, while on the cartridge his pixel position is short of it on the
    walked axis, and the other axis is a multiple of 16 and unchanged. Rounding down, a believer `k`
    pixels into a step right or down lands on his origin cell (`k` < 8) or the half-cell between
    (`k` ≥ 8), never the destination; one stepping left or up lands on the half-cell (`k` ≤ 8) or the
    destination (`k` ≥ 9). In those cases the cartridge's writhing believer is drawn up to one cell
    back along that axis, and its blocking and talk position (`$31` → `$1F`) follow its pixels there
    until the next map load drops him, where the port draws, blocks and answers from the destination
    cell. The scene's frames, its lines and the flags it writes are the same either way.

- `Event_DyingBoy` (`$86`, `ps4.asm:152726-152855`, [record](../scenes/163_DyingBoy.md)) differs
  from the cartridge three ways, none of which changes a flag, an item or where anyone stands:
  - **Facing frames.** It calls `Event_UpdateObjFacing` (`$05A936`, a `VInt_Prepare` at its end in
    the US build) once for the boy, once for each filled party slot through `loc_5A97C`, and once
    for Chaz: seven frames with a five-member party. The port's facing ops take none
    ([#164](https://github.com/TusanHomichi/PSIV/issues/164)). Nothing moves on those frames; the
    RNG seed and `Main_Frame_Count` (`ps4.asm:617,623`) fall that many frames behind.
  - **The slow step.** `clr.b (FieldObj_Step_Offset).w` slows the gathering walk and Chaz's walk to
    the bed's foot to sixteen frames a cell. The port does not apply `SetStepOffset`
    ([#337](https://github.com/TusanHomichi/PSIV/issues/337)), so they walk at eight. The gathering
    still ends inside its fixed 150-frame wait; Chaz's two walks, which end their waits, take half
    the cartridge's frames.
  - **The sword is not drawn.** `FieldObj_ChazAlisSword` (`$334`, tile `$43C`) has no record among
    the pack's temporary objects, so the 164 frames pass with Chaz's sprite hidden and nothing in
    his place. Drawing it needs the art staged for `CulversHouse`
    ([#329](https://github.com/TusanHomichi/PSIV/issues/329) owns scene-built objects).

- The facing frame in Torinco's `Event_DyingBoyFather` (`$84`), `Event_FatherAfterBoyRecovery`
  (`$85`) and `Event_MeetingSekreas` (`$87`) (`ps4.asm:152706-152725`, `:152856-152865`,
  [record](../scenes/162_TorincoCulvers.md)). Each turns object 0 with one
  `Event_UpdateObjFacing` (`$05A936`), whose US build ends in a `VInt_Prepare`, before its text.
  The port's `FaceOppositeOf` takes no frame (the engine-wide gap is
  [#164](https://github.com/TusanHomichi/PSIV/issues/164), which also owns charging it), so each
  scene opens its text one frame sooner. That frame runs no `Field_UpdateObjects`, no
  `RunMapUpdates` and reads no pad, so nothing moves on it and the text, the turn and the flag are
  the same. What drifts is the VInt's own work, `UpdateRNGSeed` and `Main_Frame_Count`
  (`ps4.asm:617,623`): the shared RNG stream and absolute frame phase leave each scene one frame
  behind the cartridge. `torinco_scenes.rs` pins the zero-frame turn, so #164's fix fails it and
  retires this entry.

- `Event_WreckageEngine`'s swap and palette (`$29`, `$06DBE8..$06DE7B`, `ps4.asm:147172-147370`,
  [record](../scenes/206_WreckageEngine.md)). Four differences, kept on purpose:
  - **Rika absent, at both branches.** Both `FindCharacterSlot(5)` / `tst.b d1` / `beq` tests
    (to `loc_6DCB2` before the reading, to `loc_6DE58` after it) branch only when Rika leads.
    Without Rika, `d1` is `-1`, so the cartridge takes the swap path at each: the first stores
    `lsl.w #6` of it (`$FFC0`) in `$FFFFECD0`, both walk the object `$40` bytes before
    `Character_1` alongside the leader, the first turns that object up, and the second turns the
    leader up and pans the camera back to him. The port's `WreckageOp::BranchIfLeads` takes
    `if_leads` when Rika is absent, as when she leads: no swap and no `Face` either time, and no
    pan back. Rika is in the party whenever the Wreckage is reachable, so no campaign reaches this.
  - **The swap's pace.** `loc_6DE7C` runs `Field_UpdateObjects` and a `VInt_Prepare` a frame at a
    time, with `Joypad_Held` cleared, until the leader's `$30`/`$34` reach its `$38`/`$3A`. The
    port's `MoveActorOffset` / `MoveActorToActorAxis` legs, each ended by `WaitForActor` on the
    leader, walk at the scene runner's step pace. The leader and Rika end on each other's cells in
    both; how many frames each leg takes was not measured on the cartridge, so the frames from the
    first text to the resumed one, and the RNG and frame phase after them, may differ by up to the
    difference in pace over the three legs, each way.
  - **The facing frames.** `Event_UpdateObjFacing` (`$05A936`) ends in a `VInt_Prepare` in the US
    build; the port's `Face` takes none. The scene turns Rika after the swap and the leader after
    the swap back, so it is two frames shorter (none when Rika leads), with the same drift as
    `Event_MonsenEarthquake`'s above.
  - **The palette words are logged, not drawn.** The cartridge adds `$220` to
    `Palette_Table_Buffer+$1A` four times, eight frames apart (`loc_6DD6C`), and `loc_6DE58` writes
    `$620` and `$2CE` to `Palette_Table_Buffer+$7C`/`+$7E`. The port keeps each as a
    `PresentationOp` (`AddPaletteWord`, `SetPaletteWords`), with every wait between them; the
    runtime's CRAM model applies them, and the map, sprites, panels and dialogue window draw
    from it ([SCENE_PRESENTATION.md](../scenes/SCENE_PRESENTATION.md#drawing-the-cram)). No game
    state reads them.

## Public reverse-engineering references used

- `squidfeatures/ps4disasm` (`ps4.asm`, `ps4.constants.asm`), including the documented record layouts and stable symbolic IDs.
- `alechenninger/ps4disasm`, a later fork used as an additional orientation/reference source.

The public disassembly documents, among other things:

- initial character records as 66-byte entries
- inventory records as 22-byte entries
- enemy records as 48-byte entries
- enemy-skill records as 8-byte entries
- techniques, skills, and combo records as 8-byte entries
- vehicle records as 26-byte entries
- character level records as 22-byte entries, addressed through a per-character pointer table
- battle formations as variable-length `$FF`-terminated records inside Kosinski-compressed blobs

## Retail US offsets proven against this ROM

- Enemy data: `0x2816BC`
- Enemy skills: `0x28336C`
- Combo data: `0x285424`
- Vehicle data: `0x2855E2`
- Level pointer table: `0x004074`
- Primary Chaz level table / start of primary progression block: `0x2856B0`
- Mirrored progression block: `0x2A3A42`
- Initial character stats: `0x2A8ACA`
- Inventory data: `0x2A8E28`
- Technique data: `0x2A9BE8`
- Skill data: `0x2A9D28`

Kosinski-compressed blobs (end-exclusive):

- Battle formation indexes: `0x2836EC..0x283E67`
- Battle formation data 1: `0x283E6C..0x2842AE`
- Battle formation data 2: `0x2842BC..0x284713`
- Battle formation data 3: `0x28471C..0x284B7E`
- Battle formation data 4: `0x284B8C..0x284F7C`
- Boss formation data: `0x284F7C..0x285012`

- Shop inventories (`ShopInventories`): `0x0681A4..0x0682A3`, 49 `$FF`-terminated lists indexed by `GetOffsetByID_FF_Delim` (terminator counting, no pointer table, no bounds check — the bound lives in the data, corroborated by the 49-entry shopkeeper-portrait and greeting tables)
- Shop locations (`loc_68394`): `0x068394`, `$FFFF`-terminated 8-byte entries `(map id, x, y, group<<8|index)`; group 0 is the inn system with its own index space, groups 1–2 read `ShopInventories`

The offsets are accepted only after the full ROM SHA-256 matches the supported retail build, then short known signatures are checked again at those offsets before extraction.

## Important boundary

`ITEM_SYMBOLS` and `ENEMY_SYMBOLS` are disassembly identifiers, not decoded cartridge text. Records now also carry `display_name`, decoded from the cartridge's own name tables; the symbols stay because they disambiguate duplicates the ROM's display text does not (two skills both display as `FLAELI`; the ROM's `SHOOTINSTR` is the symbols' `Shootnstar`).
