# Dialogue tree selection: issue #79

[Scene dialogue](SCENE_DIALOGUE.md) · [Issue #79](https://github.com/TusanHomichi/PSIV/issues/79)

## Outcome and scope

An ordinary Speak on a type-0 map area opens the cartridge-selected world
entry. NPCs and default scene dialogue retain the map-loaded tree; explicit
scene tree loads and saved dialogue cursors retain their existing contracts.
This lane changes decoding, schema validation and runtime dispatch only.
Flight/destination-menu bodies, campaign source and Godot source are untouched.

The brief is the authority for this delivery. `gh issue view 79` failed with
exit 1 (api.github.com unreachable). The quarantined prior handoff/census were
read only as reference; no patch was applied. All rules and census records
below were re-derived from the US image, SHA256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`.

## Cartridge rules and callers

Line references are to `reference/ps4disasm/ps4.asm`. That fork includes Grand
Cross additions; the matching US routine bytes decide which branch applies.
The extractor verifies instruction grammar, then reads operands and pointers.

| Rule | Source lines | US byte evidence / consequence |
| --- | --- | --- |
| Initial world is 0 | `:88670`; `ps4.constants.asm:2368` | `World_Index` is the byte at `$FFFFF400`, the high byte of the saved word |
| Map and world do not form a shared tree index | `:107530-107535`, `:107572-107577` | Map index ×4 selects `FieldMapPtrs`; its record's tree pointer loads dialogue, then its area list is retained separately |
| Map tree load | `:111677-111691` | US `$053EFE..$053F11`: read pointer, preserve a0, decompress to `$FFFF3000`; the fork's uncompressed option is absent |
| Interaction order | `:118247-118264`, `:118268-118282` | Areas, tile interaction, special/counter objects, ordinary NPCs/chests. Type-0 areas precede object talk |
| Type-0 dispatch is the only caller of world selection | `:118286-118325` | `InteractionRoutines[0]` selects `Interaction_DisplayDialogue`; US `$058844..$058871` is the English branch, including the high-entry check |
| World selection uses an unsigned byte ×4 | `:118313-118325` | US `$058848..$05886D`: clear d0, read `$F400`, double twice, load the indexed long, call `$053F00` |
| First-world high entries select the extra slot | `:118318-118325` | US `$058852..$05885D`: `bne`, `cmpi.b`, unsigned `bcs`, alternate byte offset `$18`; decoded cutoff `$7F` |
| Pointer table has six world slots plus the override | `:118403-118413` | US `$058934..$05894F`: world 0 tree 28, worlds 1–5 tree 30, extra slot tree 29. Data is emitted into ignored `dialogue/trees.json`, never copied into runtime source |
| Type-0 parameter is an entry byte, independent of map id | `:118808-118878` | Area coordinates are checked against the adjacent tile; byte 8 selects the handler and byte 9 supplies d4. Nonzero bank/flag gates test-and-set before dispatch (`:118845-118869`); story flag zero is unconditional |
| Type-0 preamble and exit | `:118330-118353`, `:118363-118397` | Read d4, find the entry; retail has the `$FA` loop and `$F6` event path, no fork-only `$FB` extension. Closing restores the map tree, so the next NPC/default scene uses its map binding |
| NPC talk keeps the loaded tree | `:118603-118626`, `:118644-118658` | US `$058B90`: clear d0, read object's byte `$14`, find entry. No world-table load. `$F3` controls object turning, not tree choice |
| Entry lookup retains the selected tree | `:119345-119360` | US `$059164`: scan `$FF` terminators in the decompressed tree; entry id is masked to a byte |
| Scene dialogue retains the current tree | `:121603-121635`, `:121659-121719` | All five `Event_GetAndRunDialogue*` wrappers call the same lookup; none reads World_Index |
| Scene tree changes are explicit | `:111680-111691`, `:121813-121814`; e.g. `:153741-153742` | `DialogueTreesToRAM` changes the loaded tree; an explicit scene load remains authoritative over the map default |
| Scene resume retains its text pointer | `TextCtrlCode_Terminate2/3`, `:142640-142645`, `:142844-142845`; `popdlg` / `Event_RunDialogue5`, e.g. `:153774-153776` | A resume is not a new world/map selection; runtime resumes retain the saved tree and flow |
| Flights can change world before changing map | `:133677-133690`, `:155599-155614` | Destination confirm writes World_Index before takeoff-map loading; Cancel does not write it |
| CrashLanding changes world after loading Dezolis | `:155837-155847` | The final write is byte 1, so selection cannot be cached by map record |
| Valid destination worlds are 0 through 5 | `:133524-133538`; `ps4.constants.asm:2368` | Six menu world indices. Runtime invalid bytes fail with a diagnostic, not a guessed tree or clamping |

`Interaction_DisplayDialogue` has no other source call site: its only dispatch
is the type-0 table row. The revision-0 and revision>0 references to
`WorldDialogueTreePtrs` are two compile-time branches, not two runtime rules.
Type-3 `Interaction_DisplayDialogue2` (`:118522-118553`) reads a direct text
pointer rather than selecting an indexed tree. Shops, TALK and direct scene
calls also use the current-tree lookup: `GetDialogueByID` calls occur at
`:118332`, `:118606`, `:121604`, `:121635`, `:121660`, `:121689`, `:121719`,
`:125791`, `:125814`, `:149083`, `:151129`, `:151248`, `:151258`, `:151298`,
`:151338`, `:151373`, `:151379`, `:151419`, `:151444`, `:154209`, `:155041`,
`:155772`, `:158040`, `:158653`, `:158662`, `:158675`, `:158682`, `:158689`.
The complete mechanically collected callsite list is retained in
`build/x79-evidence/cartridge-trace.txt`; any later fork line drift can be
checked there without trusting a build-address comment.

The cursor save/restore macros are `ps4.macrosetup.asm:152-167`, with
`Saved_Dialogue_Addr` defined at `ps4.constants.asm:2256`. US `$06A12C`
writes the text pointer word to `$ECF0`; `$F7` branches from `$06A152` to
`$06A394`, which branches to that terminator.
The fork labels that slot `$ED64`; the US byte operand is `$ECF0`. The
compressed-dialogue macro reconstructs the RAM pointer on resume without a
world-table read.

## Implementation and pack compatibility

`psiv_tools/dialogue_pack/selection.py` owns extraction. It verifies the retail
instruction shape, decodes the entry cutoff and extra-slot offset, locates the
pointer table via its `lea` operand and resolves pointers against freshly
walked Kosinski tree starts. Malformed opcodes, truncated tables and unknown
tree pointers fail emission. `psiv-data` validates six world entries and every
tree reference, including the high-entry override.

`rust/psiv-runtime/src/dialogue/selection.rs` is the sole indexed-open selector.
NPC opens, type-0 area opens and default/explicit scene opens all use it.
Existing mid-message jumps and resumes stay in the already selected tree.
System/status text and standalone choices have no indexed tree to select.
Type-0 areas open through the existing runtime window/signal path, avoiding a
new public event variant or a shell-side rule. The field dispatcher checks
areas before an NPC/chest, preserves the first eligible area order and applies
the retail bank gate before dispatch.

Older packs can still load NPC and scene dialogue. A type-0 open without the
new table emits a rebuild fault and consumes that interaction; it never falls
back to the map's unrelated text or an NPC underneath the area. The native
owner must install the rebuilt pack before testing this player-visible path.
No Godot source/API/art changes were made. **Godot-visible behavior changed:**
Speak on type-0 areas now opens the world-selected dialogue window.
The orchestrator owns the 12 certified pairs; this lane does not claim a
presentation certificate or a native connected playthrough.

## Census and issue #42

Reproduction (repository-owned census entry point):

```bash
python3 -m psiv_tools.dialogue_pack.selection_census \
  "Phantasy Star IV (USA).md" runtime-pack \
  --json build/x79-evidence/census.json \
  --markdown build/x79-evidence/census-table.md
```

All **361 real maps**, **505 type-0 areas on 145 maps**, and all six legal
world indices are covered. All 505 areas disagree with the old map binding.
For each individual area the JSON retains its ROM record offset, parameter,
selected tree for each world and disagreement bits; map tree pointers and
area metadata are independently checked against freshly decoded ROM records.
The other 56 map-pointer slots are null placeholders, not visitable maps.

The matrix includes every world a map can be visited with and also
counterfactual worlds. This is deliberately exhaustive selector coverage,
**not an exact campaign/geographic reachability census**. Static transition
lists alone cannot prove reachability: Piata Academy F1 even carries unused
Dezolis/Rykros transition records. No map/world pairs are excluded based on
geography, collision, story progression or the brief's unverified prior census.
NPC/default-scene selection is the map column on every world; an explicit
scene tree supersedes that column. In each area cell `tree(s) (N)` gives the
selected tree numbers and the number of areas differing from the map tree.
`—` means the map has no type-0 area. On W0, a mixed `28/29` cell means the
map has parameters on both sides of the decoded unsigned cutoff.

[Issue #42](https://github.com/TusanHomichi/PSIV/issues/42)'s trigger-census Tree
column in [opening triggers](00_triggers.md#which-maps-check-which-triggers)
is correct **as a map-loaded NPC/default-scene binding**, not as the tree for
all interactions. Piata's map tree 2 and Academy's tree 1 remain correct;
their type-0 areas select tree 28 on World_Index 0. Thus the ambiguous column
has the same root cause as #79. The owning doc's header/clarification is updated;
no issue was opened or closed from this lane.

<!-- BEGIN GENERATED SELECTION CENSUS -->
| Map | NPC/default scene tree | Type-0 areas | W0 | W1 | W2 | W3 | W4 | W5 |
| --- | ---: | ---: | --- | --- | --- | --- | --- | --- |
| `$000` Motavia | 43 | 0 | — | — | — | — | — | — |
| `$001` Dezolis | 43 | 0 | — | — | — | — | — | — |
| `$002` Rykros | 43 | 0 | — | — | — | — | — | — |
| `$010` Piata | 2 | 8 | 28 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) |
| `$011` PiataAcademy | 1 | 12 | 28 (12) | 30 (12) | 30 (12) | 30 (12) | 30 (12) | 30 (12) |
| `$012` PiataAcademyNearBasement | 33 | 0 | — | — | — | — | — | — |
| `$013` PiataAcademy_F1 | 1 | 14 | 28 (14) | 30 (14) | 30 (14) | 30 (14) | 30 (14) | 30 (14) |
| `$014` AcademyPrincipalOffice | 33 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$015` AcademyBasement | 33 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$016` AcademyBasement_B1 | 33 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$017` AcademyBasement_B2 | 33 | 8 | 28 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) |
| `$018` PiataDorm | 2 | 6 | 28 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) |
| `$019` PiataInn | 1 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$01A` PiataHouse1 | 1 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$01B` PiataItemShop | 1 | 0 | — | — | — | — | — | — |
| `$01C` PiataHouse2 | 1 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$01D` Mile | 3 | 10 | 28 (10) | 30 (10) | 30 (10) | 30 (10) | 30 (10) | 30 (10) |
| `$01E` MileDead | 43 | 13 | 28/29 (13) | 30 (13) | 30 (13) | 30 (13) | 30 (13) | 30 (13) |
| `$01F` MileWeaponShop | 3 | 0 | — | — | — | — | — | — |
| `$020` MileHouse1 | 4 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$021` MileItemShop | 3 | 0 | — | — | — | — | — | — |
| `$022` MileHouse2 | 4 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$023` MileInn | 3 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$024` Zema | 4 | 16 | 28 (16) | 30 (16) | 30 (16) | 30 (16) | 30 (16) | 30 (16) |
| `$025` ZemaHouse1 | 4 | 6 | 28 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) |
| `$026` ZemaWeaponShop | 4 | 0 | — | — | — | — | — | — |
| `$027` ZemaInn | 4 | 0 | — | — | — | — | — | — |
| `$028` ZemaHouse2 | 4 | 7 | 28 (7) | 30 (7) | 30 (7) | 30 (7) | 30 (7) | 30 (7) |
| `$029` ZemaHouse2_B1 | 4 | 0 | — | — | — | — | — | — |
| `$02A` ZemaItemShop | 4 | 0 | — | — | — | — | — | — |
| `$02B` BirthValley | 3 | 0 | — | — | — | — | — | — |
| `$02C` BirthValley_B1 | 3 | 0 | — | — | — | — | — | — |
| `$02D` ValleyMazeUnused | 7 | 0 | — | — | — | — | — | — |
| `$039` Krup | 5 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$03A` KrupKindergarten | 5 | 21 | 28 (21) | 30 (21) | 30 (21) | 30 (21) | 30 (21) | 30 (21) |
| `$03B` KrupWeaponShop | 5 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$03C` KrupItemShop | 5 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$03D` KrupHouse | 5 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$03E` KrupInn | 5 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$03F` KrupInn_F1 | 5 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$040` Molcum | 7 | 5 | 28 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$041` Tonoe | 7 | 9 | 28 (9) | 30 (9) | 30 (9) | 30 (9) | 30 (9) | 30 (9) |
| `$042` TonoeStorageRoom | 7 | 0 | — | — | — | — | — | — |
| `$043` TonoeGryzHouse | 7 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$044` TonoeHouse1 | 7 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$045` TonoeHouse2 | 7 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$046` TonoeInn | 7 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$047` TonoeBasement | 7 | 0 | — | — | — | — | — | — |
| `$048` TonoeBasement_B1 | 7 | 0 | — | — | — | — | — | — |
| `$049` TonoeBasement_B2 | 7 | 0 | — | — | — | — | — | — |
| `$04A` TonoeBasement_B3 | 7 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$04B` Nalya | 8 | 5 | 29 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$04C` NalyaHouse1 | 8 | 0 | — | — | — | — | — | — |
| `$04D` NalyaHouse2 | 8 | 0 | — | — | — | — | — | — |
| `$04E` NalyaItemShop | 8 | 0 | — | — | — | — | — | — |
| `$04F` NalyaHouse3 | 8 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$050` NalyaHouse4 | 8 | 6 | 29 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) |
| `$051` NalyaHouse5 | 8 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$052` NalyaInn | 8 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$053` NalyaInn_F1 | 8 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$054` Aiedo | 11 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$055` AiedoBakery | 10 | 0 | — | — | — | — | — | — |
| `$056` AiedoBakery_B1 | 10 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$057` HuntersGuild | 26 | 8 | 29 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) | 30 (8) |
| `$058` HuntersGuildStorage | 10 | 0 | — | — | — | — | — | — |
| `$059` StripClubDressingRoom | 10 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$05A` StripClub | 10 | 0 | — | — | — | — | — | — |
| `$05B` AiedoWeaponShop | 10 | 0 | — | — | — | — | — | — |
| `$05C` AiedoPrison | 11 | 0 | — | — | — | — | — | — |
| `$05D` AiedoHouse1 | 10 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$05E` ChazHouse | 10 | 9 | 29 (9) | 30 (9) | 30 (9) | 30 (9) | 30 (9) | 30 (9) |
| `$05F` AiedoHouse2 | 10 | 5 | 29 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$060` AiedoHouse3 | 11 | 6 | 29 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) | 30 (6) |
| `$061` AiedoHouse4 | 10 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$062` AiedoHouse5 | 10 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$063` AiedoSupermarket | 10 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$064` AiedoPub | 10 | 0 | — | — | — | — | — | — |
| `$065` RockyHouse | 11 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$066` AiedoHouse6 | 10 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$067` AiedoHouse7 | 10 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$068` Kadary | 9 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$069` KadaryChurch | 9 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$06A` KadaryPub | 9 | 0 | — | — | — | — | — | — |
| `$06B` KadaryPub_F1 | 9 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$06C` KadaryStorageRoom | 9 | 0 | — | — | — | — | — | — |
| `$06D` KadaryHouse1 | 9 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$06E` KadaryHouse2 | 9 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$06F` KadaryHouse3 | 9 | 5 | 29 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$070` KadaryItemShop | 9 | 0 | — | — | — | — | — | — |
| `$071` KadaryInn | 9 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$072` KadaryInn_F1 | 9 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$073` Monsen | 12 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$074` MonsenInn | 12 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$075` MonsenHouse1 | 12 | 0 | — | — | — | — | — | — |
| `$076` MonsenHouse2 | 12 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$077` MonsenHouse3 | 12 | 5 | 29 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$078` MonsenHouse4 | 12 | 4 | 29 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$079` MonsenHouse5 | 12 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$07A` MonsenItemShop | 12 | 0 | — | — | — | — | — | — |
| `$07B` Termi | 13 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$07C` TermiItemShop | 13 | 0 | — | — | — | — | — | — |
| `$07D` TermiHouse1 | 13 | 2 | 29 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$07E` TermiWeaponShop | 13 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$07F` TermiInn | 13 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$080` TermiHouse2 | 13 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$081` Passageway | 10 | 0 | — | — | — | — | — | — |
| `$082` ZioFort | 13 | 0 | — | — | — | — | — | — |
| `$083` ZioFort_Part2 | 13 | 0 | — | — | — | — | — | — |
| `$084` ZioFort_F1 | 13 | 0 | — | — | — | — | — | — |
| `$085` ZioFort_F2West | 13 | 0 | — | — | — | — | — | — |
| `$086` ZioFortWestTunnel | 13 | 0 | — | — | — | — | — | — |
| `$087` ZioFortJuzaRoom | 13 | 0 | — | — | — | — | — | — |
| `$088` ZioFortEastTunnel | 13 | 0 | — | — | — | — | — | — |
| `$089` ZioFort_F2East | 13 | 0 | — | — | — | — | — | — |
| `$08A` ZioFort_F3 | 13 | 0 | — | — | — | — | — | — |
| `$08B` ZioFort_F4 | 13 | 0 | — | — | — | — | — | — |
| `$08C` LadeaTower | 34 | 0 | — | — | — | — | — | — |
| `$08D` LadeaTower_F1 | 34 | 0 | — | — | — | — | — | — |
| `$08E` LadeaTower_F2 | 34 | 0 | — | — | — | — | — | — |
| `$08F` LadeaTower_F3 | 34 | 0 | — | — | — | — | — | — |
| `$090` LadeaTower_F4 | 34 | 0 | — | — | — | — | — | — |
| `$091` LadeaTower_F5 | 34 | 0 | — | — | — | — | — | — |
| `$092` IslandCave | 42 | 0 | — | — | — | — | — | — |
| `$093` IslandCave_F1 | 42 | 0 | — | — | — | — | — | — |
| `$094` IslandCave_F1_Part2 | 42 | 0 | — | — | — | — | — | — |
| `$095` IslandCave_Part2 | 42 | 0 | — | — | — | — | — | — |
| `$096` IslandCave_B1 | 42 | 0 | — | — | — | — | — | — |
| `$097` IslandCave_F2 | 42 | 0 | — | — | — | — | — | — |
| `$098` IslandCave_F3 | 42 | 0 | — | — | — | — | — | — |
| `$099` SoldiersTempleOutside | 42 | 0 | — | — | — | — | — | — |
| `$09A` SoldiersTemple | 42 | 0 | — | — | — | — | — | — |
| `$09B` ValleyMaze | 7 | 0 | — | — | — | — | — | — |
| `$09C` ValleyMaze_Part2 | 7 | 0 | — | — | — | — | — | — |
| `$09D` ValleyMaze_Part3 | 7 | 0 | — | — | — | — | — | — |
| `$09E` ValleyMaze_Part4 | 7 | 0 | — | — | — | — | — | — |
| `$09F` ValleyMaze_Part5 | 7 | 0 | — | — | — | — | — | — |
| `$0A0` ValleyMaze_Part6 | 7 | 0 | — | — | — | — | — | — |
| `$0A1` ValleyMaze_Part7 | 7 | 0 | — | — | — | — | — | — |
| `$0A2` BioPlant | 34 | 0 | — | — | — | — | — | — |
| `$0A3` BioPlant_Part2 | 34 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0A4` BioPlant_Part3 | 34 | 0 | — | — | — | — | — | — |
| `$0A6` BioPlant_B1 | 34 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$0A7` BioPlant_B2 | 34 | 13 | 28 (13) | 30 (13) | 30 (13) | 30 (13) | 30 (13) | 30 (13) |
| `$0A8` BioPlant_B2_Part2 | 34 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0A9` BioPlant_B3 | 34 | 0 | — | — | — | — | — | — |
| `$0AA` BioPlant_B3_Part2 | 34 | 0 | — | — | — | — | — | — |
| `$0AB` BioPlant_B4 | 34 | 0 | — | — | — | — | — | — |
| `$0AC` BioPlant_B4_Part2 | 34 | 0 | — | — | — | — | — | — |
| `$0AD` BioPlant_B4_Part3 | 34 | 0 | — | — | — | — | — | — |
| `$0AE` Wreckage | 8 | 0 | — | — | — | — | — | — |
| `$0AF` Wreckage_Part2 | 8 | 0 | — | — | — | — | — | — |
| `$0B0` Wreckage_Part3 | 8 | 0 | — | — | — | — | — | — |
| `$0B1` Wreckage_F1 | 8 | 0 | — | — | — | — | — | — |
| `$0B2` Wreckage_F1_Part2 | 8 | 0 | — | — | — | — | — | — |
| `$0B3` Wreckage_F2 | 8 | 0 | — | — | — | — | — | — |
| `$0B4` Wreckage_F2_Part2 | 8 | 0 | — | — | — | — | — | — |
| `$0B5` Wreckage_F2_Part3 | 8 | 0 | — | — | — | — | — | — |
| `$0B6` Wreckage_F2_Part4 | 8 | 0 | — | — | — | — | — | — |
| `$0B7` MachineCenter | 34 | 0 | — | — | — | — | — | — |
| `$0B8` MachineCenter_B1 | 34 | 0 | — | — | — | — | — | — |
| `$0B9` MachineCenter_B1_Part2 | 34 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0BA` PlateSystem | 13 | 0 | — | — | — | — | — | — |
| `$0BB` PlateSystem_F1 | 13 | 0 | — | — | — | — | — | — |
| `$0BC` PlateSystem_F2 | 13 | 0 | — | — | — | — | — | — |
| `$0BD` PlateSystem_F3 | 13 | 0 | — | — | — | — | — | — |
| `$0BE` PlateSystem_F4 | 13 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0BF` MotaSpaceport | 39 | 0 | — | — | — | — | — | — |
| `$0C0` ClimCenter | 17 | 0 | — | — | — | — | — | — |
| `$0C1` ClimCenter_F1 | 17 | 0 | — | — | — | — | — | — |
| `$0C2` ClimCenter_F2 | 17 | 0 | — | — | — | — | — | — |
| `$0C3` ClimCenter_F3 | 17 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0C4` WeaponPlant | 43 | 0 | — | — | — | — | — | — |
| `$0C5` WeaponPlant_F1 | 43 | 0 | — | — | — | — | — | — |
| `$0C6` WeaponPlant_F2 | 43 | 0 | — | — | — | — | — | — |
| `$0C7` WeaponPlant_F3 | 43 | 0 | — | — | — | — | — | — |
| `$0C8` VahalFort | 43 | 0 | — | — | — | — | — | — |
| `$0C9` VahalFort_F1 | 43 | 0 | — | — | — | — | — | — |
| `$0CA` VahalFort_F2 | 43 | 0 | — | — | — | — | — | — |
| `$0CB` VahalFort_F3 | 43 | 0 | — | — | — | — | — | — |
| `$0CC` Nurvus_Part2 | 35 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0CD` Nurvus_Part3 | 35 | 0 | — | — | — | — | — | — |
| `$0CE` Nurvus_B1 | 35 | 0 | — | — | — | — | — | — |
| `$0CF` Nurvus_B2 | 35 | 0 | — | — | — | — | — | — |
| `$0D0` Nurvus_B3 | 35 | 0 | — | — | — | — | — | — |
| `$0D1` Nurvus_B1Tunnel | 35 | 0 | — | — | — | — | — | — |
| `$0D2` Nurvus_B4 | 35 | 0 | — | — | — | — | — | — |
| `$0D3` Nurvus_B4_Part2 | 36 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0D4` DezoSpaceport | 39 | 0 | — | — | — | — | — | — |
| `$0D5` Nurvus_B5 | 35 | 0 | — | — | — | — | — | — |
| `$0D6` Nurvus_B3Tunnel | 35 | 0 | — | — | — | — | — | — |
| `$0D7` Nurvus | 35 | 0 | — | — | — | — | — | — |
| `$0D8` ValleyMazeOutside | 7 | 0 | — | — | — | — | — | — |
| `$0D9` ValleyMazeOutside2 | 7 | 0 | — | — | — | — | — | — |
| `$0DA` PassagewayNearAiedo | 7 | 0 | — | — | — | — | — | — |
| `$0DB` PassagewayNearKadary | 7 | 0 | — | — | — | — | — | — |
| `$0E0` Uzo | 25 | 0 | — | — | — | — | — | — |
| `$0E1` UzoHouse1 | 25 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0E2` UzoHouse2 | 25 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0E3` UzoInn | 25 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0E4` UzoHouse3 | 25 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0E5` UzoItemShop | 25 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0E6` Torinco | 24 | 0 | — | — | — | — | — | — |
| `$0E7` CulversHouse | 24 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0E8` TorincoHouse1 | 24 | 5 | 29 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$0E9` TorincoHouse2 | 24 | 3 | 29 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$0EA` TorincoItemShop | 24 | 1 | 29 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$0EB` TorincoInn | 24 | 0 | — | — | — | — | — | — |
| `$0EC` MonsenCave | 12 | 0 | — | — | — | — | — | — |
| `$0ED` RappyCave | 25 | 0 | — | — | — | — | — | — |
| `$0F0` LeRoofRoom | 40 | 0 | — | — | — | — | — | — |
| `$0F1` SilenceTm | 40 | 0 | — | — | — | — | — | — |
| `$0F2` StrengthTower | 41 | 0 | — | — | — | — | — | — |
| `$0F3` StrengthTower_F1 | 41 | 0 | — | — | — | — | — | — |
| `$0F4` StrengthTower_F2 | 41 | 0 | — | — | — | — | — | — |
| `$0F5` StrengthTower_F3 | 41 | 0 | — | — | — | — | — | — |
| `$0F6` StrengthTower_F4 | 41 | 0 | — | — | — | — | — | — |
| `$0F7` CourageTower | 40 | 0 | — | — | — | — | — | — |
| `$0F8` CourageTower_F1 | 40 | 0 | — | — | — | — | — | — |
| `$0F9` CourageTower_F2 | 40 | 0 | — | — | — | — | — | — |
| `$0FA` CourageTower_F3 | 40 | 0 | — | — | — | — | — | — |
| `$0FB` CourageTower_F4 | 41 | 0 | — | — | — | — | — | — |
| `$0FC` AngerTower | 41 | 0 | — | — | — | — | — | — |
| `$0FD` AngerTower_F1 | 41 | 0 | — | — | — | — | — | — |
| `$0FE` AngerTower_F2 | 41 | 0 | — | — | — | — | — | — |
| `$100` TheEdge | 42 | 0 | — | — | — | — | — | — |
| `$101` TheEdge_Part2 | 42 | 0 | — | — | — | — | — | — |
| `$102` TheEdge_Part3 | 42 | 0 | — | — | — | — | — | — |
| `$103` TheEdge_Part4 | 42 | 0 | — | — | — | — | — | — |
| `$104` TheEdge_Part5 | 42 | 0 | — | — | — | — | — | — |
| `$105` TheEdge_Part6 | 42 | 0 | — | — | — | — | — | — |
| `$106` TheEdge_Part7 | 42 | 0 | — | — | — | — | — | — |
| `$107` TheEdge_Part8 | 42 | 0 | — | — | — | — | — | — |
| `$108` TheEdge_Part9 | 42 | 0 | — | — | — | — | — | — |
| `$120` Tyler | 14 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$121` TylerHouse1 | 14 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$122` TylerWeaponShop | 14 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$123` TylerItemShop | 14 | 0 | — | — | — | — | — | — |
| `$124` TylerHouse2 | 14 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$125` TylerInn | 14 | 0 | — | — | — | — | — | — |
| `$126` Zosa | 17 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$127` ZosaHouse1 | 17 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$128` ZosaHouse2 | 17 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$129` ZosaWeaponShop | 17 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$12A` ZosaItemShop | 17 | 0 | — | — | — | — | — | — |
| `$12B` ZosaInn | 17 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$12C` ZosaHouse3 | 17 | 5 | 28 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$12D` Meese | 19 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$12E` MeeseHouse1 | 18 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$12F` MeeseItemShop2 | 18 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$130` MeeseItemShop1 | 18 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$131` MeeseWeaponShop | 18 | 0 | — | — | — | — | — | — |
| `$132` MeeseInn | 18 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$133` MeeseClinic | 18 | 5 | 28 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$134` MeeseClinic_F1 | 18 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$136` Jut | 23 | 0 | — | — | — | — | — | — |
| `$137` JutHouse1 | 23 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$138` JutHouse2 | 23 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$139` JutHouse3 | 23 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$13A` JutHouse4 | 23 | 5 | 28 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$13B` JutHouse5 | 23 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$13C` JutWeaponShop | 23 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$13D` JutItemShop | 23 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$13E` JutHouse6 | 23 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$13F` JutHouse6_F1 | 23 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$140` JutHouse7 | 23 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$141` JutHouse8 | 23 | 5 | 28 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) | 30 (5) |
| `$142` JutInn | 23 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$143` JutChurch | 23 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$144` Ryuon | 15 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$145` RyuonItemShop | 15 | 0 | — | — | — | — | — | — |
| `$146` RyuonWeaponShop | 15 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$147` RyuonHouse1 | 15 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$148` RyuonHouse2 | 15 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$149` RyuonHouse3 | 15 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$14A` RyuonPub | 16 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$14B` RyuonInn | 15 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$14C` RajaTemple | 14 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$14D` Reshel1 | 19 | 0 | — | — | — | — | — | — |
| `$14E` Reshel2 | 19 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$14F` Reshel3 | 19 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$150` Reshel2House | 19 | 2 | 28 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) | 30 (2) |
| `$151` Reshel2WeaponShop | 19 | 0 | — | — | — | — | — | — |
| `$152` Reshel3House1 | 19 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$153` Reshel3ItemShop | 19 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$154` Reshel3House2 | 19 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$155` Reshel3WeaponShop | 19 | 0 | — | — | — | — | — | — |
| `$156` Reshel3Inn | 19 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$157` Reshel3House3 | 19 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$158` MystVale | 17 | 0 | — | — | — | — | — | — |
| `$159` MystVale_Part2 | 17 | 0 | — | — | — | — | — | — |
| `$15A` MystVale_Part3 | 17 | 0 | — | — | — | — | — | — |
| `$15B` MystVale_Part4 | 17 | 0 | — | — | — | — | — | — |
| `$15C` MystVale_Part5 | 17 | 0 | — | — | — | — | — | — |
| `$15D` ElsydeonCave | 20 | 0 | — | — | — | — | — | — |
| `$15E` ElsydeonCave_B1 | 20 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$15F` Hangar | 14 | 0 | — | — | — | — | — | — |
| `$160` GumbiousEntrance | 38 | 0 | — | — | — | — | — | — |
| `$161` Gumbious | 38 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$162` Gumbious_F1 | 38 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$163` Gumbious_B1 | 38 | 0 | — | — | — | — | — | — |
| `$164` Gumbious_B2 | 37 | 0 | — | — | — | — | — | — |
| `$165` Gumbious_B2_Part2 | 37 | 0 | — | — | — | — | — | — |
| `$166` EspMansionEntrance | 21 | 0 | — | — | — | — | — | — |
| `$167` EspMansion | 21 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$168` EspMansionWestRoom | 21 | 0 | — | — | — | — | — | — |
| `$169` EspMansionEastRoom | 21 | 0 | — | — | — | — | — | — |
| `$16A` EspMansionNorth | 21 | 0 | — | — | — | — | — | — |
| `$16B` EspMansionNorthEastRoom | 21 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$16C` EspMansionNorthWestRoom | 21 | 4 | 28 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) | 30 (4) |
| `$16D` EspMansionCourtyard | 20 | 0 | — | — | — | — | — | — |
| `$16E` InnerSanctuary | 20 | 0 | — | — | — | — | — | — |
| `$16F` InnerSanctuary_B1 | 20 | 1 | 28 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) | 30 (1) |
| `$170` AirCastle_Part6 | 22 | 0 | — | — | — | — | — | — |
| `$171` AirCastle | 22 | 0 | — | — | — | — | — | — |
| `$172` AirCastle_Part2 | 22 | 0 | — | — | — | — | — | — |
| `$173` AirCastle_Part3 | 22 | 0 | — | — | — | — | — | — |
| `$174` AirCastle_Part4 | 22 | 0 | — | — | — | — | — | — |
| `$175` AirCastle_Part5 | 22 | 0 | — | — | — | — | — | — |
| `$176` AirCastle_F1_Part9 | 22 | 0 | — | — | — | — | — | — |
| `$177` AirCastle_F1_Part5 | 22 | 0 | — | — | — | — | — | — |
| `$178` AirCastle_F1_Part2 | 22 | 0 | — | — | — | — | — | — |
| `$179` AirCastle_F1_Part10 | 22 | 0 | — | — | — | — | — | — |
| `$17A` AirCastleInner | 22 | 0 | — | — | — | — | — | — |
| `$17B` AirCastle_F1_Part11 | 22 | 0 | — | — | — | — | — | — |
| `$17C` AirCastle_F1_Part12 | 22 | 0 | — | — | — | — | — | — |
| `$17D` AirCastle_F1_Part13 | 22 | 0 | — | — | — | — | — | — |
| `$17E` AirCastle_Part8 | 22 | 0 | — | — | — | — | — | — |
| `$17F` AirCastle_Part7 | 22 | 0 | — | — | — | — | — | — |
| `$180` AirCastle_F1_Part4 | 22 | 0 | — | — | — | — | — | — |
| `$181` AirCastle_F1 | 22 | 0 | — | — | — | — | — | — |
| `$182` AirCastle_F1_Part3 | 22 | 0 | — | — | — | — | — | — |
| `$183` AirCastle_F2 | 22 | 0 | — | — | — | — | — | — |
| `$184` AirCastleXeAThoulRoom | 22 | 0 | — | — | — | — | — | — |
| `$185` AirCastleInner_B1 | 22 | 0 | — | — | — | — | — | — |
| `$186` AirCastleInner_B1_Part2 | 22 | 0 | — | — | — | — | — | — |
| `$187` AirCastleInner_B1_Part3 | 22 | 0 | — | — | — | — | — | — |
| `$188` AirCastleInner_B2 | 22 | 0 | — | — | — | — | — | — |
| `$189` AirCastleInner_B3 | 22 | 0 | — | — | — | — | — | — |
| `$18A` AirCastleInner_B4 | 22 | 0 | — | — | — | — | — | — |
| `$18B` AirCastleInner_B5 | 22 | 0 | — | — | — | — | — | — |
| `$18C` ZelanSpace | 35 | 0 | — | — | — | — | — | — |
| `$18D` Zelan | 35 | 0 | — | — | — | — | — | — |
| `$18E` Zelan_F1 | 35 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$18F` KuranSpace | 35 | 0 | — | — | — | — | — | — |
| `$190` Kuran | 35 | 0 | — | — | — | — | — | — |
| `$191` Kuran_F1 | 35 | 0 | — | — | — | — | — | — |
| `$192` Kuran_F2 | 35 | 0 | — | — | — | — | — | — |
| `$193` Kuran_F1_Part2 | 35 | 0 | — | — | — | — | — | — |
| `$194` Kuran_F1_Part3 | 35 | 0 | — | — | — | — | — | — |
| `$195` Kuran_F1_Part5 | 35 | 0 | — | — | — | — | — | — |
| `$196` Kuran_F2_Part2 | 35 | 0 | — | — | — | — | — | — |
| `$197` Kuran_F1_Part4 | 35 | 0 | — | — | — | — | — | — |
| `$198` Kuran_F3 | 35 | 3 | 28 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) | 30 (3) |
| `$199` GaruberkTower | 22 | 0 | — | — | — | — | — | — |
| `$19A` GaruberkTower_Part2 | 22 | 0 | — | — | — | — | — | — |
| `$19B` GaruberkTower_Part3 | 22 | 0 | — | — | — | — | — | — |
| `$19C` GaruberkTower_Part4 | 22 | 0 | — | — | — | — | — | — |
| `$19D` GaruberkTower_Part5 | 22 | 0 | — | — | — | — | — | — |
| `$19E` GaruberkTower_Part6 | 22 | 0 | — | — | — | — | — | — |
| `$19F` GaruberkTower_Part7 | 22 | 0 | — | — | — | — | — | — |
| `$1A0` AirCastleSpace | 35 | 0 | — | — | — | — | — | — |
<!-- END GENERATED SELECTION CENSUS -->

