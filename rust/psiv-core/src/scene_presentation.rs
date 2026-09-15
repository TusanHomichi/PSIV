//! Renderer-owned records emitted by scene transcriptions.
//!
//! These are deliberately data, not a second scene interpreter. The retail
//! routines write VDP windows, palette words and temporary field objects
//! directly; the headless runtime keeps those writes observable without
//! making the field core own VRAM.

use crate::scene::ActorRef;
use crate::state::CharId;

/// ROM-backed presentation assets named by the retail source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationAsset {
    /// The Meseta window tile string used by the Aiedo shop scene.
    WinTilesMeseta,
    /// The second shopkeeper portrait loaded by the same scene.
    ShopkeeperDialPortrait2,
}

/// One renderer-owned operation preserved by a scene transcription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationOp {
    /// Hide or show a party sprite while a scene-owned object replaces it.
    SetCharacterVisible {
        /// Character id, independent of party order.
        who: CharId,
        /// Whether the ordinary sprite is drawn.
        visible: bool,
    },
    /// `Pal_VariableFadeToRed`: all 64 palette entries, eight stages.
    FadeToRed {
        /// `$ED52` delay byte: each stage lasts this value plus one VBlanks.
        /// The historical field name is retained for scene-data compatibility.
        lines: u8,
    },
    /// `Pal_VariableFadeFromRed`: restore the saved green and blue components.
    FadeFromRed {
        /// `$ED52` delay byte, as in `FadeToRed`.
        lines: u8,
    },
    /// `Map_LoadChunks` after an in-place map update.
    ReloadMapChunks,
    /// Decompress a scene-owned field asset into the retail RAM scratch area.
    /// The field core does not own that scratch area, but retaining the source
    /// and destination makes the cartridge write auditable for a renderer.
    LoadSceneAsset {
        /// ROM source label/address.
        source_rom_addr: u32,
        /// Destination RAM address.
        destination_ram: u32,
    },
    /// `Field_LoadSprites`/`Field_BuildSprites` followed by the VInt prep used
    /// when the scene rebuilds the live sprite table.
    RebuildSprites,
    /// Destroy one retail dialogue/window layer.
    WindowDestroy {
        /// The `Window_Render_Mode` byte written before the call.
        render_mode: u8,
    },
    /// Save the five party sprite X positions and park them at a fixed X.
    SavePartySpriteX {
        /// The parked X position.
        parked_x: u16,
    },
    /// Restore the five party sprite X positions saved by the scene.
    RestorePartySpriteX,
    /// Write two consecutive palette words.
    SetPaletteWords {
        /// Word offset from `Palette_Table_Buffer`.
        offset: u16,
        /// First word.
        first: u16,
        /// Second word.
        second: u16,
    },
    /// Write `Game_Mode_Routine` for the shop transition.
    SetGameMode {
        /// Retail mode value.
        mode: u8,
    },
    /// Clear `Joypad_Held` before rebuilding the field sprites.
    ClearHeldInput,
    /// Create one retail dialogue/window layer.
    WindowCreate {
        /// The `Window_Render_Mode` byte written before the call.
        render_mode: u8,
    },
    /// Load a window tile string into the retail window layout.
    LoadWindowTiles {
        /// Window-group record read by the cartridge.
        group: u8,
        /// Destination VRAM word address.
        vram: u16,
        /// The priority bit passed to the loader.
        priority: u8,
        /// ROM asset selected by the source.
        asset: PresentationAsset,
    },
    /// Decompress a portrait into its retail tile number.
    LoadPortrait {
        /// ROM asset selected by the source.
        asset: PresentationAsset,
        /// Destination tile number.
        tile: u16,
    },
    /// Draw the portrait mapping onto Plane A.
    DrawPortrait {
        /// Mapping table ROM address (`loc_2A2B36`).
        mapping_rom_addr: u32,
        /// Destination X tile.
        x: u8,
        /// Destination Y tile.
        y: u8,
        /// Width in tiles.
        width: u8,
        /// Height in tiles.
        height: u8,
    },
    /// Add a character macro after constructing its field object.
    AddMacro {
        /// Party slot passed to `Event_AddMacro`.
        slot: usize,
    },
    /// Set a temporary object's destination without claiming it as a map NPC.
    SetObjectDestination {
        /// Field-object slot.
        slot: usize,
        /// Destination X in pixels.
        x: i32,
        /// Destination Y in pixels.
        y: i32,
    },
    /// Patch a dialogue record's portrait before opening it.
    SetDialoguePortrait {
        /// Dialogue entry whose record was patched.
        entry: u16,
        /// Retail portrait selector.
        portrait: u8,
    },
    /// Copy literal words between the work buffers used by a retail scene.
    CopyRamWords {
        /// Source work-RAM address.
        source_ram: u32,
        /// Destination work-RAM address.
        destination_ram: u32,
        /// Number of words copied.
        words: u16,
    },
    /// Clear a work-RAM word range with a literal value.
    ClearRamWords {
        /// Destination work-RAM address.
        destination_ram: u32,
        /// Number of words cleared.
        words: u16,
        /// Value written to each word.
        value: u16,
    },
    /// Clear a retail longword range with `trap #0` semantics.
    ClearRamLongs {
        /// Destination work-RAM address.
        destination_ram: u32,
        /// Number of longwords cleared.
        longs: u16,
        /// Value written to each longword.
        value: u32,
    },
    /// Fill a word range with a nonzero literal, as the ending's `$EEE`
    /// palette wash does after loading Termi.
    FillRamWords {
        /// Destination work-RAM address.
        destination_ram: u32,
        /// Number of words written.
        words: u16,
        /// Value written to each word.
        value: u16,
    },
    /// Clear both field planes before the staff roll text is drawn.
    ClearPlanes,
    /// Clear the final Termi object banks and rebuild both plane maps before
    /// the `ArtNem_Fin` panel is uploaded.
    EndingFinaleFieldPrep {
        /// `Character_1` base address.
        character_ram: u32,
        /// Longwords cleared from the character bank.
        character_longs: u16,
        /// `Field_Obj_Secondary` base address.
        secondary_ram: u32,
        /// Longwords cleared from the secondary bank.
        secondary_longs: u16,
        /// Final scratch-object base address.
        scratch_ram: u32,
        /// Longwords cleared from the scratch bank.
        scratch_longs: u16,
        /// Plane-map width in tiles.
        plane_width: u8,
        /// Plane-map height in tiles.
        plane_height: u8,
        /// Retail `PlaneMapToRAM2` flags.
        plane_flags: u16,
    },
    /// Run the repeated DMA/VInt loop used by the ending's fade transition.
    DmaPlanesLoop {
        /// Corrected frame count.
        frames: u16,
    },
    /// Run the retail `Pal_IncreaseTone`/`VInt_Prepare` loop.
    PaletteIncreaseTone {
        /// Corrected loop count.
        frames: u16,
    },
    /// Copy the ending's temporary palette line to zero before panel 171.
    ClearPaletteLine {
        /// Palette line index.
        line: u8,
        /// Number of words cleared.
        words: u8,
    },
    /// `Pal_VariableFadeIn`/`Pal_VariableFadeOut` with the raw mode byte.
    VariablePaletteFade {
        /// True for fade-in, false for fade-out.
        fade_in: bool,
        /// Raw `Palette_Variable_Fade` mode value.
        mode: u8,
    },
    /// Rykros's six-step palette animation from `loc_78296`.
    RykrosPaletteCycle {
        /// ROM table containing the six nine-word palette frames.
        source_rom_addr: u32,
        /// Palette buffer address written by the helper.
        destination_ram: u32,
        /// Words copied for each frame.
        words_per_frame: u8,
        /// `dbra` delays, already corrected to frame counts.
        delays: &'static [u8],
    },
    /// Move the camera to an actor's live position. This is the dynamic form
    /// of `Event_MoveCamera` used by the Raja Sick return beat.
    CameraToActor {
        /// Actor whose current coordinates are read.
        actor: ActorRef,
        /// Retail scroll speed.
        speed: u16,
    },
    /// Construct Raja Sick's temporary chest/object at `$C4C0`.
    RajaSickTemporaryObject {
        /// Field-object RAM address.
        ram_addr: u32,
        /// Retail object id.
        object_id: u16,
        /// Retail art tile.
        art_tile: u16,
        /// Corrected `DoMainUpdatesLoop` frame count.
        frames: u16,
    },
    /// Clear Raja's field-art byte in `Raja_Stats` after removal.
    RajaSickResetRaja {
        /// `Raja_Stats` RAM address.
        stats_ram: u32,
        /// Character whose roster record was reset.
        character: CharId,
        /// Byte offset cleared.
        art_offset: u8,
    },
    /// Recompute the four companion facings through `loc_77710`, using the
    /// live Raja object as the reference. The helper chooses the dominant
    /// axis and then updates/animates each field object; it is not a fixed
    /// `Face` operation.
    RajaSickArrangeParty {
        /// Character whose temporary object is the reference point.
        reference: CharId,
        /// Characters passed through the helper, in retail order.
        companions: &'static [CharId],
    },
    /// Load the three non-panel assets used by the retail credits renderer.
    EndingCreditsAssets {
        /// `ArtKos_SmallPlanet` and its VRAM byte address.
        small_planet_rom_addr: u32,
        /// Destination VRAM byte address for the small planet.
        small_planet_vram: u16,
        /// `ArtKos_LargePlanet` and its VRAM byte address.
        large_planet_rom_addr: u32,
        /// Destination VRAM byte address for the large planet.
        large_planet_vram: u16,
        /// `ArtNem_CreditFont` and its destination tile.
        credit_font_rom_addr: u32,
        /// Destination VRAM tile for the credit font.
        credit_font_tile: u16,
    },
    /// One of the three retail credits scroll loops.
    EndingCreditsStage {
        /// Stage number, one through three.
        stage: u8,
        /// `TextCounter` terminal value.
        scroll_delay: u16,
        /// Foreground camera increment per frame.
        foreground_step: u32,
        /// Background camera increment per frame.
        background_step: u32,
        /// Scratch mapping RAM read by `PlaneMapToRAM`.
        source_ram: u32,
        /// Plane rebuilt when the scroll crosses the `$13` bit.
        plane: CreditsPlane,
        /// Credit command table selected by the stage.
        commands_rom_addr: u32,
    },
    /// The credits' final palette ramp before returning to the field.
    EndingCreditsPaletteRamp {
        /// First palette table (`loc_7A274`).
        primary_rom_addr: u32,
        /// Secondary two-word table (`loc_7A2A0`).
        secondary_rom_addr: u32,
        /// Number of eight-word primary steps.
        primary_words: u8,
        /// Number of steps in the loop.
        steps: u8,
    },
    /// The two palette loops around the staff-roll title and text planes.
    EndingStaffRollTransition {
        /// Palette word offset (`Palette_Table_Buffer + $5E`).
        palette_offset: u16,
        /// Per-step decrement during the first loop.
        fade_out_step: u16,
        /// Per-step increment during the second loop.
        fade_in_step: u16,
        /// Frames between palette writes.
        frame_divisor: u8,
        /// Wait before the staff-roll music write.
        first_wait: u16,
        /// Staff-roll music id.
        music: u8,
        /// Wait after the music write.
        second_wait: u16,
        /// Palette bit that terminates the fade-in loop.
        target_bit: u8,
        /// Final plane-update hold.
        final_hold: u16,
    },
    /// The final Termi panel and palette upload after the credits scroll.
    EndingFinale {
        /// `ArtNem_Fin` source.
        art_rom_addr: u32,
        /// Destination tile.
        art_tile: u16,
        /// `loc_1DF52C` Enigma mapping.
        mapping_rom_addr: u32,
        /// Destination VRAM word address.
        mapping_vram: u16,
        /// `loc_1DF59A` palette line.
        palette_rom_addr: u32,
        /// Palette words copied.
        palette_words: u8,
        /// `loc_58666` lightning/update iterations.
        lightning_frames: u8,
    },
}

/// Plane selected by a credits scroll stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPlane {
    /// Plane A is the foreground text plane.
    A,
    /// Plane B is the star field/background plane.
    B,
}
