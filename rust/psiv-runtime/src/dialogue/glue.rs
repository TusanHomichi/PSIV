//! The runtime's dialogue API: what the shell calls, and what the runner
//! needs from the rest of the game.
//!
//! The shell owns no dialogue rule. It hands the runtime one [`Pad`] per
//! frame, draws the [`DialogueView`] it gets back, and presents the
//! [`DialogueSignal`]s — panels, sounds, palettes, log lines. Opening and
//! closing, the typewriter, the choices, the `$F2` writes and the scene
//! acknowledgements all happen in here.
//!
//! The order the two frame calls keep is the cartridge's node order: the
//! window's input came before the field's tick, and the window's own tick came
//! after it. `docs/RUNTIME_DESIGN.md` records why that split is load-bearing
//! (a `$F2` flag write must land after the field tick that precedes it, not
//! before).

use psiv_data::DialogueSet;

use crate::Runtime;
use crate::dialogue::{DialogueSignal, DialogueView};
use crate::pad::Pad;

/// What a talk did.
///
/// The two failure kinds are deliberately not one: an object the map never
/// bound says nothing at all (the shell logs the missing binding), while a
/// bound object whose entry shows nothing is ordinary retail data — most of a
/// tree is empty entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcDialogueOpen {
    /// A window is up.
    Opened,
    /// The map has no dialogue tree, or the object no dialogue id.
    NoBinding,
    /// The object is bound, but its entry showed nothing: an empty entry, or a
    /// preamble `$F6`.
    Nothing,
}

/// What a scene's dialogue open did.
///
/// Three cases, and the last one is why this is not a `bool`: an entry that
/// resolved to nothing is acknowledged (the scene runs on), while a
/// `SetDialogueTree` address the pack does not have leaves the scene's
/// dialogue barrier *pending*, because every entry index after that op is
/// relative to a tree the runtime cannot read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneDialogueOpen {
    /// A window is up.
    Opened,
    /// The entry resolved to nothing: an empty entry, or no pack at all. The
    /// caller acknowledges the scene with `dialogue_closed`.
    Empty,
    /// The scene selected a tree the pack does not have. Nothing opened and
    /// nothing is acknowledged.
    UnknownTree,
}

impl Runtime {
    /// The pack's dialogue, for the renderer's art paths and its own
    /// panel-text tree lookup.
    ///
    /// Every runtime has one: the constructors take it from the data they are
    /// built from, which carries it because [`GameData::load`] read it with the
    /// rest of the pack, and a runtime refuses to build from data without it.
    ///
    /// [`GameData::load`]: psiv_data::GameData::load
    #[must_use]
    pub fn dialogue_pack(&self) -> &DialogueSet {
        self.dialogue
            .pack()
            .expect("a runtime is constructed with its dialogue pack")
    }

    /// Whether a dialogue window is on screen. The shell gates its input on
    /// this and ticks the field with a neutral input while it is true.
    #[must_use]
    pub fn dialogue_open(&self) -> bool {
        self.dialogue.is_open()
    }

    /// What the renderer draws this frame, or `None` with no window up.
    #[must_use]
    pub fn dialogue_view(&self) -> Option<DialogueView> {
        self.dialogue.view()
    }

    /// A `$F6` the dialogue fired, once. The shell starts the scene with
    /// [`Runtime::start_event`], on the frame after the flow raised it.
    pub fn take_dialogue_event(&mut self) -> Option<u16> {
        self.dialogue.take_event()
    }

    /// The dialogue window's input half of a frame: the pad's presses, the
    /// player's answer, and the close.
    ///
    /// Called once per frame, window or not — the pad is latched either way,
    /// so a press reads as fresh exactly once. With a window up this also
    /// mirrors the cartridge's suspension of field-object updates, set at the
    /// same point the shell set it before, which is what keeps the shared RNG
    /// stream identical.
    pub fn dialogue_frame(&mut self, pad: Pad) -> Vec<DialogueSignal> {
        if self.dialogue.is_open() {
            self.field_suspended = true;
        }
        let notice_up = self.field_notice().is_some();
        self.dialogue.input(&mut self.game, pad, notice_up);
        let signals = self.dialogue.drain_signals();
        for signal in &signals {
            match signal {
                // The scene hears the answer here, before its tick: a scene
                // branch consumes it on this frame, as it did when the shell
                // forwarded the window's pending choice.
                DialogueSignal::ChoiceAnswered(yes) => self.dialogue_choice(*yes),
                // Retail's two closes: `$F7` saves the text cursor for the
                // scene, an entry terminator ends the message.
                DialogueSignal::Closed { suspended } => {
                    // `$F7` and `$FF` both return through `loc_69B00`, which
                    // clears the panel rendering byte.
                    self.scene_panel_sprites = false;
                    if *suspended {
                        self.dialogue_closed();
                    } else {
                        self.dialogue_ended();
                    }
                }
                DialogueSignal::Action(_) | DialogueSignal::Log(_) | DialogueSignal::Fault(_) => {}
            }
        }
        signals
    }

