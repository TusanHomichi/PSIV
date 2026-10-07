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

use crate::geom::{Cell, Direction};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::drift::{CoordCmp, NpcDrift};
use crate::scenes::{DestinationMask, FlightLeg};
use crate::state::{CharId, Flag, PARTY_SLOTS};

pub use crate::scene_types::{
    ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneEffect, SceneFault, SceneInput,
};

/// How many ops one tick may execute before the runner assumes the script is
/// looping and faults. Generous for real scenes, finite for broken ones.
pub const OP_BUDGET_PER_TICK: usize = 1024;

/// One instruction of a scene.
///
/// Jump targets are op indices into the same scene slice, so a scene is
/// self-contained data with no labels to resolve at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneOp {
    /// `loc_64C4A`: prepare the planet screen and its instant packed caption.
    FlightPlanet,
    /// `loc_5ABDC`: Y-only, twelve-bit wrapped flight pan at 2 px/frame.
    FlightPan,
    /// A flight Pal_FadeIn, including display-enable and terminal passes.
    FlightFadeIn {
        /// Display-enable, palette loop and terminal frame count.
        frames: u16,
    },
    /// Apply the normal transition record at the leader's current cell.
    /// Elevator events own its fade, destination door and departure step.
    TakeMapTransition,
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
    /// Restore selected chunks to the baked base layout, keeping all other
    /// live map edits and objects. Each tuple is (chunk x, chunk y, base id);
    /// the runtime verifies the literal id before removing an overlay.
    RestoreMapChunks {
        /// Collision-authoritative base chunks written by the scene.
        chunks: &'static [(u32, u32, u16)],
    },
    /// Write original map chunks relative to an actor's pixel position.
    /// The pack supplies both the chunk graphics and its collision values.
    WriteActorMapChunks {
        /// Object whose current position locates the writes.
        actor: ActorRef,
        /// (Pixel X offset, pixel Y offset, chunk id), divided into 32px
        /// chunk coordinates after adding the actor's current position.
        chunks: &'static [(i32, i32, u16)],
    },
    /// Write original map chunks at absolute chunk coordinates.
    ///
    /// The other live-layout primitive: `GetMapLayoutOffset` (`$53514`) or
    /// `GetMapLayoutChunkBG` (`$53EEC`) resolves an address from *literal*
    /// column and row operands, the routine writes chunk ids into it, and a
    /// `RefreshPlaneBG` follows. The overworld "appearing" events are this
    /// shape, as is `Cutscene_PsycoWand`'s Krup patch. Collision reads the
    /// layout the plane holds, so these writes reach the live map the moment
    /// they run. When the row a scene's table ends on is the map's *own* layout
    /// at those chunks, [`SceneOp::RestoreMapChunks`] is the exact op instead.
    /// See `docs/scenes/LIVE_LAYOUT_WRITES.md`.
    WriteMapChunks {
        /// (Chunk x, chunk y, replacement chunk id) in chunk coordinates.
        chunks: &'static [(u32, u32, u16)],
    },
    /// Replace the live map layout, both planes: `KosDecomp` of `fg` into
    /// `Map_Layout_FG` and of `bg` into `Map_Layout_BG`, the shape
    /// `Event_GaruberkTwEyeAction1` ends with (`ps4.asm:148974-148979`) and
    /// `MapDataMan_GaruberkTowerPart2` repeats at every load once its temp
    /// flag is set. Collision reads the layout, so the change reaches the live
    /// map at once; the runtime installs the pack's layout variant decoded
    /// from these sources and keeps every object where it stands.
    ReplaceMapLayout {
        /// ROM source of the foreground layout.
        fg: u32,
        /// ROM source of the background layout.
        bg: u32,
    },
    /// Clear VRAM and CRAM (`InitVRAMAndCRAM`, `$5A658`): fades out, resets a
    /// VDP register and rebuilds the Plane A buffer. Engine-visible effect is
    /// the fade; the rest is the renderer's.
    InitVramAndCram,
    /// Return from the scene with an explicit value (`d0`).
    ///
    /// Who reads the value depends on how the scene was dispatched
    /// (`FieldRoutine_Event`, `ps4.asm:120542-120548`):
    ///
    /// * A **cutscene** (event index `$8000` and up) goes through
    ///   `FieldRoutine_Cutscene`, which tests the flags its `jsr` left:
    ///   **zero** does `bset #2, Map_Load_Flags` and `move.w #8,
    ///   Game_Mode_Index` (`GameMode_LoadFieldMap`), a field reload before
    ///   control returns (`ps4.asm:120739-120758`); non-zero skips it. This is
    ///   how `Cutscene_PiataPrincipal` gets the office back, and it is why a
    ///   cutscene that mounts a vehicle and sets `Map_Load_Flags` bit 0
    ///   (`Cutscene_MeetingKyra`) hands the load flags to a real load that
    ///   consumes them. The runtime models it in
    ///   `rust/psiv-runtime/src/scene_return.rs`.
    /// * A plain **event** goes through `loc_5A27A` (`ps4.asm:120553-120568`),
    ///   which ignores `d0` altogether: it clears the input and
    ///   `Char_Move_Flags`, hands a pending event battle to `RunEventBattle`,
    ///   and calls `loc_5B368` (`ps4.asm:122258`, eight palette-buffer writes).
    ///   There is no map reload on any value, so
    ///   `Event_IgglanovaBattle`'s `moveq #1, d0` (`ps4.asm:152028`) is a
    ///   convention of the scene, not a mechanism: its battle hand-off is the
    ///   `bset #3, Routine_Exit_Flags` before it (`ps4.asm:152027`).
    Return {
        /// The `d0` value.
        value: u16,
    },
    /// Branch on the last text stream ending at FF rather than yielding at F7.
    BranchDialogueEnd {
        /// Op index for the completed-conversation return path.
        if_ended: usize,
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
    /// Branch on an actor's live pixel coordinate against a literal: the
    /// `cmpi.w #value, curr_x_pos(a4)` / `Bcc` pair events use to ask where an
    /// object stands (`Event_TylerGraveOpening`: `cmpi.w #$120, $30(a4)` /
    /// `bne.w` at `$06FC98`). The coordinate is the word the cartridge reads,
    /// whole pixels (`$30` for X, `$34` for Y), compared as unsigned words.
    BranchIfActorCoord {
        /// Whose coordinate.
        actor: ActorRef,
        /// Which coordinate.
        axis: Axis,
        /// The branch condition, in the `Bcc` that follows the compare.
        cmp: CoordCmp,
        /// The literal.
        value: u16,
        /// Op index taken when the condition holds.
        if_true: usize,
        /// Op index taken otherwise.
        if_false: usize,
    },
    /// Branch on the byte under `Saved_Dialogue_Addr` (`$FFFFECF0`) after the
    /// last dialogue: `popdlg` (`moveq #-1, d0 / move.w $ECF0.w, d0 / movea.l
    /// d0, a0`) then `cmpi.b #value, (a0)` (`Event_Gyuna`, `$070CA2`).
    ///
    /// The text engine stores `a0` with `pushdlg` in `TextCtrlCode_Terminate2`
    /// (`ps4.asm:142643`), which `$F7` (`TextCtrlCode_Terminate3`) and the
    /// `$FE`/`$FF` terminators all reach, and `a0` has already stepped past
    /// the terminator it just read. So the byte is the first of whatever the
    /// text would run next: the continuation after an `$F7`, or the first byte
    /// of the next entry after a terminator. The runtime reports it when the
    /// dialogue closes; with no dialogue run yet nothing is equal.
    BranchIfSavedDialogueByte {
        /// The literal.
        value: u8,
        /// Op index taken when the byte equals it.
        if_equal: usize,
        /// Op index taken otherwise.
        if_not: usize,
    },
    /// `DoMainUpdatesLoop` (`$5A73C`) over map objects that carry step
    /// constants: each frame `FieldObj_UpdatePosition` (`$04501C`) adds the
    /// object's `x_step_constant` (`$20`) and `y_step_constant` (`$24`), signed
    /// 16.16 pixels, to `curr_x_pos`/`curr_y_pos` (`$30`/`$34`). Blocks for
    /// `frames` ticks (already the `dbra` count), and each tick reports every
    /// object's whole-pixel position so the runtime moves it on the field map at
    /// the cartridge's rate and occupancy follows frame by frame.
    DriftNpcs {
        /// The objects and their step constants.
        drifts: &'static [NpcDrift],
        /// Loop iterations.
        frames: u16,
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
    /// Write a field object's `dialogue_id` (`$14(a4)`): the line it speaks
    /// the next time it is talked to. The guards at the Esper Mansion door are
    /// the first user: `Event_EsperGuardPermission` writes `1` or `2` into
    /// both (`$14` and `$54` off `Field_Obj_Secondary`, `$FFFFC300`) so they
    /// answer the party differently from then on. A map reload rebuilds the
    /// object from its record and the write is gone, as in retail.
    SetNpcDialogue {
        /// The object, by its index in the map's object list.
        npc_index: usize,
        /// The dialogue entry it speaks.
        dialogue_id: u16,
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
    /// Snap the overlapped party onto the vehicle lattice. If either original
    /// sprite axis had bit `$10`, refresh objects and wait for the existing
    /// runtime camera glide before the following `SetVehicleIndex`.
    AlignVehicleBoarding {
        /// The body object selected before the later persistent selector.
        index: u16,
    },
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
    /// The boarding events write both sound words only if the saved byte
    /// differs from `id`. Runtime owns that persistent byte and makes this
    /// literal comparison; the three retail callers all pass `$8D`.
    PlayMusicIfSavedDifferent {
        /// The track written to `Sound_Index` and `Saved_Sound_Index`.
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
    /// Write the literal `Render_Sprites_In_Cutscenes` byte (`$ECFD`). Despite
    /// its name, nonzero suppresses field sprites during bit-15 cutscenes
    /// and selects the panel portrait layout; zero selects ordinary field
    /// rendering. The original text terminator clears it.
    SetRenderSpritesInCutscene {
        /// The literal nonzero/zero value, not a sprite visibility boolean.
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
    /// Create a field object at literal pixel coordinates. It remains alive
    /// until a despawn or full scene reset, independently of animation age.
    CreateFieldObject {
        /// Retail secondary object slot.
        slot: usize,
        /// Retail object id.
        object_id: u16,
        /// Loaded art tile.
        art_tile: u16,
        /// World pixel X.
        x: i32,
        /// World pixel Y.
        y: i32,
    },
    /// `Event_StepObject`: move one object by signed 16.16 pixel increments
    /// for the stated number of frames before advancing the scene.
    StepFieldObject {
        /// Retail secondary object slot.
        slot: usize,
        /// Signed 16.16 X increment per frame.
        step_x: i32,
        /// Signed 16.16 Y increment per frame.
        step_y: i32,
        /// Loop iterations, including the final DBRA iteration.
        frames: u16,
    },
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
    /// The ship's destination menu (`loc_63BC4`, `ps4.asm:133499-133726`):
    /// the runtime hands the frame to a session-owned list window and the
    /// scene blocks until the player picks a row or cancels.
    ///
    /// Confirm continues at the next op, with `World_Index` already written
    /// (`:133677`). Cancel skips the next `cancel_skip` ops (`:133692`), so a
    /// scene that embeds a copy of the flight keeps working at any offset.
    DestinationMenu {
        /// Where the row mask comes from.
        mask: DestinationMask,
        /// Ops skipped on Cancel.
        cancel_skip: u16,
    },
    /// Load the map a flight table names for the current map and the chosen
    /// world: `loc_64B02`, `loc_64B34` or `loc_64B5A` (`ps4.asm:134540-134610`).
    /// Blocks like [`SceneOp::LoadMap`]; a table with no row is a fault.
    LoadFlightMap {
        /// Which table.
        leg: FlightLeg,
    },
    /// Write `World_Index` (`move.b #n, (World_Index).w`, `ps4.asm:155847`,
    /// `:156396`, `:156940`, `:157065`, `:157791`).
    SetWorldIndex {
        /// The world.
        world: u8,
    },
    /// Skip the next `skip` ops unless the current map is one of `maps`: the
    /// takeoff's `cmpi.w #MapID_MotaSpaceport / #MapID_DezoSpaceport,
    /// (Field_Map_Index).w` pair (`ps4.asm:133667-133674`).
    SkipUnlessMap {
        /// The maps that do not skip.
        maps: &'static [u16],
        /// Ops skipped otherwise.
        skip: u16,
    },
    /// Skip the next `count` ops: a relative `bra` over an alternative.
    SkipOps {
        /// Ops skipped.
        count: u16,
    },
    /// Flip one flag in place: `TempEveFlags_Toggle` (`$576EA`, `bchg` on
    /// `Temp_Event_Flags`) or `EventFlags_Toggle` (`$576E0`). The platform and
    /// terminal events use it so a second use undoes the first.
    ToggleFlag {
        /// Which flag.
        flag: Flag,
    },
    /// Branch on whether `who` is in the party: `Event_GetCharacter`
    /// (`$5A6D6`) returns with N set when `FindCharacterSlot` finds no slot,
    /// and the caller's `bmi` takes the absent path
    /// (`Event_DominatorsDefeated`, `$073258..$073294`).
    BranchIfPartyMember {
        /// The character asked about.
        who: CharId,
        /// Op index taken when they hold a party slot.
        if_present: usize,
        /// Op index taken otherwise.
        if_absent: usize,
    },
    /// `loc_5A97C` (`$5A97C`): `Event_UpdateObjFacing` on every character
    /// object whose id word is non-zero, in slot order. Every party member
    /// turns; an empty slot is skipped.
    FaceParty {
        /// The facing written to each.
        facing: Direction,
    },
    /// Write one byte of a character record's `skills` array
    /// (`move.b #id, skills+n(a0)`, `Event_Burstroc` `$072512`): the skill is
    /// learned. Only the id byte is written; the use counts are untouched.
    SetCharacterSkill {
        /// The roster record (`Wren_Stats` is `$FFFFF880`).
        who: CharId,
        /// Index into the eight-byte `skills` array.
        slot: u8,
        /// The skill id written.
        skill: u8,
    },
    /// The moving platforms' ride loop (`loc_6C50E`, `$06C50E`, and its five
    /// siblings): every frame the table's step word, sign-extended and shifted
    /// into signed 16.16 pixels (`ext.l d0 / lsl.l #8, d0`), is added to the
    /// platform object's `curr_y_pos` and, in the same frame, to `curr_y_pos`
    /// and `dest_y_pos` of all five party objects, and to the camera step
    /// counters; the loop ends when the platform object reaches its target.
    /// The target is a whole number of frames away, so the op carries the
    /// count. Blocks for `frames` ticks, running map updates each one.
    ///
    /// The party moves rigidly: the distance (`step_y * frames`) must be a
    /// whole number of 16-pixel cells, or the scene faults. The platform's own
    /// motion is announced to the renderer as a [`SceneOp::StepFieldObject`]
    /// on `slot`.
    RidePlatform {
        /// The temporary platform object's slot (`Field_LoadObject`'s first
        /// free one).
        slot: usize,
        /// Signed 16.16 pixels per frame.
        step_y: i32,
        /// Loop iterations.
        frames: u16,
    },
    /// The conveyor belts' carry loop (`Event_ConveyorBeltDown`, `$06CE5C`,
    /// and its three siblings): from the leader's resting cell the party is
    /// walked one cell at a time in `direction` at `FieldObj_Step_Offset` 0
    /// (16 frames per cell) for as long as the live collision-plane chunk
    /// under the leader (`GetChunkAndCollision`: chunk at `curr_x_pos`,
    /// `curr_y_pos + $10`) lies in `first_chunk..=last_chunk`, tested on
    /// every pixel of the step. The first test comes after one pixel, so the
    /// leader always takes at least one step, and a step in flight when the
    /// test fails is completed. Blocks until the leader arrives; the chunk
    /// layout is read through [`crate::SceneRunner::tick_with`], and a runner
    /// ticked without one faults.
    ConveyorRide {
        /// The belt's direction.
        direction: Direction,
        /// Lowest chunk id the belt covers (`cmpi.b #$A8, d7`).
        first_chunk: u8,
        /// Highest chunk id the belt covers (`cmpi.b #$AB, d7`).
        last_chunk: u8,
    },
    /// Unconditional jump.
    Jump {
        /// Op index.
        to: usize,
    },
    /// Finish the scene.
    End,
}

pub use crate::scene_runner::actor::ScriptedActor;