## Local pack and focused evidence

Commands were run in `cx/x79-tree` on base
`f6e84f9f1799938a5b5b4baf4b4a31e0a06420fd`. The accepted `runtime-pack`
symlink remained read-only. Checks requiring the new table use
`PSIV_RUNTIME_PACK=$PWD/build/x79-pack`; test defaults remain the repository
pack, with explicit missing-input diagnostics rather than lane-local defaults.

```bash
python3 -m psiv_tools pack "Phantasy Star IV (USA).md" build/x79-pack
python3 tools/pack_diff.py runtime-pack build/x79-pack
```

The first pack build exited 2 after 198.512 seconds because this worktree
lacked ignored battle-animation PNGs (`010E38_frame00.png` was the first).
The brief authorized linking the owner's fixtures. 184 individual read-only
PNG symlinks were added without replacing the tracked fixture README or
modifying their targets; the exact list is `build/x79-evidence/fixture-links.json`.
The repaired build exited 0 in 217.601 seconds. Existing decoder anomalies
remain recorded in its raw log: 56 null map slots and 11 doorway records with
no stored map-change cells. They are not new dialogue failures.

Pack comparison exited 1 for expected differences: 5,081 files compared,
5,079 identical, two changed, none added or removed. Only
`dialogue/trees.json` (the decoded selection datum) and `manifest.json`
(its digest) differ. The old/new tree SHA256 values are respectively
`700fdfb609aa9da5ce5456f8e6dfdc346e2511d8fab0dd3329564c5a8b368cce` /
`4e12ad20bfa0274fe94f881d5884f51fdd7140ba097c7aa0c40c037e280bea2c`.
Accepted/new manifest SHA256 values are
`b6d9dd8c4d9ad6b75c9b85801ff372b812bbf8b04db3f6f7420c38ab916683bc` /
`4dbf748d8858fb3d46f32daa364ad761f6ce2d9af446208c3fa972c32cfc67e0`.
No ROM-derived table or text was committed in program source.