    /// The dialogue window's own half of a frame: the `$F2` actions released
    /// at their byte positions, the open animation, the typewriter and the
    /// flow's signals.
    ///
    /// The shell calls it after it has run the field and applied the frame's
    /// events — the point the window node's `physics_process` held before the
    /// window moved here. It is a no-op with no window up.
    pub fn dialogue_tick(&mut self) -> Vec<DialogueSignal> {
        self.dialogue.window(&mut self.game);
        let signals = self.dialogue.drain_signals();
        if signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
        {
            self.scene_panel_sprites = false;
        }
        signals
    }

    /// Whether the running scene is a high-bit panel cutscene whose panel byte
    /// is up: its dialogue takes the panel layout and the field sprites are
    /// hidden.
    #[must_use]
    pub fn panel_dialogue_mode(&self) -> bool {
        self.scene_panel_sprites
            && self
                .scene_event()
                .is_some_and(|event| event.0 & 0x8000 != 0)
    }

    /// Opens the dialogue an object owns at `npc_index`: the current map's
    /// dialogue tree and the object's dialogue id, then turning the object to
    /// face the party — the cartridge's default for a talk, which `$F3`
    /// suppresses.
    ///
    /// The map-effect dialogue override is what this uses, not the map
    /// record's own binding: clinics and story rooms change what a person
    /// says.
    pub fn open_npc_dialogue(&mut self, npc_index: usize) -> NpcDialogueOpen {
        let Some(tree) = self
            .map_record()
            .map(|record| record.dialogue_tree)
            .filter(|tree| *tree != 0)
        else {
            return NpcDialogueOpen::NoBinding;
        };
        let Some(dialogue_id) = self.npc_dialogue_id(npc_index) else {
            return NpcDialogueOpen::NoBinding;
        };
        if !self.dialogue.open_entry(tree, dialogue_id, &self.game) {
            return NpcDialogueOpen::Nothing;
        }
        let toward = self.party.leader().facing().opposite();
        self.face_npc(npc_index, toward);
        NpcDialogueOpen::Opened
    }

    /// Opens a scene-owned line: the entry is resolved against the tree the
    /// scene is reading (its `SetDialogueTree` op, or the map's own binding).
    /// `panel_layout` selects the panel cutscene's portrait placement.
    pub fn open_scene_dialogue(&mut self, entry: u16, panel_layout: bool) -> SceneDialogueOpen {
        let Some(tree) = self.scene_dialogue_tree() else {
            return SceneDialogueOpen::UnknownTree;
        };
        if self
            .dialogue
            .open_scene_entry(tree, entry, panel_layout, &self.game)
        {
            SceneDialogueOpen::Opened
        } else {
            SceneDialogueOpen::Empty
        }
    }

    /// The dialogue tree a running scene's entries index into: the map's own
    /// binding (`MapRecord::dialogue_tree`) until the scene's
    /// `SetDialogueTree` (`$53F00`) points it at another tree, whose number
    /// the pack's tree table supplies from the recorded ROM address.
    ///
    /// `None` when the scene recorded an address no packed tree claims — a
    /// data defect, and one the caller must not paper over: the entry indices
    /// after that op are relative to a tree that is not there.
    #[must_use]
    pub fn scene_dialogue_tree(&self) -> Option<u8> {
        match self.scene_tree_address {
            None => self.map_record().map(|record| record.dialogue_tree),
            Some(address) => self
                .dialogue_pack()
                .trees
                .trees
                .iter()
                .find(|tree| tree.rom_address() == Some(address))
                .map(|tree| tree.tree),
        }
    }

    /// Reopens the scene's saved text cursor (`Saved_Dialogue_Addr`) after its
    /// movement and presentation ops.
    pub fn resume_scene_dialogue(&mut self, panel_layout: bool) -> bool {
        self.dialogue.resume_scene(panel_layout, &self.game)
    }

    /// The leader's "Nothing here" line for `character_slot` (0 = Chaz): the
    /// cartridge's answer to an empty-handed confirm.
    pub fn open_nothing_here(&mut self, character_slot: usize) -> bool {
        self.dialogue.open_nothing_here(character_slot, &self.game)
    }

    /// A scene's yes/no branch, with no text choice in front of it.
    pub fn open_scene_choice(&mut self) -> bool {
        self.dialogue.open_standalone_choice(&self.game)
    }

