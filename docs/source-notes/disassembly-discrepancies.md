# Disassembly discrepancies

Scope: every place where the fork's annotations, labels or
option-gated branches disagree with the retail bytes, and the
chronological retail-bug records that follow from them — flags,
shops, encounter tables, layouts — with their corrections and
retractions kept in place.

Index: [source and provenance notes](../../SOURCE_NOTES.md).

## Discrepancies found against the disassembly's annotations

Scene-level fork divergences (the 17 ungated Grand Cross scene rewrites) are
documented per scene in `docs/scenes/*.md`, each with retail byte ranges and
byte-diff analyses — including `AlysFound`, where the fork kept the retail
prologue/epilogue and cut a hole in the middle, omitting the dialogue call
and the flag-set whose absence would re-fire the trigger forever.


- `Battle_FormationData4`: the inline range annotation in `ps4.asm` gives
  `0x284B8C-0x284C3D`, which is only the first of many annotated chunks for
  that blob. The Kosinski stream actually runs to `0x284F7C`, immediately
  before `Battle_BossFormationData`, and the decompressor consumes exactly
  0x3F0 bytes.
- Formation `0x177` (block 3 record `$77`) declares four enemies in byte 4 but
  lists three enemy/position pairs, and its group bitmasks cover three slots.
  The disassembly's uncompressed source has the same bytes, so this is the
  ROM's own inconsistency rather than a decode error. Both the declared count
  and a mismatch flag are emitted. (Blast radius, proven from the code: the
  count byte's only reader is the Slasher weapon's hit-effect positioning, so
  the bug misplaces battle visuals in one rare Dezolis encounter and affects
  nothing else.)
- Shops: none. `ShopInventories`, its `ItemID_*` constants, and the
  surrounding labels match the retail bytes exactly. A clean oracle is itself
  a finding.
- `DialoguePortraitArtPtrs` is 7 entries longer in the fork than in retail:
  the retail table ends at index `$27` (Sekreas); the seven shopkeeper
  portraits exist in the ROM but are reached through the shop tables near
  `0x068000`, not through the portrait table.
- The fork's `revision!=0` branch of `Art_DialogueFont` includes a 6,720-byte
  italics font that appears nowhere in the retail image; retail carries the
  1,280-byte plain dialogue font.
- General caution: the `reference/` clone is configured as a Grand Cross hack
  build (`grand_cross=1`, `bugfixes=1`, `optional_fixes=1`,
  `no_random_battles=1`, `external_formation_data=1`). Its `revision`-gated
  and option-gated branches describe the hack, not necessarily retail; every
  slice must prove which branch matches the cartridge rather than trusting
  the default.
- `script/documentation.txt` omits control code `$FB` (extended event-flag
  check, 3 operand bytes) entirely.
- `$F6` (event) takes a 2-byte operand. The disassembly disagrees with itself
  (`GetEventFromDialogue` reads a word; `TextCtrlCode_Event` skips one byte);
  the ROM settles it — a 1-byte reading puts 43 corpus bytes outside the
  font, a 2-byte reading puts zero. The 1-byte skip is a dormant quirk: every
  retail `$F6` sits at an entry start, where the preprocessor handles it.
- `CharName_Chaz = "Shay"` in `ps4.constants.asm` is the Grand Cross hack's
  rename, applied unconditionally in this clone. The cartridge says `Chaz`.
- Five of the 43 `script/dialogue N.asm` sources do not reproduce retail
  bytes: tree 17 is wholesale rewritten by the fork (84 edits); trees 19, 29,
  31 and 34 each carry two single-byte defects that cancel in total length,
  so they pass a naive size check. The tests pin each defect's exact offset
  and shape so a future clone update fails loudly instead of silently
  widening the exception set. The other 38 trees round-trip byte for byte.