| Focused command | Result | Local receipt |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test session_dialogue_selection -- --test-threads=1 --nocapture` | Exit 0; 4 passed; 30.328 s including compilation | `build/x79-evidence/focused-runtime.{json,log}` |
| `CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-data -p psiv-runtime --lib selection -- --test-threads=1 --nocapture` | Exit 0; 2 data + 5 runtime passed (one unrelated name match); 33.383 s | `build/x79-evidence/focused-units.{json,log}` |
| `PYTHONPATH=. python3 -m unittest tests.test_dialogue_tree_selection -v` | Exit 0; 6 passed, no skips; 1.591 s | `build/x79-evidence/focused-python.{json,log}` |

The first focused Rust run reported three unused-Result warnings in the new
flag writes; all three were repaired before the unit and full-workspace runs.
Negative controls reject an altered unsigned branch, unknown pointer,
truncated table, wrong map binding, wrong world count and absent referenced
tree. Runtime controls reject missing tables and invalid world bytes, retain
an already-open tree and explicit scene trees, and keep NPCs map-bound.
Pad controls show Piata's old empty entry and Aiedo's missing low-tree high
entry cannot satisfy the corrected interaction; neutral input opens nothing.
These saves are isolated placement fixtures, not a connected native route.

## Release route receipt

```bash
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/x79-route --tape build/x79-route/run.tape --report build/x79-route/report.json --pack build/x79-pack
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/x79-route-accepted --tape build/x79-route-accepted/run.tape --report build/x79-route-accepted/report.json --pack runtime-pack
```

The initial release build exited 0 in 68.500 s. A post-review rebuild after
source-comment corrections exited 0 in 39.095 s and produced the identical
binary SHA256
`07bc2fd79bce7a0951e1098bb6e9fd827bc52ccc12dae5445368efdedb752eac`.
The candidate-pack route exited 0 in 35.022 s: **30 chapters, 2,617,668 frames,
completed**, digest **`0bd25017e4696c02`**. The current route source SHA256 is
`50cae11e9d4ba36c35062077ca4d20eca30e5a515240d9db44b3a45074cb0944`.
The final chapter is `mota-spaceport`; this is the existing bounded headless
route, not a native or complete-game playthrough.

The accepted-pack control exited 0 in 34.650 s with the same frozen binary and
current route. Both report JSONs, both tapes and all 31 corresponding save
files are identical. There is **no digest change between these packs**.
This isolates pack effects; it is not an old-source baseline. Both tape
SHA256 values are
`eb8181ddbc19f0d0fc0cac69937fe36b1cbeace26d0d8fd079712db0783d3835`.
The older [29-chapter receipt](../campaign/RUNNER_LOG_ZELAN.md) gives digest
`949c2abe3342e838` for 2,615,778 frames; it predates the current 30-chapter route
and is not reused as a current pass or a matched comparison.

Raw commands/timing/exits are `build/x79-evidence/release-build.{json,log}`,
`release-frozen.{json,log}`, `route.{json,log}` and `route-accepted.{json,log}`
in the same evidence directory. `route-comparison.json` records full tape and
save hashes. Gameplay artifacts are `build/x79-route/{run.tape,report.json}`
and `build/x79-route-accepted/{run.tape,report.json}`; every run used a separate
save directory, with `PSIV_SAVE_DIR` set explicitly. No owner save was opened
for writing.

## Full gate receipt

Heavy runs were serial in this lane, Cargo used two build jobs and Rust tests
one thread. `build/x79-evidence/code-freeze.json` identifies the unchanged
code/test/decoder candidate; final ledger metadata and file hashes are recorded
separately. Results below are current local results, not reused historical passes.

| Command | Result | Raw receipt |
| --- | --- | --- |
| `CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1` | Exit 0; **1,348 passed, 0 failed, 3 ignored**, 0 filtered; 41 binary/doc summaries; no warnings; 1,150.350 s | `build/x79-evidence/workspace.{json,log}`, `workspace-summary.json` |
| `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | Exit 0; no warnings; 55.042 s; `CARGO_BUILD_JOBS=2` | `build/x79-evidence/clippy.{json,log}` |
| `cargo fmt --manifest-path rust/Cargo.toml --all --check` | Exit 0; 2.066 s | `build/x79-evidence/fmt.{json,log}` |

