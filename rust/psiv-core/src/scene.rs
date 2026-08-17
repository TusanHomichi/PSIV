//! The scene interpreter: scripted cutscenes as data.
//!
//! The cartridge's ~162 `Event_*` routines are hand-written 68k, but composed
//! almost entirely from a small vocabulary — `Event_UpdateObjFacing`,
//! `Event_GetAndRunDialogue`, `Event_MoveCamera`, `Event_AddMacro`,
//! move-object-and-wait loops, `Current_Party_Slots` writes, flag sets, NPC
//! despawns. This module implements that vocabulary once as [`SceneOp`] and
//! runs sequences of it deterministically.
//!
//! # Actor motion is the same movement model as everything else
//!
//! A scripted walk is not a special animation: the cartridge's
//! `Event_MoveSingleObject` / `Event_MoveCharacters` set an object's
//! `dest_x_pos`/`dest_y_pos` and then call the ordinary object update until
//! `curr == dest`. The object's direction comes from `FieldObj_GetAutoInput`
//! (`ps4.asm:93232`), which compares current against destination one axis at a
//! time — X first unless `Char_Move_Flags` bit 1 is set. So a scene actor
//! walks in whole cell-steps at the same rate as the party, and turns to face
//! the way it walks. [`ScriptedActor`] is exactly that.
//!
//! # Blocking
//!
//! The runner executes ops until it reaches one that blocks — a wait, a
//! dialogue, a choice, or the end. Everything non-blocking in between runs in
//! the same tick, which is what makes a "set three flags and move the camera"
//! run of ops take one tick rather than three.
//!
//! Dialogue is the interesting one: the runner emits
//! [`SceneEffect::DialogueOpen`] and then stops advancing until the runtime
//! reports the window closed with [`SceneInput::DialogueClosed`]. The engine
//! neither draws nor times the window; it only knows the script is waiting.

use crate::field::StepFrames;
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;
use crate::scene_presentation::PresentationOp;
use crate::state::{CharId, Flag, PARTY_SLOTS};

pub use crate::scene_types::{SceneEffect, SceneFault, SceneInput};

/// How many ops one tick may execute before the runner assumes the script is
/// looping and faults. Generous for real scenes, finite for broken ones.
pub const OP_BUDGET_PER_TICK: usize = 1024;

/// Which coordinate a comparison reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    /// `curr_x_pos`.
    X,
    /// `curr_y_pos`.
    Y,
}

/// Who a scene op is talking about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorRef {
    /// A party member by slot; 0 is the leader.
    PartyMember(usize),
    /// A field object by its index in [`FieldMap::npcs`].
    ///
    /// Scene-lane proved these live at field-object RAM `$FFFFC300 + N*$40`,
    /// so "NPC index N" is an unambiguous handle into the current map.
    Npc(usize),
    /// A character, resolved to whichever field object currently holds them
    /// via the party slots — the `Event_GetCharacter` (`$5A6D6`) primitive.
    /// Lets a scene name Chaz or Alys instead of guessing a slot.
    Character(CharId),
}

/// A dialogue tree entry to open. The engine carries no text — the id is
/// resolved against the pack by the runtime.
///
/// `GetDialogueByID` (`$59164`) skips `d0` `$FF` terminators through whichever
/// tree was decompressed to `$FFFF3000`, so an entry index is only meaningful
/// against the *current* tree. [`SceneOp::SetDialogueTree`] changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DialogueId(pub u16);

/// Which dialogue-window setup a scene asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DialogueWindow {
    /// `Event_GetAndRunDialogue` (`$5AC66`) — every scene but one.
    #[default]
    Standard,
    /// `Event_GetAndRunDialogue5` (`$5ADF8`) — `Cutscene_PiataPrincipal`.
    Cutscene,
    /// `Event_GetAndRunDialogue3`, used by the Rykros surface.
    Cutscene3,
    /// The ending's first `RunText3` window before the panel sequence.
    EndingIntro,
    /// `Event_GetAndRunDialogue4` / `Event_RunDialogue4` in `$8021`.
    Ending,
    /// `Event_RunDialogue5` used for the Rykros hand-off line.
    Cutscene5,
}