- RETAIL CARTRIDGE BUG: `GetChunkAndCollision` (`0x045AB8`) selects the
  layout base from `$FFFFEC24` correctly but takes the *opposite* plane's row
  size as its stride, contradicting `SetupChunksFG` (`0x054356`),
  `SetupChunksBG` (`0x0543A6`) and its own bounds check eleven instructions
  earlier. A crossed branch, confirmed in retail opcode bytes (all three
  sites pinned). Dormant because every map gives both planes the same width;
  our decoder takes the stride from the layout it is handed, i.e. the
  behavior the original intended.
- RETAIL CARTRIDGE BUG: `Battle_EnemyFormationIndexes` is 416 bytes for a
  417-map id space; MapID `$1A0` (`AirCastleSpace`) reads one byte past the
  end into `Character_Init` data, yielding nonexistent group 74. Dormant only
  because that map has random battles disabled. Relatedly, `ValleyMazeUnused`
  (`$02D`) stores `$FF` (no encounters) but leaves random battles *enabled* —
  rolling an encounter there would index far past the tables; dormant because
  the map is unused. `MystVale_Part4` and `AirCastleXeAThoulRoom` assign
  groups with battles off (wasted, harmless). All surfaced in
  `encounter_anomalies`.
- The fork's inline address annotations around the `0x8000` region are ~0x552
  off retail; `Battle_EnemyFormationIndexes` was located by content, not
  annotation. My earlier scouting note placing it at `0x0085A2` was wrong —
  that range is a 15-word palette.
- RETAIL CARTRIDGE BUG: `ClimCenter_F2`'s BG layout pointer targets
  `ClimCenter_F3`'s all-zero 32×32 buffer, but the map is 48×48 — 1,280 BG
  cells render whatever the previously loaded map left in `Map_Layout_BG`
  (zeros on a cold boot). Collision reads the FG plane there, so only the
  picture is affected. The pack zero-fills and records the anomaly.
- `InnerSanctuary_B1`'s BG layout names chunk `$FF` in exactly one cell while
  the map loads 128 chunks — the only such cell in the cartridge (all 718
  planes swept). `SetupChunksBG` has no bounds check, so hardware reads a
  `Chunk_Table` slot the map never wrote: stale data or, cold-booted, an
  empty definition. A stray byte in the original build data; the pack
  substitutes the empty definition and records the substitution.
- Seven Academy maps store zero-padded 32×32 layout buffers for 32×16 grids;
  the surplus is unreachable (`GetChunkAndCollision` wraps Y). The loader
  never checks blob lengths — the grid extent comes from the dimension bytes
  alone — and `layouts.decode_layout` now implements exactly that rule.
- `Zema_LockedDoorsOffs` is named backwards in the disassembly: the offsets
  are where the doors *open* (chunks with map-change cells are written in
  once `EventFlag_IgglanovaZema` is set; the stored layout holds solid
  chunks). Five doorway warps (four in Zema, one in BirthValley_B1) cover no
  type-1 cell in the stored layout for this reason — event-gated doors, not
  defects.
- Census facts (what retail data actually contains vs what the code
  permits): collision type `$7` exists (172 cells, 8 maps, every one
  adjacent to a `$9` water cell — a shoreline artifact of one chunk set) and
  is walkable via `TileColl_Empty`; types `$A` (sand) and `$B` (ice) never
  appear in any field map; dialogue-tree binding is 1-based (1..=43, never
  0); exactly one field object has a facing byte outside {0,4,8,$C}
  (AiedoPub object 2, byte $10); retail selects only 9 of the 15
  `XYRangeJmpTbl` routines. The pack manifest carries a `census` section so
  consumers assert against observed reality instead of hardcoding ranges.
- RETAIL CARTRIDGE BUG: the in-game world-map viewer reads its chunk ids 64
  bytes early. `FieldRoutine_WorldMap` (`0x0666CC`) loads the BG *pointer
  table* address (`loc_10BE02`) and streams 16×1,024 bytes from there as raw
  chunk ids, but the page data starts 64 bytes later at `0x10BE42` — so the
  minimap draws the pointer table's own bytes as terrain in its first
  half-row and shifts the whole planet by 64 chunks. Same shape for Dezolis
  (`0x0666DA`). Settled by rendering both readings: aligned yields Motavia
  with the Nurvus crater centered; retail's yields a split continent with a
  garbage strip. Confined to that viewer screen.