The three ignored Rust tests were not executed:

- `an_arrival_prompt_is_the_next_objectives_to_answer`: long route to Aiedo;
  opt-in release test.
- `the_whole_route_defeats_zio_saves_and_replays`: long full-route opt-in
  release test. The separate route command passed, but does not certify every
  assertion in this ignored test.
- `battle::engine::tests::replay::replay::data::dump_manifest_entries`:
  opt-in harvesting tool which writes generated manifest entries.

No new regression was skipped with the rebuilt pack. Self-review, not an
independent review, checked the entire 19-file candidate and the pack's two
changes. The correction-ladder rung is **remove the pattern**: the one runtime
selector owns all indexed opens and pack-derived references are validated.

| Remaining gate command | Result | Raw receipt |
| --- | --- | --- |
| `PYTHONPATH=. python3 -m unittest discover -s tests` | Exit 0; **1,264 passed, 0 failed, 0 skipped**; unittest 871.857 s, wall 874.528 s | `build/x79-evidence/python.{json,log}` |
| `python3 tools/size_guard.py` | Pre-close exit 0; 1,062 scanned, 120 exempt, none over limit | `build/x79-evidence/size-preclose.{json,log}`; final `size-close.{json,log}` |
| `python3 tools/check_docs.py` | Pre-close exit 0; 180 files, 798 links, 150 anchors, 106 command paths, no problems | `build/x79-evidence/docs-preclose.{json,log}`; final `docs-close.{json,log}` |
| `git diff --check` | Pre-close exit 0; no whitespace errors | `build/x79-evidence/diff-preclose.{json,log}`; final `diff-close.{json,log}` |

