//! Opening a line: the pack's trees, the preamble `$FA` chains, the system
//! messages and the scene's resumes.
//!
//! Everything here resolves an entry into a window and hands it to the
//! runner's `start`; the frame halves in the parent module drive what then
//! happens. The one thing this module is careful about is *which* bank
//! the `$FA` chains read: the pack's entries are walked against the live
//! [`GameState`], so a preamble that routes on a story flag takes today's
//! branch and not a copy taken at boot.

use psiv_core::GameState;
use psiv_data::{Ctrl, DialogueEntry, Segment};

use super::{DialogueRunner, Opening, TextFlow};

impl DialogueRunner {
    /// Opens an NPC's line: the map's dialogue tree (1-based) and the object's
    /// `dialogue_id`.
    pub(crate) fn open_entry(&mut self, tree: u8, dialogue_id: u16, game: &GameState) -> bool {
        self.open_resolved_tree(tree, dialogue_id, game, false, false)
    }

    /// Opens a scene-owned line, preserving `$F7` pauses regardless of whether
    /// the original flags select the field or panel portrait position.
    pub(crate) fn open_scene_entry(
        &mut self,
        tree: u8,
        dialogue_id: u16,
        panel_layout: bool,
        game: &GameState,
    ) -> bool {
        self.open_resolved_tree(tree, dialogue_id, game, true, panel_layout)
    }

    fn open_resolved_tree(
        &mut self,
        tree: u8,
        dialogue_id: u16,
        game: &GameState,
        scene_dialogue: bool,
        panel_layout: bool,
    ) -> bool {
        self.suspended = None;
        self.choice.reset();
        self.tree = tree;
        self.scene_dialogue = scene_dialogue;
        self.cutscene_portrait = panel_layout;
        match self.resolve(tree, dialogue_id, game) {
            Ok((id, opening)) => self.start(opening, &format!("tree {tree} entry {id}")),
            Err(fault) => {
                self.fault(fault);
                false
            }
        }
    }

    /// Follows `$FA` preamble jumps against the live flags, bounded so a
    /// cyclic chain (a data bug) cannot hang. Reports the entry the window
    /// actually opened.
    fn resolve(
        &self,
        tree: u8,
        dialogue_id: u16,
        game: &GameState,
    ) -> Result<(u16, Opening), String> {
        let Some(set) = self.set.as_ref() else {
            return Err("no dialogue pack loaded".to_owned());
        };
        let mut id = dialogue_id;
        for _ in 0..16 {
            let Some(entry) = set.entry(tree, id) else {
                return Err(format!("tree {tree} has no entry {id}"));
            };
            match TextFlow::open_at(entry, game) {
                Opening::Jump(next) => id = next,
                opening => return Ok((id, opening)),
            }
        }
        Err(format!(
            "tree {tree} entry {dialogue_id}: preamble jump chain too deep"
        ))
    }

    /// The scene's next `RunDialogueResume` reopens at `Saved_Dialogue_Addr`.
    pub(crate) fn resume_scene(&mut self, panel_layout: bool, game: &GameState) -> bool {
        self.cutscene_portrait = panel_layout;
        let Some((tree, flow)) = self.suspended.take() else {
            self.fault("scene resume has no saved cursor".to_owned());
            return false;
        };
        self.tree = tree;
        self.scene_dialogue = true;
        match flow.resume_scene(game) {
            Opening::Jump(entry) => self.open_resolved_tree(tree, entry, game, true, panel_layout),
            opening => self.start(opening, &format!("resumed tree {tree}")),
        }
    }

    /// The leader's "Nothing here" line (one per character slot).
    pub(crate) fn open_nothing_here(&mut self, character_slot: usize, game: &GameState) -> bool {
        let entry = self
            .set
            .as_ref()
            .and_then(|set| set.nothing_here(character_slot))
            .cloned();
        let Some(entry) = entry else {
            self.log("pack carries no system messages".to_owned());
            return false;
        };
        self.open_resolved(&entry, game)
    }

    /// A scene branch without a preceding text choice remains interactive.
    pub(crate) fn open_standalone_choice(&mut self, game: &GameState) -> bool {
        let entry = DialogueEntry {
            id: 0,
            text: String::new(),
            pages: Vec::new(),
            segments: vec![Segment::Control(Ctrl::YesNo {
                code: 0xF5,
                operands: vec![0, 0],
                yes_entry: 0,
                no_entry: 0,
            })],
        };
        let opened = self.open_resolved(&entry, game);
        self.scene_dialogue = true;
        self.choice.reset_standalone();
        opened
    }

    /// Displays a static field-status window using the normal frame and font.
    pub(crate) fn open_status(&mut self, lines: &[String], game: &GameState) -> bool {
        let mut segments = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if index != 0 {
                segments.push(Segment::Control(Ctrl::Newline {
                    code: 0xFC,
                    operands: Vec::new(),
                }));
            }
            segments.push(Segment::Text(line.clone()));
        }
        let entry = DialogueEntry {
            id: 0,
            text: lines.join("\n"),
            segments,
            pages: Vec::new(),
        };
        let opened = self.open_resolved(&entry, game);
        // These windows load tile strings at once, without the dialogue
        // typewriter.
        self.revealed = lines.iter().map(|line| line.chars().count()).sum();
        opened
    }

    /// Opens an entry the caller already resolved: the system messages, the
    /// field-status window, a scene's standalone prompt. `pub(super)` because
    /// the runner's own tests build their entries this way.
    pub(super) fn open_resolved(&mut self, entry: &DialogueEntry, game: &GameState) -> bool {
        self.suspended = None;
        self.scene_dialogue = false;
        let opening = TextFlow::open_at(entry, game);
        if let Opening::Jump(next) = opening {
            // System messages never jump; a jump here means a caller fed a
            // tree entry through the pre-resolved path.
            self.fault(format!("pre-resolved entry {} jumps to {next}", entry.id));
            return false;
        }
        self.start(opening, &format!("entry {}", entry.id))
    }

    /// Closes the window without answering anything: the debug harness's
    /// instant-close path. No signal is produced, so a scene that never saw a
    /// window behaves as it did when the harness skipped opening one.
    pub(crate) fn close_window(&mut self) {
        self.note_stop();
        self.flow = None;
        self.open_cells = 0;
        self.scene_dialogue = false;
    }
}
