# Data-driven scenes spike

Internal prototype and design note for loading scripted scenes from data rather
than Rust `static` tables (issue #793).

## Scope and status

This spike is **internal only** and does **not** commit to a 1.x public
`SceneOp` mod contract or a mod-folder schema yet.

What the spike proves:

1. **Zero-allocation compiled scenes with owned data scenes:** The slice-bearing
   `SceneOp` (and nested `PresentationOp`, `ActorAnimation`, `SceneEffect`)
   variants use `Cow<'static, [...]>` in
   [`rust/psiv-core/src/scene/ops.rs`](../../rust/psiv-core/src/scene/ops.rs),
   and `SceneRunner` accepts both `&'static [SceneOp]` (compiled retail scenes,
   zero heap allocation) and `Arc<[SceneOp]>` (dynamically loaded scenes).
2. **Fail-closed serialization and load-time validation:**
   - [`rust/psiv-data/src/data_scene.rs`](../../rust/psiv-data/src/data_scene.rs)
     defines the internal wire DTO (`DataSceneProgram` / `DataSceneOp`) with
     `#[serde(deny_unknown_fields)]` on every struct and enum.
   - [`rust/psiv-core/src/scene/validate.rs`](../../rust/psiv-core/src/scene/validate.rs)
     validates structural invariants across both compiled retail scenes and
     data-defined scenes (jump/skip bounds, flag banks, party/character bounds,
     animation tables, non-empty slices).
   - [`rust/psiv-runtime/src/data_scene.rs`](../../rust/psiv-runtime/src/data_scene.rs)
     translates DTO ops to owned `SceneOp`s and validates pack cross-references
     (`target_map`, `LoadMap`, `SkipUnlessMap`, `SetDialogueTree`,
     `RunDialogue`, map NPC indices, and patch-atlas chunks) against `GameData`.
3. **Runtime override seam and parity tests:**
   [`Runtime`](../../rust/psiv-runtime/src/scene_control.rs) keeps a
   crate-private `scene_overrides` table checked ahead of the compiled retail
   registry, exercised by
   [`rust/psiv-runtime/src/suites/data_scenes.rs`](../../rust/psiv-runtime/src/suites/data_scenes.rs).

The compiled retail scenes in `rust/psiv-core/src/scenes/` remain compiled Rust
and are **not** converted to JSON.

## Public mod vocabulary vs. internal retail helpers

`SceneOp` currently mixes general story/gameplay scripting primitives with
cartridge-specific 68k routine helpers (VDP register pokes, DMA timing loops,
and one-off retail minigame state machines). A future public mod contract
should expose a clean, documented subset and keep hardware-specific or
single-scene retail helpers internal.

| Category | Candidate public mod vocabulary | Keep internal (retail helpers) |
| --- | --- | --- |
| **Control flow** | `Jump`, `SkipOps`, `SkipUnlessMap`, `BranchFlag`, `BranchChoice`, `BranchDialogueEnd`, `BranchIfAligned`, `BranchIfActorGreater`, `BranchIfActorCoord`, `BranchIfPartyMember`, `BranchIfMoneyBelow`, `BranchIfInventoryFull`, `BranchIfVehicle`, `Return`, `End` | `BranchIfSavedDialogueByte` (raw text-stream byte offset), `BranchOnRngLow2` (`Event_RajaSick`-specific RNG nibble), `DestinationMenu` / `CommitDestination` / `LoadFlightMap` (hardcoded ship UI legs) |
| **Dialogue & flags** | `RunDialogue`, `RunDialogueResume`, `SetDialogueTree`, `SetFlag`, `ToggleFlag`, `SetNpcDialogue` | `RunDialogueResumeWithWindow` (retail window-routine selector); future public API should prefer symbolic tree IDs over raw `rom_addr` |
| **Actors & movement** | `MoveActor`, `MoveActorTo`, `MoveActorToActor`, `MoveActorToActorAxis`, `MoveActorOffset`, `PlaceActor`, `SetActorDest`, `Face`, `FaceOppositeOf`, `FaceParty`, `WaitForActor`, `DespawnNpc`, `OverlapCharacters`, `SetFollowMode`, `SetStepOffset` | `MoveActorCommand` (raw 68k movement byte), `CopyCharSlot`, `SwapCharSlots`, `PromoteNpcToChar` (raw `$C000` object-struct layout and VRAM `art_tile`), `SetArtTile`, `SetActorAnimation` (raw mapping word), `DriftNpcs`, `StepFieldObject`, `StepObjectAlongFacing`, `CreateFieldObject`, `CreateEclipseTorch`, `ObjectAnimation`, `PlacePartyY`, `Object(...)` |
| **Timing & camera** | `Wait`, `WaitFrames`, `PanCamera`, `FadeIn`, `FadeOut`, `PlaySound`, `SetSavedMusic` | `WaitForStart` (ending credits gate), `PlayMusicIfSavedDifferent`, `InitVramAndCram`, `DmaPlanes`, `PanelCreate`, `PanelDestroyLast`, `PanelDestroyAll`, `DrawTextToPlane`, `IntroTextFadeUp`, `LoadPalette`, `ReloadMapPalette`, `SetRenderSpritesInCutscene`, `SetMapLoadFlags`, `Presentation(...)` |
| **Maps & live layout** | `LoadMap`, `WriteMapChunks`, `WriteActorMapChunks`, `RestoreMapChunks`, `SetWorldIndex` | `TakeMapTransition`, `ReplaceMapLayout`, `SetVehicleIndex` |
| **Party, items & battles** | `JoinParty`, `RemovePartyMember`, `SetParty`, `AddMoney`, `SubtractMoney`, `AddItem`, `RemoveItem`, `RestorePartyHp`, `RecoverStats`, `StartBattle` | `ConfigureCharacter`, `SetCharacterEquipment`, `ClearCharacterStatus`, `SetCharacterSkill`, `LearnTechnique`, `SavePartySlots` / `RestorePartySlots`, `MarkGameCleared`, `Item(...)` (unchecked retail slot writes) |
| **One-off retail systems** | *(none)* | `Ride(...)`, `FifthCharacter(...)`, `Guild(...)`, `JobBoard(...)`, `Rocky(...)`, `GetOff(...)`, `Wreckage(...)` |

Two ergonomic upgrades belong in a public format before freezing a schema:

- **Named labels instead of raw op indices:** Replacing `usize` jump targets
  with symbolic labels (resolved to dense `usize` offsets at load time) keeps
  hand-edited or diffed mod scenes resilient to inserting/removing lines.
- **Symbolic dialogue tree and flag/item references:** Resolving dialogue trees
  by 1-based tree number or symbolic name (rather than raw `rom_addr`) decouples
  mods from cartridge byte offsets.

## Mod override and custom event registration model

When data-driven scenes graduate to the mod system (see
[Modding guide](../MODDING.md)), event dispatch should follow the same overlay
precedence as the rest of `psiv-pack` / `psiv-data`:

1. **Event ID lookup order:**
   `active mods (in load order, last writer wins)` -> `compiled retail registry`.
   - **Overriding an existing retail event:** A mod ships a scene declaring an
     existing retail `event_id` (e.g., `0x00A0` for `Event_PiataChazAlone`).
     At load time, the runtime validates the scene against the merged `GameData`
     and registers it in `scene_overrides`, shadowing the compiled scene without
     patching Rust code.
   - **Registering a new custom event:** A mod declares an unused `event_id` in
     a reserved mod range (for example `0x4000..=0x7FFF` for field events and
     `0xC000..=0xFFFF` for bit-15 cutscenes, keeping retail's low-bit/bit-15
     return semantics intact) and wires a map trigger, interaction area, or
     dialogue `$F6` event hook to that ID.
2. **Fail-closed validation at pack load:** Every mod scene is parsed,
   lowered, structurally validated (`validate_scene_ops`), and cross-checked
   against the merged pack (`maps`, `dialogue`, `npcs`, `patch_tiles`) before
   gameplay starts. Any invalid scene aborts mod loading with the file name,
   scene name, op index, and reason rather than faulting mid-playthrough.
3. **Conflict reporting:** If two enabled mods override the same `event_id`,
   log an explicit load diagnostic naming both mods and the winning override
   (matching the existing asset-override diagnostics).

## Recommended phased rollout

1. **Phase 0 (complete — this spike):** `Cow<'static, [...]>` slices on
   `SceneOp`, `SceneRunner::from_owned`, shared structural validator, internal
   DTO + pack cross-reference validation, and crate-private `Runtime` override
   hook with equivalence and fail-closed negative-control tests.
2. **Phase 1 (1.x experimental, opt-in):** Introduce a narrow, label-based
   public JSON schema under `docs/schemas/` covering only the stable public
   vocabulary above (dialogue, flags, basic branching, actor movement/facing,
   waits, camera pan, sound, map load, party/item/money/battle). Support
   mod-folder `scenes/*.json` loading behind a clear schema version so early
   modders can override events and add custom side-quests without exposing
   68k presentation/minigame internals.
3. **Phase 2 (2.0 stable mod scripting):** Add symbolic names for flags, items,
   characters, and dialogue trees; support custom map triggers / `$F6` dialogue
   hooks in map overlays; publish a schema compatibility guarantee alongside
   the existing pack schemas in `docs/MODDING.md`.