- Census correction (overworld data): collision types `$A` (sand) and `$B`
  (ice) are absent from the interiors but NOT the cartridge — Motavia has
  632 sand cells (no ice), Dezolis 1,704 ice cells (no sand). The earlier
  "never appear in any field map" note was interior-only truth.
- Overworld facts: the paged layout format is uncompressed (1,024-byte pages
  of chunk ids, a rolling 4KB / four-page window keyed by camera row; chunk
  definitions still Kosinski via the record's normal list); both planets are
  128×128 chunks wrapping at 4,096 px on both axes; Dezolis stores only 8
  distinct pages and aliases its bottom half from its own rim (emitted
  as-is, recorded as an anomaly). Nine page-copy hooks rewrite layout cells
  from event flags — 12 patches, 51 writes, 147 cells — which is how the
  spaceports, Machine Center and The Edge doors appear and how the Bio Plant
  seals; the pack emits them as `layout_patches`, proven by applying each
  patch and watching doorway cells become type 1. Priority tiles (bit 15,
  which survives the collision-bit mask) draw above sprites on hardware; 339
  maps carry an overlay, and all 22 maps without one have exactly zero
  priority tiles.
- Field sprite facts, all read from the cartridge: walking is 8 frames per
  16px cell (`FieldObj_MovementsTbl` `0x047AA8`, constant `$0200`; slow/fast
  blocks `$0100`/`$0400` exist, and retail's selector mask is `#3` where the
  clone's is `#7`). The sprite-mapping record's piece count is stored minus
  one while animation sequence counts are stored exactly — two opposite
  conventions three bytes apart. Mapping byte 5 (mirror-builder X offset) is
  real data no retail field path ever reads. Pattern words are added with a
  genuine 16-bit carry (1,056 composed pieces depend on it). A sprite's CRAM
  line comes solely from its routine's `$13(a4)` byte; all 11 party routines
  store line 2, which `loc_53F14` splices from `Pal_Init_Line_3` on every
  map — why the party's colors never change. Animation free-runs (Chaz's
  4-frame cycle is 44 frames against 8-frame steps) and idle is walking's
  frame 0, not a separate sequence. AiedoPub object 2's facing byte `$10`
  sends `FieldObj_Animate` past its own table; the pack names those
  sequences `idle_facing_0x10`/`walk_facing_0x10` so nobody reads the byte
  as a direction.
- New-game findings (instruction-level provenance in the pack's
  game_start.json): retail hands over control with **Chaz alone** on
  PiataAcademy_F1 at cell (48,19) facing down — Alys is the NPC he finds;
  she leads only after Event_AlysFound. 500 meseta, empty inventory, event
  flag 7 set by the opening event, 11 unnamed extended event flags and three
  town flags preloaded by the initialiser (probably dialogue-state; recorded
  as ids, not guessed). The clone mislabels the initialiser's flag-table
  copy target as Chest_Flags where retail writes Extended_Event_Flags — a
  reading under which every new game would start with 11 chests looted —
  and carries Grand Cross start-position edits ($58/$22/right vs retail's
  $60/$24/down, self-documented by "; was" comments).
- Nine Enigma call sites are revision-gated and the cartridge runs the `else`
  (English) branch — proven three ways: the retail code contains each
  mapping's `lea`/`move.w #base` pair exactly once with the retail base
  values; the decoded cell counts match only the retail dimensions; and the
  `revision=0` bases would point mappings outside their art blobs.
  `GetMapLayoutChunkFG/BG` hard-code the 128-byte overworld stride and are
  overworld-only helpers despite their general names. `MapEni_GameStartMotaBG`
  / `ArtNem_GameStartMotaBG` are unreferenced in the disassembly source; the
  cartridge reaches them at `0x073C4E`.
- RETAIL CARTRIDGE BUG: `Battle_ProcessRUN` calls `Battle_CalculateChances`
  (`$00B5A6`) with `d5` — the critical threshold — uninitialised
  (`ps4.asm:7704-7712` loads only `d1-d4`). Dormant: the caller tests only
  the result's sign, and both "normal" (0) and "critical" (1) verdicts are
  non-negative, so whatever garbage `d5` holds cannot change the escape
  outcome. A port must document this rather than silently invent a value.
- RETAIL CARTRIDGE BUG: enemy skill 112 `BLACK WAVE` declares effect `$2C`,
  one past the end of the 44-entry `AbilityEffectsOffs` table (`$0061BE`,
  ids `$00-$2B`). The `TRAP #2` dispatcher (`$000216`) has no bounds check;
  dispatching it would jump to the odd address `$00B033` and raise a 68000
  address error — a crash. Dormant: the skill's only user is enemy 152
  `Zio3` (16383 HP, all-255 defences — a debug/leftover boss) which appears
  in zero of the 504+27 formations. Verdict for the port: reject effect ids
  outside `$00-$2B` at data-load time and record the one offender as a
  census anomaly. Full battle fact base: `docs/BATTLE_SCOUT.md`.
- RETAIL CARTRIDGE BUG: Wren's battle pose 3 (Charge) decompresses 42 words
  into a 36-word plane buffer. The per-slot buffers (`loc_9A80`) are `$48`
  bytes apart and the draw loop (`loc_86D6`) stamps 6x6, so the surplus 6
  words land in the next party slot's buffer and are never drawn — dormant
  unless the neighbouring slot isn't redrawn afterwards. Every other pose
  across all eleven characters is exactly 36 words. Found by
  `psiv_tools/battle_art.py`, which pins the anomaly rather than widening
  its size rule.
- [PARTIALLY CORRECTED — see the FLAG MODEL FINAL entry] RETAIL FINDING (2026-08-15, triple-verified): the cartridge has FOUR flag
  banks, not five. The clone defines `Temp_Event_Flags = $FFFFF156` with its
  own Test/Set/Clear doors — but the retail image contains ZERO instructions
  addressing $F156 (no `lea (xxx).w` = 41F8F156, no absolute-long
  0000F156, no word-lea to any address in $F141-$F15F), while the $F140
  (Chest_Flags) door shows exactly the site count the clone splits across
  its chest AND temp labels (Test/Set at four banks each — event, extended,
  $F140, town — Clear at three; nothing ever clears a town flag). So the
  retail "TempEveFlags" calls dispatch through the chest bank's door.
  ANSWERED by call-site bytes: every retail `move.w #$13,d0` (the clone's
  TempEveFlag_Xanafalgue) jsr-targets the $F140 doors — set 0x04AEA4 ->
  0x05767A, test 0x051E18 -> 0x057638, clear 0x0522BE -> 0x0576BC — with
  the id RAW, no offset. Retail temp flag N and chest flag N are the same
  bit. CANDIDATE RETAIL BUG (behavioral test pending): TempEveFlag_
  BioPlantAlarm = 8 = ChestFlag_Alshline, so the Bio Plant alarm's
  set-on-trigger / clear-on-exit plausibly re-arms or force-loots the
  Alshline chest on hardware. psiv-core's five-bank model must merge temp
  into the $F140 bank (one 256-id space) to reproduce retail. Found by the map-effects decoder
  (docs/MAP_EFFECTS.md finding 5); ROM byte sweeps re-verified
  independently by the lead.
- RETAIL CARTRIDGE BUG (a second instance of a known one):
  `Battle_BackgroundIndexes` (0x006CA8) is 416 bytes for the 417-map id
  space — the identical off-by-one `Battle_EnemyFormationIndexes` has.
  MapID `$1A0` reads the byte past the end (the first byte of the 32-entry
  post-step selector at `loc_6E48`, value 0 → MotaDesert). Dormant for the
  same reason: that map has random battles disabled. Two independently
  authored 416-byte tables over a 417-map space suggests the id-space count
  itself was wrong somewhere in Sega's build tooling. Found during battle
  background emission (battle/art/backgrounds).
- [RETRACTED — see the FLAG MODEL FINAL entry] The flag-bank alias (above) collides on SIX id pairs in actual use, not
  one: temp $08 BioPlantAlarm = chest Alshline, and temp $09-$0D — Vahal Fort's two
  moving platforms and two conveyor-direction terminals plus a Weapon
  Plant platform — = the PsycoWand, ControlKey, Canceller, EclpsTorch
  and AeroPrism chests. All six pairs live in ONE byte, $FFFFF141
  (masks $80/$40/$20/$10/$08/$04, MSB-first), already logged as the
  oracle's chestb1 column — the future mid-game experiment needs route
  reach only, and a single platform round trip demonstrates the alias
  BIDIRECTIONALLY (down sets, up clears the paired chest bit).
  Platform state and treasure state are one bit each on retail hardware —
  a platform left mid-cycle plausibly marks a chest looted (or re-arms
  it) elsewhere in the world. Full table and behavioural pin in
  psiv-core/src/state.rs; hardware confirmation queued with the oracle
  (those dungeons are late-game, so it waits for deeper routes).
  WIDENED (map-load-clear scout): six was the count among transcribed
  trigger tables only — the constants file shows essentially every temp
  id across $08..$1D doubling as a chest id (e.g. $14 GrbkTwEyeball =
  GrbrkTwStarDew, $18 ChazHouse = PiataMonomate), so the hardware-test
  framing is "the whole range", not an enumeration. Seven map-load
  routines clear twenty of these ids as explicit immediates
  (psiv-core/src/map_load.rs carries the transcribed table).
- [INTERPRETATION CORRECTED — the measurement stands; see the FLAG MODEL FINAL entry] The flag-bank alias is CONFIRMED ON HARDWARE (tape 17): the Xanafalgue
  temp flag ($13) lands at $FFFFF142 bit 4 — byte 2 of the CHEST bank,
  exactly where the reversed-bit arithmetic (`bset 7-(id&7)`) predicts —
  while the clone's fifth bank at $F156 stays zero for all 24,880 frames.
  The innocent explanation is ruled out, not assumed away: ItemFound never
  runs and the basement's own chests live in byte 3, which never moves.
  Measured consequence: walking the opening-act Piata basement WRITES THE
  GARUBERK TOWER MOON SLASHER CHEST'S FLAG (ChestFlag_GrbrkTwMoonSlashr =
  $13). Bounded claims: whether that chest reads as looted at Garuberk
  (unreachable by tape), and whether anything clears the bit on leaving
  the basement, are both untested — a leave-and-reenter tape is queued.
