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

/// What a [`crate::SceneOp::PanCamera`] frames: `Event_MoveCamera`'s `d0`/`d1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraSubject {
    /// Literal operands, in pixels.
    At {
        /// `d0`.
        x: i32,
        /// `d1`.
        y: i32,
    },
    /// An object's live `$30`/`$34`, read when the op runs.
    Actor(ActorRef),
    /// An object's live `$30` and a literal `d1`: `Event_WreckageEngine`'s
    /// `move.w $30(a0), d0 / move.w #$1F0, d1`.
    ActorX {
        /// Whose X.
        actor: ActorRef,
        /// `d1`.
        y: i32,
    },
}

/// One renderer-owned operation preserved by a scene transcription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationOp {
    /// `FieldRoutine_Cutscene`'s zero return reaching `GameMode_LoadFieldMap`
    /// (`ps4.asm:120745-120746`, `:107505-107635`): the scene is over for the
    /// renderer, the field map is loaded under black, and `Pal_FadeIn`
    /// reveals it. The runtime owns the frames and emits this op once, where
    /// the reload begins; the renderer draws nothing else for the return.
    FieldReload {
        /// Black frames before the fade: the return's VInt and the map-load
        /// work, measured per map (`scenes::field_reload`).
        setup: u16,
        /// `Pal_FadeIn`'s frames, 16, or 0 when `Map_Load_Flags` bit 7 skipped
        /// it (`ps4.asm:107628`).
        fade: u16,
    },
    /// `FieldRoutine_PlaceName`'s window, up for the retail 124 frames
    /// (`ps4.asm:136552-136646`) when the travel table names the pair
    /// (`Field_Map_Index`, `Field_Map_Index_2`).
    PlaceNameWindow,
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
    /// `Event_MonsenEarthquake`'s shake (`loc_6E11E`, `loc_6E634`): for
    /// `frames` frames, a table entry goes to the four
    /// `Camera_*_Step_Counter` words and `SFXID_GraveOpening` sounds on every
    /// fourth. The scene waits the frames itself; the runtime moves the camera.
    Quake {
        /// How many frames.
        frames: u16,
        /// `loc_6E634` begins on `loc_6E73A`, two frames that also shake Y,
        /// before falling to `loc_6E6B6` like the plain one.
        opening: bool,
    },
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
    /// `move.w #$8C89, (a6)` / `move.w #$8C81, (a6)`: VDP register 12 with
    /// shadow/highlight on or off (`Event_ZioNurvus`, `$06F2F6`, `$06F3AA`).
    ShadowHighlight {
        /// Register 12's `$08` bit.
        enabled: bool,
    },
    /// Write two consecutive palette words.
    SetPaletteWords {
        /// Word offset from `Palette_Table_Buffer`: the `lea`/displacement
        /// byte offset halved.
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
    /// Give a scene-created object a destination its routine walks to:
    /// `FieldObj_GetAutoInput` closes one gap first, then the other, one cell
    /// at a time at the normal step (`$0200`, two pixels a frame). The scene's
    /// own wait carries the frames; the object walks beside it.
    WalkObject {
        /// Field-object slot.
        slot: usize,
        /// Destination X in pixels.
        x: i32,
        /// Destination Y in pixels.
        y: i32,
        /// Close the Y gap first: `Char_Move_Flags` bit 1, which
        /// `FieldObj_GetAutoInput` (`ps4.asm:93234`) reads for every object,
        /// not only the party. It must match the scene's `SetFollowMode` in
        /// effect when the walk runs, which a test of every scene checks.
        y_first: bool,
    },
    /// Give a scene-created object constant step words that its routine adds
    /// every frame from now on (`FieldObj_UpdatePosition`), as
    /// `FieldObj_StripperCoat` does with `$FFFE`/`$FFFF` in the high words.
    DriftObject {
        /// Field-object slot.
        slot: usize,
        /// Signed 16.16 X step per frame.
        step_x: i32,
        /// Signed 16.16 Y step per frame.
        step_y: i32,
    },
    /// Patch a dialogue record's portrait before opening it.
    SetDialoguePortrait {
        /// Dialogue entry whose record was patched.
        entry: u16,
        /// Retail portrait selector.
        portrait: u8,
    },
    /// A `trap #1` copy, `trap #0` clear or literal fill into one of the two
    /// palette buffers (`move.w (a0)+, (a1)+` per word: `Trap01Exception`).
    /// Every scene write of this shape targets `Palette_Table_Buffer`
    /// (`$FFFFFB00`) or `Palette_Table_Buffer_2` (`$FFFFFB80`); the runtime's
    /// CRAM model applies it.
    PaletteCopy {
        /// Where the words come from.
        source: PaletteSource,
        /// The buffer written.
        destination: PaletteBuffer,
        /// First word written (and, for a buffer source, read).
        start: u8,
        /// Words written: the `trap #1` count plus one.
        words: u8,
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
    /// What the retail `Pal_IncreaseTone` (`$58608`) / `VInt_Prepare` loop
    /// does to the palette: for `frames` VBlanks, on each frame whose
    /// `Main_Frame_Count & 3` is zero, every one of the 64 `Palette_Table_Buffer`
    /// words not already `$EEE` steps one tone per component toward white
    /// (each capped at `$E`; [`crate::palette::increase_tone`]). The record
    /// describes the palette only; the
    /// frames themselves are the scene's following [`crate::SceneOp::WaitFrames`]
    /// (`VInt_Prepare` with no map update), which the registry guard
    /// `every_palette_tone_loop_is_followed_by_its_frames` holds it to.
    PaletteIncreaseTone {
        /// Corrected loop count (the `dbra` count plus one): how many frames
        /// the tone steps over, not frames this record spends.
        frames: u16,
        /// A palette range the loop rewrites from ROM after every tone step,
        /// before the frame's `VInt_Prepare`, so it never brightens.
        hold: Option<PaletteHold>,
    },
    /// The `loc_58666` (`$058666`) / `VInt_Prepare` loop that follows a tone
    /// loop: for `frames` VBlanks, on each frame whose `Main_Frame_Count & 3`
    /// is zero, every one of the 64 `Palette_Table_Buffer` words steps each
    /// component one level back toward its copy in `Palette_Table_Buffer_2`
    /// ([`crate::palette::restore_tone`]). Blue included, and the step is on
    /// the whole word; it is not the recovery flash's restore. The frames are
    /// the scene's following [`crate::SceneOp::WaitFrames`].
    PaletteRestoreTone {
        /// Corrected loop count (`moveq #$27, d7`: 40).
        frames: u16,
    },
    /// `addi.w #add` on one `Palette_Table_Buffer` word
    /// (`Event_WreckageEngine`'s brightening screens, `$06DD6C`).
    AddPaletteWord {
        /// Word offset from `Palette_Table_Buffer`.
        offset: u16,
        /// Added to the word, as `addi.w` does (wrapping).
        add: u16,
    },
    /// `Event_Recovery`'s flash (`loc_6D3BA` and `loc_6D40A`, `ps4.asm:146867-146934`,
    /// ROM `$06D3BA..$06D467`): for `frames` VBlanks, on each frame whose
    /// `Main_Frame_Count & 3` is zero, every one of the first `words` words of
    /// `Palette_Table_Buffer` steps one tone, toward white (`brighten`: red and
    /// green up by one level until `$E`, [`crate::palette::flash_brighten`])
    /// or back toward its copy in `Palette_Table_Buffer_2` (red and green down
    /// by one until each equals the copy's, [`crate::palette::flash_restore`]).
    /// Blue is masked and kept by both. The frames themselves are the scene's following
    /// [`crate::SceneOp::WaitFrames`]: `VInt_Prepare` with no map update.
    PaletteToneFlash {
        /// Toward white, or back toward the saved buffer.
        brighten: bool,
        /// Palette words stepped: `move.w #$1F, d0`, the first two lines.
        words: u16,
        /// Corrected loop count (`moveq #$F, d7`: 16).
        frames: u16,
    },
    /// The Garuberk Tower eye's art cycle (`loc_6F91E`, `ps4.asm:148977-149019`,
    /// and its copy `loc_6F9BC`, `:149031-149073`): for `frames` VBlanks, on each
    /// frame whose `Main_Frame_Count` is even, step the ping-pong frame index in
    /// `$FFFFECF2` (0 to 2 and back; the direction in `$FFFFECF3`) and queue a
    /// DMA of that frame, `$9E0` bytes per frame from `RAM_Start`, `$4F0` words
    /// to the VRAM command `$4060`. The three frames are the art the scene
    /// decompressed first ([`PresentationOp::LoadSceneAsset`]); the frames
    /// themselves are the scene's following `WaitFrames`.
    GaruberkEyeArtCycle {
        /// Corrected loop count (`moveq #$3B, d7`: 60; `move.w #$77, d7`: 120).
        frames: u16,
    },
    /// `Pal_VariableFadeIn` (`$05B1B0`) / `Pal_VariableFadeOut` (`$05B168`)
    /// with the `$FFFFED52` delay byte. Each first copies all 64 words to
    /// `Palette_Table_Buffer_2`; the fade-in then clears the buffer and spends
    /// one `VDP_EnableDisplay` frame. A pass steps every nibble one level and
    /// waits `mode + 1` frames, and the loop ends on the pass that leaves the
    /// palette black (out) or changes nothing (in), so its length depends on
    /// the palette's contents: the scene blocks until the runtime, which holds
    /// the palette, releases it with the count
    /// ([`crate::SceneRunner::release_palette`]).
    VariablePaletteFade {
        /// True for fade-in, false for fade-out.
        fade_in: bool,
        /// Raw `Palette_Variable_Fade` mode value.
        mode: u8,
    },
    /// Rykros's six-step palette animation, `loc_78296` (`$078296..$0782D1`):
    /// for each delay byte of `loc_782D2` up to its `$FF`, nine words of the
    /// table at `source_rom_addr` (`$12` bytes a frame) go to
    /// `destination_ram`, then `VInt_Prepare` runs `delay + 1` times (`dbf
    /// d0`). The op blocks for every frame.
    RykrosPaletteCycle {
        /// ROM table containing the six nine-word palette frames.
        source_rom_addr: u32,
        /// Palette buffer address written by the helper.
        destination_ram: u32,
        /// Words copied for each frame.
        words_per_frame: u8,
        /// The `dbf` waits, already corrected to frame counts (each raw
        /// `loc_782D2` byte plus one).
        delays: &'static [u8],
    },
    /// Zio Fort's barrier ramp: `steps` writes of three consecutive palette
    /// words taken from one ROM table, `frame_delay` frames apart. The
    /// cartridge's loop reads `(a2)+`, `$C(a2)` and `$1A(a2)` with `a2`
    /// advancing two bytes per step (`Event_ZioFortBarrier`, `$06ED4E`), so
    /// the table holds the three interleaved series one after another.
    PaletteRampFromTable {
        /// ROM table the three series come from.
        source_rom_addr: u32,
        /// Palette buffer line the writes land in.
        destination_ram: u32,
        /// Table rows, one palette write each.
        steps: u8,
        /// `DoMapUpdateLoop` delay per step, already corrected to frames.
        frame_delay: u16,
    },
    /// `Event_PlateSysEngine`'s light (`loc_6E842`, `ps4.asm:147908-147920`):
    /// `steps` passes over the word table at `source_rom_addr`. Each pass
    /// reads `move.w (a1)+, d0`; a negative word (`bpl` not taken) sends the
    /// pointer back to the table's start without spending the pass. The pass
    /// then writes `d0` and the word `second_skip` bytes past the advanced
    /// pointer to the two palette words at `offset`, and waits `frame_delay`
    /// frames (`loc_6E922`, `VInt_Prepare` only: no map update). The words
    /// are the cartridge's and live in the pack, never in source; expand them
    /// with [`crate::palette::palette_cycle_writes`]. The op blocks for all
    /// the passes.
    PaletteCycleFromTable {
        /// ROM table the passes read.
        source_rom_addr: u32,
        /// Word offset from `Palette_Table_Buffer`, as [`Self::SetPaletteWords`].
        offset: u16,
        /// Bytes past the advanced pointer the second word is read from.
        second_skip: u8,
        /// `dbf` passes, already corrected (`moveq #$F, d7`: 16).
        steps: u8,
        /// Frames each pass waits.
        frame_delay: u16,
    },
    /// Move the camera to an actor's live position. This is the dynamic form
    /// of `Event_MoveCamera` used by the Raja Sick return beat.
    CameraToActor {
        /// Actor whose current coordinates are read.
        actor: ActorRef,
        /// Retail scroll speed.
        speed: u16,
    },
    /// A map object's whole-pixel position after one frame of
    /// `SceneOp::DriftNpcs` (`FieldObj_UpdatePosition`, `$04501C`). The
    /// runtime writes it to the field map so occupancy, collision and the
    /// renderer follow the drift frame by frame.
    NpcPixelPosition {
        /// The object's index in the map's object list.
        npc: usize,
        /// `curr_x_pos` integer word.
        x: i32,
        /// `curr_y_pos` integer word.
        y: i32,
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

/// The `trap #1` copy a tone loop makes each frame after `Pal_IncreaseTone`
/// (`$58608`): `Event_EnterAngerTower`'s `lea Pal_Init_Line_3, a0` / `lea
/// Palette_Line_3, a1` / `move.w #$F, d7` / `trap #1` (`$07383A..$073849`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteHold {
    /// The ROM table the words are copied from.
    pub source_rom_addr: u32,
    /// The `Palette_Table_Buffer` address they are copied to.
    pub destination_ram: u32,
    /// Words copied (`trap #1` copies `d7 + 1`).
    pub words: u8,
}

/// One of the two palette buffers a scene writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteBuffer {
    /// `Palette_Table_Buffer` (`$FFFFFB00`), the words `VInt` uploads to CRAM.
    Live,
    /// `Palette_Table_Buffer_2` (`$FFFFFB80`), the copy fades and restores
    /// step back toward.
    Saved,
}

impl PaletteBuffer {
    /// The buffer at a work-RAM address, when it is one of the two.
    #[must_use]
    pub const fn at(ram: u32) -> Option<Self> {
        match ram {
            0xFFFF_FB00 => Some(Self::Live),
            0xFFFF_FB80 => Some(Self::Saved),
            _ => None,
        }
    }
}

/// Where a [`PresentationOp::PaletteCopy`] reads its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteSource {
    /// The same words of a palette buffer.
    Buffer(PaletteBuffer),
    /// A ROM palette the pack carries (`presentation/panels.json`).
    Rom(u32),
    /// One literal value in every word (`trap #0` writes zero).
    Fill(u16),
}

/// Plane selected by a credits scroll stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPlane {
    /// Plane A is the foreground text plane.
    A,
    /// Plane B is the star field/background plane.
    B,
}