/// Where a dialogue entry index comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DialogueSource {
    /// A literal entry index written into the scene.
    Entry(DialogueId),
    /// Read `dialogue_id` (`$14`) off a field object. `Event_AlysFound` does
    /// exactly this — `move.b $14(a4),d0` — so the line Alys speaks is a
    /// property of her map object, not of the scene.
    NpcDialogueId(ActorRef),
}

/// One instruction of a scene.
///
/// Jump targets are op indices into the same scene slice, so a scene is
/// self-contained data with no labels to resolve at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneOp {
    /// Start `actor` walking to `to`. Does **not** block; pair it with
    /// [`SceneOp::WaitForActor`] when the script should wait.
    MoveActor {
        /// Who walks.
        actor: ActorRef,
        /// Where to.
        to: Cell,
    },
    /// Turn `actor` to face another actor head-on.
    ///
    /// `Event_PrincipalConfession` does `move.w facing_dir(leader),d0 / bchg
    /// #2,d0` — bit 2 is the axis-flip within each facing pair, so the result
    /// is the exact opposite of the other actor's facing (down<->up,
    /// right<->left). That is [`Direction::opposite`], and it makes the
    /// principal turn toward the party from whichever side they approached.
    /// Note it reads the *leader's* facing, which after `Event_AlysFound` is
    /// Alys.
    FaceOppositeOf {
        /// Who turns.
        actor: ActorRef,
        /// Whose facing is flipped.
        of: ActorRef,
    },
    /// Place an actor at a pixel position outright, setting both current and
    /// destination — staging, not walking.
    PlaceActor {
        /// Who.
        actor: ActorRef,
        /// X in pixels.
        x: i32,
        /// Y in pixels.
        y: i32,
    },
    /// Set an actor's destination without starting a scripted walk, so the
    /// ordinary follow logic drives them there.
    SetActorDest {
        /// Who.
        actor: ActorRef,
        /// X in pixels.
        x: i32,
        /// Y in pixels.
        y: i32,
    },
    /// Exchange two character objects wholesale — the three-way `trap #1`
    /// through the `$E200` scratch buffer that `Event_GameStart` uses to put
    /// Alys in front.
    SwapCharSlots {
        /// One character-object index.
        a: usize,
        /// The other.
        b: usize,
    },
    /// Re-upload the current map's palette (`Map_Palettes_Addr` ->
    /// `Palette_Table_Buffer`, 48 words in two copies).
    ReloadMapPalette,
    /// Clear VRAM and CRAM (`InitVRAMAndCRAM`, `$5A658`): fades out, resets a
    /// VDP register and rebuilds the Plane A buffer. Engine-visible effect is
    /// the fade; the rest is the renderer's.
    InitVramAndCram,
    /// Return from the scene with an explicit value.
    ///
    /// The value is part of the contract, not a C convention. For a
    /// **cutscene**, `FieldRoutine_Cutscene` reloads the map when `d0 == 0`
    /// (`bset #2, Map_Load_Flags` then `GameMode_LoadFieldMap`), which is how
    /// `Cutscene_PiataPrincipal` gets the office back with post-briefing NPC
    /// state. For a plain **event**, a non-zero return is what suppresses the
    /// map reload — `Event_IgglanovaBattle` relies on that so its battle
    /// hand-off survives.
    Return {
        /// The `d0` value.
        value: u16,
    },
    /// Jump on whether one actor's coordinate is greater than another's.
    ///
    /// `Event_MeetingHahn` picks its walk direction with
    /// `cmp.w hahn_x, leader_x / bhi` — unsigned, so this is a strict
    /// greater-than on `a` against `b`.
    BranchIfActorGreater {
        /// The actor whose coordinate is on the left of the comparison.
        a: ActorRef,
        /// The one on the right.
        b: ActorRef,
        /// Which coordinate.
        axis: Axis,
        /// Op index taken when `a`'s coordinate is greater.
        if_greater: usize,
        /// Op index taken otherwise.
        if_not: usize,
    },
    /// Load a palette from ROM (intro only).
    LoadPalette {
        /// The palette's ROM address.
        rom_addr: u32,
        /// How many words to copy.
        words: u16,
    },
    /// Decompress one Nemesis art blob into a tile (`LoadVRAMAddressFromTileNumber`
    /// plus `NemDecomp`).
    LoadArt {
        /// The retail ROM source address.
        rom_addr: u32,
        /// The destination tile number.
        tile: u16,
    },
    /// Park the camera at a pixel position without scrolling (intro only).
    SetCameraPos {
        /// X in pixels.
        x: i32,
        /// Y in pixels.
        y: i32,
    },
    /// Upload the title image (intro only).
    LoadTitleImage {
        /// Art ROM address.
        art: u32,
        /// Plane-mapping ROM address.
        mapping: u32,
        /// Columns.
        width: u16,
        /// Rows.
        height: u16,
        /// Destination VRAM address.
        vram: u32,
    },
    /// Set the prologue text colour (intro only).
    SetTextColour {
        /// A CRAM colour word.
        colour: u16,
    },
    /// Draw a dialogue entry straight onto a plane. The prologue crawl and
    /// retail ending staff roll use this instead of a dialogue window.
    DrawTextToPlane {
        /// Which entry of the current tree.
        entry: u16,
        /// Plane buffer address.
        plane: u32,
        /// Destination VRAM address.
        vram: u32,
    },
    /// Ramp the prologue text colour up one step (intro only). The routine
    /// acts one frame in four and adds `$222` per step.
    IntroTextFadeUp,
    /// Ramp the prologue text colour down one step (intro only).
    IntroTextFadeDown,
    /// Turn `actor` in place — a **direct facing write**, not a movement.
    ///
    /// Scenes are not bound by the walker's no-turn-in-place rule. Input-driven
    /// movement commits a whole 16-pixel step the moment a direction is pressed
    /// toward a walkable cell (measured on hardware: a one-frame tap moves a
    /// full cell), so the party only turns without moving against a blocked
    /// cell. `Event_UpdateObjFacing` (`$5A936`) has no such constraint — it
    /// writes `facing_dir` and returns, which is how `Event_AlysFound` turns
    /// Alys to face right without her taking a step.
    Face {
        /// Who turns.
        actor: ActorRef,
        /// Which way.
        facing: Direction,
    },
    /// Open a dialogue and block until the runtime closes it.
    RunDialogue {
        /// Which entry, and where its index comes from.
        source: DialogueSource,
        /// Which window the cartridge opens. `Event_GetAndRunDialogue`
        /// (`$5AC66`) and `Event_GetAndRunDialogue5` (`$5ADF8`) fetch the same
        /// text and differ only in window setup, which is the renderer's
        /// business — but the sequence records which was called.
        window: DialogueWindow,
    },
    /// Resume the dialogue where `RunText` left off.
    ///
    /// `RunText` always records its stopping point in `Saved_Dialogue_Addr`
    /// (`$ECF0`); this is the clone's `popdlg` idiom. Blocks like
    /// [`SceneOp::RunDialogue`].
    RunDialogueResume,
    /// Resume a saved dialogue through a named retail window routine.
    RunDialogueResumeWithWindow {
        /// The retail window routine selected for the resume.
        window: DialogueWindow,
    },
    /// Point the dialogue system at a different tree (`DialogueTreesToRAM`,
    /// `$53F00`). Entry indices after this are relative to the new tree.
    SetDialogueTree {
        /// The tree's ROM address, as the transcription records it.
        rom_addr: u32,
    },
    /// Set or clear an event flag.
    SetFlag {
        /// Which flag.
        flag: Flag,
        /// The value to write.
        value: bool,
    },
    /// Put a character in a party slot.
    JoinParty {
        /// Which slot, 0-based.
        slot: usize,
        /// Who joins.
        who: CharId,
    },
    /// Remove a character and close the resulting party-slot gap.
    RemovePartyMember {
        /// Who leaves.
        who: CharId,
    },
    /// Replace the whole party at once — the shape `Event_AlysFound` uses.
    SetParty {
        /// The five slots.
        slots: [Option<CharId>; PARTY_SLOTS],
    },
    /// Copy the five party slots to retail's transient `Saved_Char_ID_Mem`.
    ///
    /// These bytes bridge separate scene dispatches (the Elsydeon cave and
    /// the Anger Tower top). They are scene/runtime state, not save-file
    /// state, so the save serializer deliberately never sees them.
    SavePartySlots,
    /// Restore the slots captured by [`SceneOp::SavePartySlots`].
    RestorePartySlots,
    /// Add `amount` HP to every occupied party record and clear its status.
    RestorePartyHp {
        /// The retail healing amount.
        amount: u16,
    },
    /// Write a character's retail equipment and optionally restore HP/TP.
    ConfigureCharacter {
        /// The roster record.
        who: CharId,
        /// Right hand, left hand, head, body.
        equipment: [u8; 4],
        /// Whether the source copies max HP/TP into current HP/TP.
        restore_hp_tp: bool,
    },
    /// Patch only the equipment slots the retail routine writes, preserving
    /// the other two slots on the roster record.
    SetCharacterEquipment {
        /// The roster record.
        who: CharId,
        /// Right hand, left hand, head, body; `None` leaves a slot intact.
        slots: [Option<u8>; 4],
    },
    /// Clear a character's status byte.
    ClearCharacterStatus {
        /// The roster record.
        who: CharId,
    },
    /// Apply the retail bug-fix tail that revives Chaz when HP is zero.
    ReviveIfDead {
        /// The character to inspect.
        who: CharId,
    },
    /// Remove one or more consecutive field objects from the map.
    ///
    /// **The width is load-bearing.** The despawn is a `trap #0` block clear
    /// over `d7 + 1` longwords, and one object struct is `$40` bytes — so
    /// `#$F` clears one object and `#$2F` clears three. `Event_AlysFound`
    /// clears one; `Event_MeetingHahn` clears **three**, because map `$12`'s
    /// NPC 0 is Hahn and NPCs 1-2 are the `InvisibleBlock` pair fencing him in.
    /// Modelling this as "clear one" leaves two invisible walls across the
    /// basement corridor.
    DespawnNpc {
        /// The first object cleared.
        npc_index: usize,
        /// How many consecutive objects the block clear covers.
        count: usize,
    },
    /// Move the camera to a pixel position at `speed`.
    ///
    /// `Event_MoveCamera` (`$5AAEE`) subtracts `$98`/`$58` for the half-screen
    /// offset itself, so these are the values the scene passes, untouched.
    MoveCamera {
        /// Target X in pixels.
        x: i32,
        /// Target Y in pixels.
        y: i32,
        /// Scroll speed.
        speed: u16,
    },
    /// Walk an actor to a pixel position — `Event_MoveCharacters` (`$5AA84`,
    /// the party, followers in tow) or `Event_MoveSingleObject` (`$5A9FC`).
    MoveActorTo {
        /// Who walks.
        actor: ActorRef,
        /// Target X in pixels.
        x: i32,
        /// Target Y in pixels.
        y: i32,
        /// Whether the scene spins until the walk completes. The retail
        /// primitives drive the object until `curr == dest`, so this is `true`
        /// for every transcribed use; `false` exists for staging moves that
        /// the following ops are expected to overlap.
        wait: bool,
    },
    /// Walk `actor` to the current cell of `target`. Retail uses this shape
    /// when it copies Gryz's live position into Character_1 before opening
    /// Tonoe's basement door; a literal destination would lose that
    /// map-state dependency.
    MoveActorToActor {
        /// Who walks.
        actor: ActorRef,
        /// Whose current cell is the destination.
        target: ActorRef,
        /// Whether to block until the walk completes.
        wait: bool,
    },
    /// Walk `actor` to the target actor's coordinate on one axis while
    /// preserving the actor's other coordinate. This is the exact shape of
    /// Rika's opening alignment: `Event_MoveSingleObject` receives the
    /// leader's X and the temporary object's own Y.
    MoveActorToActorAxis {
        /// Who walks.
        actor: ActorRef,
        /// Whose coordinate supplies the destination.
        target: ActorRef,
        /// Which coordinate to copy.
        axis: Axis,
        /// Whether to block until the walk completes.
        wait: bool,
    },
    /// Walk an actor by a live offset from its current cell. The source uses
    /// this for Rika's `$10,$10` look-around beat after the Zema map load.
    MoveActorOffset {
        /// Who walks.
        actor: ActorRef,
        /// X offset in pixels.
        dx: i32,
        /// Y offset in pixels.
        dy: i32,
        /// Whether to block until the walk completes.
        wait: bool,
    },
    /// Drive an actor with one of the NPC movement-command bytes and
    /// optionally spin until the step completes — the `AlysFound` idiom of
    /// calling the NPC's own field routine with `d0 = command`.
    ///
    /// The command byte indexes the cartridge's NPC movement-command table,
    /// which this crate does not carry, so the runner reports it rather than
    /// executing it. Prefer [`SceneOp::MoveActorTo`] where a transcription can
    /// express the move as a destination.
    MoveActorCommand {
        /// Who moves.
        actor: ActorRef,
        /// The movement-command byte.
        command: u8,
        /// Whether the scene spins until the step finishes.
        wait: bool,
    },
    /// Collapse the followers onto the leader (`Event_OverlapCharacters`,
    /// `$5A87A`).
    OverlapCharacters,
    /// Turn an NPC into a party character: the `trap #1` struct copy plus
    /// field-object construction that makes NPC-Alys become field-Alys.
    PromoteNpcToChar {
        /// The map object being promoted.
        npc: usize,
        /// Who they become.
        char_id: CharId,
        /// Which party slot they take.
        slot: usize,
        /// The object's art tile.
        art_tile: u16,
        /// Their facing on arrival.
        facing: Direction,
    },
    /// Add to the purse (`Current_Money`).
    AddMoney {
        /// How much.
        amount: u32,
    },
    /// Hand control to a battle (`Event_Battle_Index` + the routine-exit bit).
    StartBattle {
        /// The event battle index.
        index: u16,
    },
    /// Write `Char_Move_Flags` (`$ECFE`): bit 0 disables the follow chain,
    /// bit 1 picks Y-first auto-pathing, bit 2 locks the camera.
    SetFollowMode {
        /// The raw flag bits.
        bits: u8,
    },
    /// Write `FieldObj_Step_Offset` (`$ECE0`): 0 slower, 1 normal, 2 faster.
    SetStepOffset {
        /// The raw value.
        value: u8,
    },
    /// Play a sound, music track, or the stop-music byte — one `Sound_Index`
    /// write covers all three.
    PlaySound {
        /// The sound id.
        id: u8,
    },
    /// Set `Saved_Sound_Index`, the track restored after an interruption.
    SetSavedMusic {
        /// The sound id.
        id: u8,
    },
    /// Fade the palette in (`Pal_FadeIn`, `$421D4`).
    FadeIn,
    /// Fade the palette out (`PalFadeOut_ClrSpriteTbl`, `$4223E`).
    FadeOut,
    /// Load a map and place the party — the scene-driven counterpart to a warp.
    LoadMap {
        /// The map to load.
        map: u16,
        /// What `Field_Map_Index` should report as the previous map.
        prev_map: u16,
        /// `Map_Start_X_Pos`, in 8-pixel units as the loader stores it.
        start_x: u16,
        /// `Map_Start_Y_Pos`, in 8-pixel units.
        start_y: u16,
        /// `Map_Start_Facing_Dir`.
        facing: Direction,
        /// `Map_Start_Char_Align`. Consumed by `loc_535D4` alongside facing
        /// when placing followers; not fully decoded.
        align: u8,
        /// `Map_Load_Flags` bits to clear before `RefreshMap`.
        clear_load_flags: u8,
    },
    /// Whether sprites render during a cutscene
    /// (`Render_Sprites_In_Cutscenes`, `$ECFD`).
    SetRenderSpritesInCutscene {
        /// The new value.
        enabled: bool,
    },
    /// Wait without running a map update — `VInt_PrepareLoop` (`$5A7AC`),
    /// which is a *different* wait from [`SceneOp::Wait`].
    WaitFrames {
        /// How many frames.
        frames: u16,
    },
    /// Block for a fixed number of ticks, running map updates
    /// (`DoMapUpdateLoop`, `$5A71E`).
    ///
    /// The retail loop is a `dbra`, so a scene's literal `d0` runs `d0 + 1`
    /// times; transcriptions record the **already-corrected** tick count.
    Wait {
        /// How many.
        ticks: u16,
    },
    /// Block until `actor` finishes walking.
    WaitForActor {
        /// Who to wait for.
        actor: ActorRef,
    },
    /// Block on the retail ending's `Joypad_Pressed` Start loop.
    WaitForStart,
    /// Jump depending on a flag.
    BranchFlag {
        /// Which flag.
        flag: Flag,
        /// Op index taken when set.
        if_set: usize,
        /// Op index taken when clear.
        if_clear: usize,
    },
    /// Ask the player a yes/no question and jump on the answer. Blocks until
    /// [`SceneInput::Choice`] arrives.
    BranchChoice {
        /// Op index taken on yes.
        if_yes: usize,
        /// Op index taken on no.
        if_no: usize,
    },
    /// Jump on whether two actors share a coordinate.
    ///
    /// `Event_AlysFound` opens with `cmp.w $34(a3),d0` — Chaz's `curr_y_pos`
    /// against Alys's — to skip the alignment step when she is already level
    /// with him. Scene-lane flagged this as the act's only *position* branch
    /// and asked that [`SceneOp::BranchFlag`] not be silently widened to cover
    /// it, so it gets its own op.
    BranchIfAligned {
        /// One actor.
        a: ActorRef,
        /// The other.
        b: ActorRef,
        /// Which coordinate to compare.
        axis: Axis,
        /// Op index taken when the coordinates match.
        if_aligned: usize,
        /// Op index taken otherwise.
        if_not: usize,
    },
    /// Split the retail mounted/foot path on `Vehicle_Index`.
    BranchIfVehicle {
        /// Op index taken when a vehicle is mounted.
        if_mounted: usize,
        /// Op index taken on foot.
        if_on_foot: usize,
    },
    /// Copy one character *field object's* whole struct over another — the
    /// `trap #1` 32-word copy that relocates Chaz's on-map object from
    /// `Character_1` to `Character_2`.
    ///
    /// This moves the **object**, not the party slot. The two are separate in
    /// the cartridge: `Current_Party_Slots` says who is in the party and the
    /// `$C000`-series objects are their on-map bodies. `Event_AlysFound`
    /// writes the slots first and copies the struct second, and conflating
    /// them silently overwrites whoever the slot write just installed.
    CopyCharSlot {
        /// Source character-object index (0 = `Character_1`).
        from: usize,
        /// Destination character-object index.
        to: usize,
    },
    /// Set a field object's art tile (`$16`).
    SetArtTile {
        /// Whose.
        actor: ActorRef,
        /// The VRAM tile number.
        tile: u16,
    },
    /// Select the active vehicle (`Vehicle_Index`).
    SetVehicleIndex {
        /// Retail vehicle id.
        index: u16,
    },
    /// Create or replace one of the VDP panel images used by the retail
    /// presentation routines (`Panel_Create`). The panel allocator and DMA
    /// are renderer work; retaining the literal id keeps the transcription
    /// auditable without pretending the field engine owns VRAM.
    PanelCreate {
        /// The retail panel id.
        id: u16,
    },
    /// Destroy one panel image (`Panel_Destroy`).
    PanelDestroy {
        /// The retail panel id.
        id: u16,
    },
    /// Pop the most recently created panel (`Panel_Destroy` with no id in the
    /// source). The allocator is stack based; the caller supplies no literal.
    PanelDestroyLast,
    /// Destroy every panel image currently staged by the scene.
    PanelDestroyAll,
    /// Flush the staged planes during a panel transition (`DMAPlanes_VInt`).
    DmaPlanes,
    /// Animate a temporary field object. These are the small effect objects
    /// the cartridge places outside the ordinary map-NPC list for Flaeli,
    /// Saya and Igglanova's fusion sequence. The runtime carries the literal
    /// fields as presentation data; ordinary actor walks use `MoveActorTo` so
    /// they still land in [`FieldMap`] and remain comparator-visible.
    ObjectAnimation {
        /// The field-object slot the retail routine writes.
        slot: usize,
        /// The retail object id.
        object_id: u16,
        /// The art tile loaded for the object.
        art_tile: u16,
        /// Number of animation frames or update-loop iterations.
        frames: u16,
    },
    /// Preserve renderer-owned writes whose state is not part of the field
    /// core. The typed payload records the retail primitive and literals;
    /// the runtime is free to consume it without changing scene control flow.
    Presentation {
        /// The renderer-owned record.
        op: PresentationOp,
    },
    /// Remove the first inventory slot containing `item`, matching the
    /// cartridge's item-removal loop and leaving the resulting hole intact.
    RemoveItem {
        /// The item id.
        item: u8,
    },
    /// Add an item through the first-free inventory slot.
    AddItem {
        /// The item id.
        item: u8,
    },
    /// Record raw `Map_Load_Flags` writes surrounding a retail map refresh.
    /// The loader consumes these bits; the scene runner keeps them as a
    /// presentation/data effect because its `LoadMap` op already names the
    /// state that matters to the headless map build.
    SetMapLoadFlags {
        /// Bits set by the scene.
        set: u8,
        /// Bits cleared by the scene.
        clear: u8,
    },
    /// Retail's `RecoverStats` presentation/state refresh between the second
    /// dialogue and the Zema map rebuild. Battle-derived stats already live in
    /// the runtime roster; this edge remains explicit for the transcription.
    RecoverStats,
    /// Set the volatile `Game_Cleared_Flag` after the player dismisses the
    /// final Termi scene. This is not save serialization state.
    MarkGameCleared,
    /// Unconditional jump.
    Jump {
        /// Op index.
        to: usize,
    },
    /// Finish the scene.
    End,
}