- [CHEST CONSEQUENCE RETRACTED — see the FLAG MODEL FINAL entry; the measured set/clear/respawn cycle stands] RETAIL CARTRIDGE BUG (measured both directions, tape 18): the basement
  round trip is a REPEATABLE UN-LOOTER. The Xanafalgue's flee sets chest
  bit $13 at despawn (same frame); arriving on the destination map clears
  it 39 frames after load (the clear is tied to LOADING the destination,
  not to leaving — whether every map clears id $13 or only some is
  untested); returning to the basement respawns the Xanafalgue. So a
  player who owns the Garuberk Tower Moon Slasher and later walks into
  the Piata Academy Basement and back out has that chest's flag CLEARED
  and can take the item again — reachable by ordinary backtracking, no
  sequence break. Remaining untested: that Garuberk's chest-open check
  reads this bit (route unreachable by tape; follows from the measured
  one-bank model). Bonus transcription datum: the fleeing Xanafalgue
  runs at 4 frames/cell — the fast step table. ENGINE NOTE: the flag
  set/clear/respawn cycle is the real model for the Xanafalgue's
  despawn; the runtime's interim session-permanent despawn ledger
  diverges (no respawn) and is slated for replacement by the map-effects
  flag gates plus the yet-unread map-load flag-clear routine.