The three light guards are repeated after this closeout metadata is appended;
only their final receipts authorize staging. No gameplay, decoder or test
source changed after the frozen workspace/Clippy/Python candidate. Separate
read-only overlap review found no type-0 area overlapping an earlier handler
outside the type-0/type-2 dispatch path. Its first query used the wrong rectangle
key and was corrected. That error and an earlier wrong reference-path read are
recorded with exits and corrections in `build/x79-evidence/exploratory-failures.json`;
`area-overlap-review.json` holds the corrected zero-overlap result.

## Archived delivery graph and handoff

All nodes share the brief's write-set, cartridge rules above and the base-plus
file hashes in `build/x79-evidence/{source-files,code-freeze,source-freeze}.json`.
Owner: this in-session Codex delivery worker, no delegated helpers or external
reviewer. Effort: the brief's scoped repair-until-pass instruction. This archive
records the completed dependency chain; no campaign graph or parallel-lane
source was changed. Permission remains local build/test and branch commit,
with no push, merge or issue mutation.

| Node / observable outcome | Dependencies | Acceptance and evidence | State |
| --- | --- | --- | --- |
| X79-SOURCE: every indexed caller's selector is identified; all maps/worlds are censused | None | US opcode checks, source line trace, 361-map/505-area census, Python corruption controls; `cartridge-trace.txt`, `census.json` | verified |
| X79-OWNER: every indexed open uses one owner and pack-decoded world metadata | X79-SOURCE | Fresh pack build, schema/reference controls, exactly two pack differences; `pack-repaired.json`, `pack-diff.log` | verified |
| X79-PAD: world/high-entry areas open from Speak; NPCs, scenes and cursors retain their rules | X79-OWNER | Four Session pad tests plus six schema/runtime tests and six Python controls; focused receipts and full workspace | verified |
| X79-ROUTE: existing release route still completes | X79-PAD | Both packs complete with identical digest/tape/31 saves; release and route receipts, `route-comparison.json` | verified |
| X79-GATES: frozen implementation passes every listed gate | X79-PAD | Workspace, Clippy, fmt, Python and pre-close guards above; final light guards recheck only closeout metadata before commit | verified |