/// A scene actor: a cell position, a facing, and a walk in progress.
///
/// Movement is the auto-input model: close the X gap first, then the Y gap,
/// one whole cell-step at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedActor {
    /// Who this is.
    pub actor: ActorRef,
    /// Current cell.
    pub cell: Cell,
    /// Current facing.
    pub facing: Direction,
    /// Where it is walking, if anywhere.
    pub target: Option<Cell>,
    step: Option<(Direction, Cell, u8)>,
}

impl ScriptedActor {
    /// A stationary actor.
    #[must_use]
    pub const fn new(actor: ActorRef, cell: Cell, facing: Direction) -> ScriptedActor {
        ScriptedActor {
            actor,
            cell,
            facing,
            target: None,
            step: None,
        }
    }

    /// Whether it is mid-step.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// Whether it still has walking to do.
    #[must_use]
    pub const fn is_walking(&self) -> bool {
        self.target.is_some() || self.step.is_some()
    }

    /// Sub-cell displacement in sixteenths, as [`crate::FieldState`] reports it.
    #[must_use]
    pub fn render_offset_16ths(&self, frames: StepFrames) -> (i32, i32) {
        let Some((dir, _, progress)) = self.step else {
            return (0, 0);
        };
        let travelled = i32::from(progress) * crate::field::SUBCELL_UNITS / i32::from(frames.get());
        let (dx, dy) = dir.delta();
        (dx * travelled, dy * travelled)
    }