- Item byte $12 has TWO readings (UpdateCharElems 0x05FD2A): held by a
  weapon hand it is the attack element Battle_LoadWpnAttackElem reads;
  worn as a shield/head/body item it instead writes resistance value 1
  into the element property it names — and the grant is unconditional,
  so armour can DOWNGRADE a character's innate immunity (0) to mere
  resistance (1). Reachable in play; the pack emits element.role per
  item and finished element_props per character as conformance vectors.
- Correction (2026-09-15): dual wielding is available through the normal
  hand selector at `loc_5F93A..loc_5FAEE`. Types 1, 2 and 5 offer right/left
  choice; `loc_5F99E` replaces the tentative default dispatch with that
  selection before inventory commit. The earlier initial-data-only claim
  stopped at `EquipItemType_OneHanded` and missed this later path. A left
  weapon and a right shield are both valid. `docs/EQUIP_SCOUT.md` records
  the repaired native flow and its verification.
- Citation correction: UpdateCharModStats is retail $05F754, not
  $05F880 ($05F880 is the unrelated routine UpdateEquipment tail-calls).
  The behaviour transcribed everywhere is correct; two docs carried the
  wrong address.
- CORRECTION to the BLACK WAVE entry: Zio3 IS fielded — boss formation
  `event_battle_index` 4, the scripted Zio fight (the earlier "zero of
  the 504+27 formations" searched only the 504 normal ones). Whether
  retail can actually dispatch effect $2C there (address-error crash) is
  UNSETTLED — thirty years without crash reports suggests the scripted
  fight ends before the AI reaches the skill, or the dispatch path
  differs; an oracle experiment waits on mid-game routes. The port's
  guard moves accordingly: the loader cannot hard-reject the record
  (that refuses the retail pack), so usable()/rejected() splits plus the
  runtime UnsupportedAbility event hold the line.
- Retail equipment bonuses go genuinely negative (agility to -5, mental
  and dexterity to -10), making the two-adder split live: add.b without
  sign extension for the four byte stats, ext.w signed for the three
  derived words. Both reproduced; the 11 conformance vectors exercise it.
- RETAIL CARTRIDGE BUG: Battle_OrderTurns' max-agility scan reads TEN
  words of the nine-entry Battle_Turn_Order table (ps4.asm:7779-7788) —
  one word past the end, into whatever follows at $FFFFEFD4. Dormant in
  effect (the stray word only matters if it exceeds every real agility);
  the port implements the intended nine and documents the deviation.

- FLAG MODEL FINAL (2026-08-15, three independent evidence lines): the
  clone mislabelled TWO banks, not one. Retail's four flag banks are:
  $F100 event (256), $F120 CHEST + "extended event" — ONE bank, two
  name-spaces on the same bits — $F140 TEMP (what the clone calls
  Chest_Flags), $F160 town. $F156 remains fiction. Evidence: (1) the
  chest system's three call sites all target the $F120 doors
  (LoadTreasureChests test 0x537E0, ItemFound entry test 0x66B2A,
  ItemFound set 0x66DE0; ItemFound's type dispatch is 0->F100,
  1->F120, else->F140), byte-verified twice independently; (2) tape 20's
  whole-64KB RAM diff — found blind, before the byte reading reached the
  oracle — shows ChestFlag_PiataMonomate (24) landing at $FFFFF123 bit 7,
  simultaneous with the item grant at ItemFound+10, and NOTHING in
  $F140-$F17F; (3) the pack census: the new-game initialiser's 11
  preloaded "extended event flags" are, eleven for eleven, real chests'
  flag ids (BioPlant_B2 x3, HuntersGuildStorage, KadaryStorageRoom,
  Nurvus_B3, Hangar, Kuran_F1, ClimCenter_F3, WeaponPlant_F2,
  TonoeBasement_B3) — RETAIL PRE-SETS ELEVEN CHESTS AT NEW GAME.
  MECHANISM CORRECTED by the clear-door sweep: there is NO arming
  scheme, because NOTHING in the cartridge clears a $F120 bit — the
  clear door's sole caller is the disassembly's own annotated dead
  routine. Chest flags are permanent, and the eleven are permanently
  DISABLED chests: seven hold HuntKnife across unrelated maps (plus
  100mst, ShortCake) — placeholder/duplicate records with defaulted
  items, switched off at boot so they never spawn lootable. And the
  REAL proof of one-bank-two-names is a second behavioural 11/11:
  every literal-id $F120 test site in story code is a real chest's
  flag ($08-$0D = EclpsTorch/FradeMantl/Canceller/PalmaRing/AeroPrism/
  RepairKit; $A1-$A5 = the five tower rings, MapUpdate_CourageTwChests
  its own routine) — chest writes, story READS. Deliberate reuse: the
  game asks "did the player take this item" by testing the chest's own
  bit. Port caution: retail's $F120 door takes the RAW id (no
  subi.w #$100 — the clone's is fiction), so any caller convention
  mixing $127-style combined ids with $27-style bank ids is where an
  off-by-$100 would hide; the engine pins the arithmetic. Consequences:
  temp and chest flags never collide (the six-pairs table and the
  basement-un-looter bug 11 retract — the measured $F142 set/clear/
  respawn cycle stands, it just gates only the Xanafalgue); the LIVE
  alias is chest <-> extended-event, same-id same-bit; chest open state
  renders via the object's facing_dir byte, derived from the flag at
  load; grant and flag write are simultaneous. Oracle process lesson
  kept in oracle/README: a negative about RAM is only as good as the
  watched range — whole-RAM diff before ever reporting an absence.
- CORRECTION to the shop-counter reading: the counter is NOT gated on
  collision type $C — loc_65D12 never touches the collision grid; the
  location table is keyed by the SHOPKEEPER OBJECT's position, and the
  $C correlation is incidental (52 counters on $C, 13 on ordinary floor,
  3 on solid — a runtime gating on collision would break thirteen
  shops). The pack binds each counter to its object, fail-closed: 65 of
  68 rows land exactly on a placement.
- Three shops were cut in development and their rows left behind: table
  rows 21-23 name Tonoe at (0,0), which no object occupies — shop
  inventories 9/10/11 are referenced only by those rows and are
  unreachable in play. The location table's 0..$30 coverage stands as
  bytes; the REACHABLE shop set is 46, not 49. Tonoe's only live
  counter is its inn.
- Shop economy facts (docs/SHOPS.md): sell price is exactly half,
  rounding down (read from the lsr, not assumed); stock is structurally
  unlimited (buy lists re-read from ROM, nothing decrements); the inn
  bill is rate x occupied party slots, DEAD MEMBERS BILLED, and a night
  is the game's only full-party revive (hp/tp/status incl. death/skill
  uses, party then vehicles); the Aiedo inn (selector 6) runs
  Event_GirlsSneakingOut instead of a night while Zio and GirlsCaught
  are both clear; there is NO church/clinic mechanism anywhere — three
  counter groups only, cure and revival are otherwise items/techniques.
  Shopkeeper greetings bypass the dialogue tree entirely (selector-
  assembled text + portrait tables, most shop objects carry
  dialogue_id 0).
- CANDIDATE RETAIL BUG (engine deviates under the bug policy): the talk
  probe has no invisible-object filter, so the invisible blocker walls
  stacked on the Academy Basement bosses (type $74, bit 3 set, dialogue
  id 0) plausibly open dialogue tree entry 0 — the principal's chain —
  when pressed at on hardware. The port makes invisible no-dialogue
  objects solid-but-silent (docs/FIELD_STATE.md, invisible-blockers
  section). Hardware confirmation tape filed with the oracle.