The explicit 19-path source list and required co-author commit message are
retained in `build/x79-evidence/source-files.json` and `commit-message.txt`.
The final handoff records the actual commit SHA (or sandbox refusal), Git state,
final guard receipts and immutable artifact hashes. Generated pack, local
captures/receipts and fixture symlinks remain ignored and are not staged.
**Next action:** the orchestrator commits the explicit file list using the
prepared message, reviews the branch, installs the decoded pack datum for
native testing and runs the 12 certified presentation pairs.
No native presentation result is claimed by this lane.

## Commit sandbox refusal

Explicit-path staging (`git add --` followed by the 19 paths in the JSON
manifest) exited **128**: creating
`/home/peter/PSIV/.git/worktrees/x79-tree/index.lock` failed with
`Read-only file system`. `git commit -F build/x79-evidence/commit-message.txt`
also exited **128** for the same reason. No files were staged and no commit
was created; HEAD remains `f6e84f9f1799938a5b5b4baf4b4a31e0a06420fd`.
The requested fallback is complete: `source-files.json` is a JSON array of
exactly 19 regular source paths, and `commit-message.txt` ends with the required
co-author trailer. `stage.json` preserves the exact staging argv and refusal;
`commit.{json,log}` preserves the commit command, exit and raw error.
No permission workaround, push, merge, issue mutation or broad cleanup was used.