    /// The direction that closes the gap to `target`: X first, then Y.
    fn direction_toward(&self, target: Cell) -> Option<Direction> {
        if target.x != self.cell.x {
            return Some(if target.x > self.cell.x {
                Direction::Right
            } else {
                Direction::Left
            });
        }
        if target.y != self.cell.y {
            return Some(if target.y > self.cell.y {
                Direction::Down
            } else {
                Direction::Up
            });
        }
        None
    }

    /// Advances one tick. Returns the cell arrived at when a walk finishes.
    ///
    /// Crate-internal: the runner drives this, callers read the actor instead.
    pub(crate) fn tick(&mut self, map: &FieldMap, frames: StepFrames) -> Option<Cell> {
        if self.step.is_none() {
            let target = self.target?;
            match self.direction_toward(target) {
                Some(dir) => {
                    self.facing = dir;
                    // Scene actors walk where the script says. Collision is not
                    // consulted: the cartridge's scripted moves write dest and
                    // drive the object there, and several scenes deliberately
                    // walk actors across cells the player could not.
                    //
                    // A target off the edge of a bounded map is the one thing
                    // that cannot be walked to. Abandoning the walk there is
                    // what keeps a `WaitForActor` from blocking forever; the
                    // arrival is still reported, at the cell actually reached.
                    let Some(to) = map.neighbor(self.cell, dir) else {
                        self.target = None;
                        return Some(self.cell);
                    };
                    self.step = Some((dir, to, 0));
                }
                None => {
                    self.target = None;
                    return Some(self.cell);
                }
            }
        }

        let (dir, to, progress) = self.step?;
        let progress = progress.saturating_add(1);
        if progress < frames.get() {
            self.step = Some((dir, to, progress));
            return None;
        }
        self.step = None;
        self.cell = to;
        if self.target == Some(self.cell) {
            self.target = None;
            return Some(self.cell);
        }
        None
    }
}
