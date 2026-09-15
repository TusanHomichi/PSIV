# Native chest interaction

The field now loads all 155 extracted chest placements into the same object
pool as NPCs. They block movement and answer normal talk input. Previously,
the pack and core rules existed, but the runtime never attached the chests
and Godot neither drew them nor handled their contents.

## Original records and art

`LoadTreasureChests` loads NPCs first, then chests. Normal chests use object
`$A0`; the plant/white boxes use `$1D4`. Their opened bits live in `$F120`,
shared with extended event flags. Facing 0 is closed; facing 4 is open.

`psiv_tools/chest_sprites.py` decodes the original Nemesis assets and both
lid mappings into the existing field-object sheet index. The 155 placements
deduplicate to three sheets: normal, map-family `$32`, and white. The legacy
`npc_placements` manifest count includes the chest users of this shared index.

| Variant | Nemesis source | VRAM tile | Mapping pointer table | Palette |
| --- | --- | --- | --- | --- |
| Normal | `$2975AE` | `$4DC` | `$5143A` | fixed CRAM line 2 |
| Map general variable `$32` | `$2976FA` | `$4DC` | `$5143A` | map CRAM line 1 |
| White | `$297674` | `$4E6` | `$51442` | fixed CRAM line 2 |

Normal mappings retain their `(0,8)..(16,32)` union bounds; white mappings
retain `(-4,6)..(20,30)`. Closed/open sequences each use the original 16-tick
single frame. `Treasure.sprite` references these sheets, and pack loading
validates that both lid sequences exist. Old packs may omit this reference.

Chest item IDs are one-based. The map exporter previously indexed its
symbol list directly with the ID, naming every item one entry too late.
For example, item 133 is Escapipe and item 141 is Alshline. The numeric
contents were already correct. Gameplay labels use the extracted cartridge
display names.

## Grant and full inventory

`FieldRoutine_ItemFound` (`ps4.asm:137246`) opens the lid and plays `$E1`
before attempting the grant. Items take the first empty slot; meseta adds
the record's amount in hundreds. A successful grant sets the chest flag
once. Reopening a claimed chest reports `It is already open.`

The runtime owns a blocking loot transaction. Movement and story triggers
stay parked until the message is acknowledged. In particular, collecting
Alshline sets chest flag `$08`, then acknowledgement allows its conversation
to start; that scene sets event flag `$32`.

When all 40 slots are occupied, the lid opens but the chest flag stays clear.
The player can choose an inventory item to discard or return the found item
to the chest. Confirmation defaults to NO. Returning closes the lid, leaves
inventory and flags unchanged, and lets the chest be opened again.

The discard guard follows `loc_67ABA`: type 9 or a zero price cannot be
discarded. The equipment JSON now includes the original `$14` price word;
older packs without that word protect the item. The discard transaction
follows `loc_67CC6`: find the first matching item ID even if a later duplicate
was selected, remove it, compact once, and put the found item in slot 39.
It does not replace the selected slot in place.

Godot uses the original window/font assets. Its normal ITEM list now pages
all 40 items in eight-row pages, with matching text/cursor spacing. Child
ITEM, target, STATE and SAVE windows draw only the moving hollow selection
marker; the previous code drew the same marker beside every option, hiding
which one was selected. The root keeps its separate red selection marker.

## Verification

- `rust/psiv-runtime/src/loot_tests.rs`: normal field input, solid objects,
  first-free grant, repeated opening, parked control, full-pack return,
  necessary-item protection, first-duplicate compaction, Alshline trigger,
  battle refresh and save reload. A separate core regression verifies that
  invalid chest/slot data cannot remove an inventory item before failure.
- Workspace tests: **853 passed** in
  `build/native-chests/workspace-tests.log`. The later atomicity guard passes
  all 20 focused state tests in `transaction-tests.log`; final strict workspace
  Clippy and the native build pass. Native captures cover the later menu fixes.
- Eleven extraction tests, including three new chest tests, pass in
  `build/native-chests/python-tests.log`. A broader 96-test pack/battle-record
  run passed 93 checks and exposed three stale assertions: chest sheet users
  were excluded, and the earlier eight shop portraits were uncategorized.
  Those assertions now account for the emitted assets; all three pass on
  rerun in `python-pack-recheck.log`.
- Four isolated Godot receipts are complete under
  `build/native-chests/{normal,meseta,white,full}/route/receipt.json`.
  Each opens the chest with normal input, checks repeat opening or full-pack
  decisions, and saves to slot 2. The full-pack run completes Alshline's
  conversation and verifies the item in slot 39. Screenshots were inspected.
- `build/native-chests/continued/receipt.json`: a fresh process uses the
  actual title CONTINUE option and loads slot 2. Inventory, flags, chest
  lids and party status match; page five visibly selects Alshline; one Down
  step followed by SAVE to slot 3 changes only the payload Y byte at `$309`
  (`$10 -> $20`). Slot 2 remains byte-identical. This run includes the final
  row-spacing and selection-marker fixes; its build hash is recorded beside
  the receipt. The earlier four receipts retain their own build hashes.

The isolated fixtures derive from the native Tonoe save, but deliberately
change location, inventory and the target chest bit. They are transaction
proof, not a campaign playthrough. The source campaign save remains
`9e27b823e34058ff8cd7d8547cf914f88ea3386b9e3395661accb9f18d516aa7`.
Native runs use software X11 rendering, fixed 60 fps and dummy audio; they
do not establish physical controller, speaker or desktop performance parity.

## Remaining fidelity work

The original full-pack window also offers USE for a carried or found item;
that branch is still missing. Its exact window geometry, two initial full
pack acknowledgements and original input timing also need comparison.
Chest-specific event staging (including the Psycho Wand and fake chest),
late map-object rewrites and a connected campaign route through these
treasures remain separate work. The connected Tonoe basement run now passes
in `build/native-alshline`: normal acquisition of item 141, its conversation,
return to Tonoe, SAVE and fresh-title Continue. `NATIVE_PLAYABILITY.md` records
its exact source chain and ending resources. Other chest-specific story paths
still need connected proof.