    /// Displays a static field-status window (a fallen or perished notice)
    /// over the normal frame and font, without the dialogue typewriter.
    pub fn open_status_dialogue(&mut self, lines: &[String]) -> bool {
        self.dialogue.open_status(lines, &self.game)
    }

    /// Closes the window without answering anything or telling the scene: the
    /// debug harness's instant-close path for scene dialogue.
    pub fn close_dialogue(&mut self) {
        self.dialogue.close_window();
    }
}

#[cfg(test)]
mod tests {
    use crate::dialogue::DialogueRunner;
    use crate::{Runtime, SceneDialogueOpen};
    use psiv_core::{GameState, StepFrames};
    use psiv_data::GameData;
    use std::path::{Path, PathBuf};

    fn pack_dir() -> PathBuf {
        match std::env::var_os("PSIV_RUNTIME_PACK") {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("runtime-pack"),
        }
    }

    fn runtime() -> Option<Runtime> {
        let dir = pack_dir();
        if !dir.join("dialogue").join("trees.json").is_file() {
            return None;
        }
        let data = GameData::load(Path::new(&dir)).ok()?;
        let rt = Runtime::new(
            data,
            0x010,
            psiv_core::Cell::new(31, 8),
            psiv_core::Direction::Down,
            StepFrames::default(),
        )
        .ok()?;
        Some(rt)
    }

    #[test]
    fn a_scene_tree_address_resolves_through_the_packs_own_table() {
        let Some(mut rt) = runtime() else {
            return;
        };
        // No scene op yet: the map's own binding, exactly as the cartridge
        // starts a scene.
        assert_eq!(
            rt.scene_dialogue_tree(),
            rt.map_record().map(|record| record.dialogue_tree)
        );
        // Alshline's explicit switch away from Zema's map tree: tree 3.
        rt.scene_tree_address = Some(0x1E0_BA0);
        assert_eq!(rt.scene_dialogue_tree(), Some(3));
        // An address no packed tree claims must not fall back to town text.
        rt.scene_tree_address = Some(0x12_3456);
        assert_eq!(rt.scene_dialogue_tree(), None);
    }

    /// The negative control for the acceptance case in
    /// `tests/session_opening.rs`: the pack is the whole difference between a
    /// message box that opens and one that cannot. A runner with no pack — the
    /// state every runtime used to start in, because the title's START and
    /// CONTINUE never called the separate loader — cannot resolve the opening's
    /// first line (academy tree 17, entry `$36`, "Chaz, we have work to do!"),
    /// so the scene waits on a barrier that never clears.
    ///
    /// The loader is gone and the constructors install the pack, so the
    /// control drives the runner's own pack-less default with the same call the
    /// runtime makes for a scene dialogue, then repeats it with the pack in
    /// place.
    #[test]
    fn a_runner_without_the_pack_cannot_open_the_openings_first_line() {
        let mut bare = DialogueRunner::new();
        let game = GameState::new();
        assert!(
            !bare.open_scene_entry(17, 0x36, false, &game),
            "no pack, no window"
        );
        assert!(!bare.is_open());
        assert!(bare.view().is_none());

        let Some(mut rt) = runtime() else {
            return;
        };
        rt.scene_tree_address = Some(0x1EB_A90);
        assert_eq!(rt.scene_dialogue_tree(), Some(17));
        assert_eq!(
            rt.open_scene_dialogue(0x36, false),
            SceneDialogueOpen::Opened
        );
        assert!(rt.dialogue_open(), "the pack opens the same line");
    }

    #[test]
    fn every_scene_dialogue_tree_address_resolves_from_the_pack() {
        let Some(mut rt) = runtime() else {
            return;
        };
        // The pack's own table, read here a second way: a scene's recorded
        // address must land on the tree that table names, not merely on some
        // tree.
        let trees: Vec<(u8, Option<u32>)> = rt
            .dialogue_pack()
            .trees
            .trees
            .iter()
            .map(|tree| (tree.tree, tree.rom_address()))
            .collect();
        for scene in psiv_core::SCENES {
            for op in scene.ops {
                if let psiv_core::SceneOp::SetDialogueTree { rom_addr } = op {
                    let expected = trees
                        .iter()
                        .find(|(_, address)| *address == Some(*rom_addr))
                        .map(|(tree, _)| *tree);
                    assert!(
                        expected.is_some(),
                        "{}: {rom_addr:#x} is no packed tree",
                        scene.name
                    );
                    rt.scene_tree_address = Some(*rom_addr);
                    assert_eq!(
                        rt.scene_dialogue_tree(),
                        expected,
                        "{}: {rom_addr:#x}",
                        scene.name
                    );
                }
            }
        }
    }
}
