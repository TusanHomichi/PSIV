//! Renderer-owned records emitted by scene transcriptions.
//!
//! These are deliberately data, not a second scene interpreter. The retail
//! routines write VDP windows, palette words and temporary field objects
//! directly; the headless runtime keeps those writes observable without
//! making the field core own VRAM.

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
    /// `Pal_VariableFadeToRed` with the retail line count.
    FadeToRed {
        /// Number of palette lines.
        lines: u8,
    },
    /// `Pal_VariableFadeFromRed` with the retail line count.
    FadeFromRed {
        /// Number of palette lines.
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
}
