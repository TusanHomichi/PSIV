//! The dialogue pack: trees, font, window chrome, portraits.
//!
//! `runtime-pack/dialogue/` is four JSONs plus their art. `trees.json` holds
//! all 43 Kosinski trees decoded into entries, each entry both as a segment
//! stream (text runs and control codes, exactly the byte stream regrouped) and
//! as the pages `RunText_CharacterLoop` would show; `font.json` maps a
//! character to its cell in the 8x16 glyph strip; `window.json` is the frame's
//! tile roles, geometry rule and open animation; `portraits.json` is the `$F4`
//! table. This module is the only thing in the workspace that knows that shape.
//!
//! Fail-closed like the rest of the crate: an unknown control name, a `$F4`
//! pointing at a portrait the table does not have, a character with no glyph, a
//! missing window role or a page wider than the window all stop the load. The
//! extractor proves "zero unknown control bytes and zero unmapped glyphs across
//! all trees" against the ROM (SOURCE_NOTES) -- this side re-proves it against
//! the pack, so a hand-edited or half-written pack cannot reach the renderer.
//!
//! ```no_run
//! use psiv_data::DialogueSet;
//! use std::path::Path;
//!
//! let dialogue = DialogueSet::load(Path::new("runtime-pack"))?;
//! // Map $010 (Piata) binds dialogue tree 1; NPC dialogue ids index into it.
//! let entry = dialogue.entry(1, 0).expect("tree 1 entry 0 exists");
//! assert_eq!(entry.pages[0].lines[0], "Are you a hunter?");
//! # Ok::<(), psiv_data::DataError>(())
//! ```

use std::fmt;
use std::path::Path;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};

use crate::error::DataError;

mod chrome;
mod trees;

pub use chrome::{
    CellRegion, FontFile, Glyph, GlyphMetrics, OpenAnimation, PixelRect, Portrait, PortraitFile,
    PortraitGeometry, PortraitWindow, Role, RoleTile, TextWindow, WindowFile, WindowGeometry,
    WindowPalette, WindowRecord,
};
pub use trees::{
    ActionKind, Ctrl, DialogueEntry, DialogueTree, FlagScope, PORTRAIT_HIDE, Page, PageEnd,
    ScrollArrow, Segment, TreeFile, TreeWindow,
};

/// Format version every file of the dialogue pack declares.
pub const DIALOGUE_FORMAT_VERSION: u32 = 1;

/// Characters on one line of the message window.
///
/// `addq.w #1,d1 / andi.w #$1F,d1` at `0x06A0CE`: the column counter wraps at
/// 32, which is what makes a full line break by itself.
pub const CHARS_PER_LINE: usize = 32;

/// Lines the message window holds.
///
/// `andi.w #1,d2` at `0x06A0F0`: the line counter is one bit wide, so a full
/// second line waits for input instead of writing a third.
pub const LINES_PER_WINDOW: usize = 2;

/// The subdirectory of the pack this module reads.
const DIALOGUE_DIR: &str = "dialogue";

// ---------------------------------------------------------------------------
// The set
// ---------------------------------------------------------------------------

/// The whole dialogue pack, loaded and cross-validated.
#[derive(Debug, Clone)]
pub struct DialogueSet {
    /// `dialogue/trees.json`: the 43 trees and their entries.
    pub trees: TreeFile,
    /// `dialogue/font.json`: the 8x16 glyph strip and its character map.
    pub font: FontFile,
    /// `dialogue/window.json`: frame tiles, geometry, open animation.
    pub window: WindowFile,
    /// `dialogue/portraits.json`: the `$F4` portrait table.
    pub portraits: PortraitFile,
}

impl DialogueSet {
    /// Loads and validates `<pack_dir>/dialogue/`.
    ///
    /// # Errors
    ///
    /// Any missing file, malformed JSON, unknown control name, unresolved
    /// portrait id, uncovered character, incomplete window role set or
    /// internally inconsistent count stops the load.
    pub fn load(pack_dir: &Path) -> Result<DialogueSet, DataError> {
        let dir = pack_dir.join(DIALOGUE_DIR);
        let set = DialogueSet {
            trees: read_json(&dir.join("trees.json"))?,
            font: read_json(&dir.join("font.json"))?,
            window: read_json(&dir.join("window.json"))?,
            portraits: read_json(&dir.join("portraits.json"))?,
        };
        set.validate(pack_dir)?;
        Ok(set)
    }

    /// One tree by its 1-based number, the way a map record binds it.
    #[must_use]
    pub fn tree(&self, tree: u8) -> Option<&DialogueTree> {
        self.trees.trees.iter().find(|t| t.tree == tree)
    }

    /// One entry: the map's `dialogue_tree` (1-based) and an NPC's
    /// `dialogue_id`.
    ///
    /// `GetDialogueByID` counts `$FF` bytes from the tree's start, so ids are
    /// dense and include the empty entries -- an id that lands on one resolves
    /// to an entry with no segments, which is the cartridge's own "this object
    /// says nothing" and not an error.
    #[must_use]
    pub fn entry(&self, tree: u8, dialogue_id: u16) -> Option<&DialogueEntry> {
        self.tree(tree)?.entry(dialogue_id)
    }

    /// A portrait by `$F4` id. Id 0 is the null entry: it hides the window.
    #[must_use]
    pub fn portrait(&self, id: u8) -> Option<&Portrait> {
        self.portraits.portraits.iter().find(|p| p.id == id)
    }

    /// Where `ch` sits in the glyph strip.
    #[must_use]
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let byte = *self.font.by_char.get(&ch)?;
        self.font.glyphs.iter().find(|g| g.byte == byte)
    }

    /// Every rule that spans two files or two records. Called by
    /// [`DialogueSet::load`]; separate so the tests can drive it on fixtures.
    fn validate(&self, pack_dir: &Path) -> Result<(), DataError> {
        let trees_path = pack_dir.join(DIALOGUE_DIR).join("trees.json");
        let window_path = pack_dir.join(DIALOGUE_DIR).join("window.json");
        let portraits_path = pack_dir.join(DIALOGUE_DIR).join("portraits.json");

        for found in [
            self.trees.format_version,
            self.font.format_version,
            self.window.format_version,
            self.portraits.format_version,
        ] {
            if found != DIALOGUE_FORMAT_VERSION {
                return Err(DataError::FormatVersion {
                    found,
                    expected: DIALOGUE_FORMAT_VERSION,
                });
            }
        }

        self.validate_window(&window_path)?;
        self.validate_trees(&trees_path)?;
        self.validate_portrait_files(pack_dir, &portraits_path)?;
        Ok(())
    }

    /// The geometry rule needs all nine roles, and the two windows the
    /// renderer places must agree with the cell grid they are derived from.
    fn validate_window(&self, path: &Path) -> Result<(), DataError> {
        for role in Role::ALL {
            if !self.window.roles.contains_key(role.as_str()) {
                return Err(defect(
                    path,
                    format!(
                        "roles is missing {role}; the geometry rule draws all nine \
                         ({}) and nothing else",
                        Role::ALL.map(Role::as_str).join(", ")
                    ),
                ));
            }
        }

        let cell = i32::try_from(self.window.geometry.cell_pixels).unwrap_or(i32::MAX);
        let border = i32::try_from(self.window.geometry.border_cells).unwrap_or(i32::MAX);
        if cell == 0 {
            return Err(defect(path, "geometry cell_pixels is 0"));
        }

        let text = &self.window.text_window;
        let interior_w = i32::try_from(text.chars_per_line * text.glyph_width)
            .expect("32 glyphs of 8 pixels fits in i32");
        let interior_h = i32::try_from(text.lines_per_window * text.glyph_height)
            .expect("2 lines of 16 pixels fits in i32");
        let expected_w = interior_w + 2 * border * cell;
        let expected_h = interior_h + 2 * border * cell;
        if text.rect.width != expected_w || text.rect.height != expected_h {
            return Err(defect(
                path,
                format!(
                    "text_window rect is {}x{} but {} glyphs of {}px on {} lines of {}px \
                     inside a {border}-cell border is {expected_w}x{expected_h}",
                    text.rect.width,
                    text.rect.height,
                    text.chars_per_line,
                    text.glyph_width,
                    text.lines_per_window,
                    text.glyph_height
                ),
            ));
        }
        if text.chars_per_line as usize != CHARS_PER_LINE
            || text.lines_per_window as usize != LINES_PER_WINDOW
        {
            return Err(defect(
                path,
                format!(
                    "text_window is {}x{} characters; the routine's masks fix it at \
                     {CHARS_PER_LINE}x{LINES_PER_WINDOW}",
                    text.chars_per_line, text.lines_per_window
                ),
            ));
        }
        if self.window.geometry.open_animation.step_cells == 0 {
            return Err(defect(
                path,
                "open_animation step_cells is 0: it never opens",
            ));
        }
        Ok(())
    }

    /// Everything the trees promise about themselves and about the other three
    /// files: counts, dense ids, in-range jumps, portraits that resolve,
    /// characters that have glyphs, pages that fit the window.
    fn validate_trees(&self, path: &Path) -> Result<(), DataError> {
        let trees = &self.trees.trees;
        if trees.len() != self.trees.tree_count as usize {
            return Err(defect(
                path,
                format!(
                    "tree_count is {} but the file holds {} trees",
                    self.trees.tree_count,
                    trees.len()
                ),
            ));
        }
        let total: usize = trees.iter().map(|t| t.entries.len()).sum();
        if total != self.trees.entry_count as usize {
            return Err(defect(
                path,
                format!(
                    "entry_count is {} but the trees hold {total} entries",
                    self.trees.entry_count
                ),
            ));
        }

        for (index, tree) in trees.iter().enumerate() {
            let number = u8::try_from(index + 1).unwrap_or(u8::MAX);
            if tree.tree != number {
                return Err(defect(
                    path,
                    format!(
                        "trees are stored in tree order 1..={}, so trees[{index}] must be tree \
                         {number}, not tree {}",
                        trees.len(),
                        tree.tree
                    ),
                ));
            }
            if tree.entries.len() != tree.entry_count as usize {
                return Err(defect(
                    path,
                    format!(
                        "{}: entry_count is {} but it holds {} entries",
                        tree.label,
                        tree.entry_count,
                        tree.entries.len()
                    ),
                ));
            }
            for (position, entry) in tree.entries.iter().enumerate() {
                let expected = u16::try_from(position).unwrap_or(u16::MAX);
                if entry.id != expected {
                    return Err(defect(
                        path,
                        format!(
                            "{}: entry ids count $FF bytes from the tree's start, so they are \
                             dense: entries[{position}] must be id {expected}, not {}",
                            tree.label, entry.id
                        ),
                    ));
                }
                self.validate_entry(path, tree, entry)?;
            }
        }
        Ok(())
    }

    fn validate_entry(
        &self,
        path: &Path,
        tree: &DialogueTree,
        entry: &DialogueEntry,
    ) -> Result<(), DataError> {
        let entries = tree.entries.len() as u32;
        let where_ = |what: &str| format!("{} entry {}: {what}", tree.label, entry.id);

        for (index, segment) in entry.segments.iter().enumerate() {
            match segment {
                Segment::Text(text) => {
                    for ch in text.chars() {
                        if !self.font.by_char.contains_key(&ch) {
                            return Err(defect(
                                path,
                                where_(&format!(
                                    "segments[{index}] uses {ch:?} (U+{:04X}), which the dialogue \
                                     font has no glyph for",
                                    u32::from(ch)
                                )),
                            ));
                        }
                    }
                }
                Segment::Control(ctrl) => {
                    if ctrl.code() != ctrl.expected_code() {
                        return Err(defect(
                            path,
                            where_(&format!(
                                "segments[{index}] is {} but carries code {:#04X}; \
                                 TextCtrlCodesJmpTbl reaches it from {:#04X}",
                                ctrl.name(),
                                ctrl.code(),
                                ctrl.expected_code()
                            )),
                        ));
                    }
                    match ctrl {
                        Ctrl::Portrait { id, .. } => {
                            if *id != PORTRAIT_HIDE && self.portrait(*id).is_none() {
                                return Err(defect(
                                    path,
                                    where_(&format!(
                                        "segments[{index}] shows portrait {id}, which \
                                         DialoguePortraitArtPtrs does not have"
                                    )),
                                ));
                            }
                        }
                        Ctrl::FlagCheck { then_entry, .. } => {
                            check_entry_ref(
                                path,
                                &where_,
                                *then_entry,
                                entries,
                                index,
                                "then_entry",
                            )?;
                        }
                        Ctrl::YesNo {
                            yes_entry,
                            no_entry,
                            ..
                        } => {
                            check_entry_ref(
                                path,
                                &where_,
                                *yes_entry,
                                entries,
                                index,
                                "yes_entry",
                            )?;
                            check_entry_ref(path, &where_, *no_entry, entries, index, "no_entry")?;
                        }
                        _ => {}
                    }
                }
            }
        }

        for (index, page) in entry.pages.iter().enumerate() {
            if page.lines.len() > LINES_PER_WINDOW {
                return Err(defect(
                    path,
                    where_(&format!(
                        "pages[{index}] has {} lines; the window holds {LINES_PER_WINDOW}",
                        page.lines.len()
                    )),
                ));
            }
            for (line, text) in page.lines.iter().enumerate() {
                if text.chars().count() > CHARS_PER_LINE {
                    return Err(defect(
                        path,
                        where_(&format!(
                            "pages[{index}] line {line} is {} characters; the window holds \
                             {CHARS_PER_LINE}",
                            text.chars().count()
                        )),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every portrait the table names has its PNG on disk. The renderer loads
    /// them by the path the pack declares, so a missing file is a load-time
    /// defect, not a blank window three hours into a playthrough.
    fn validate_portrait_files(&self, pack_dir: &Path, path: &Path) -> Result<(), DataError> {
        if self.portraits.portraits.len() != self.portraits.count as usize {
            return Err(defect(
                path,
                format!(
                    "count is {} but the file lists {} portraits",
                    self.portraits.count,
                    self.portraits.portraits.len()
                ),
            ));
        }
        for portrait in &self.portraits.portraits {
            if portrait.id == PORTRAIT_HIDE {
                return Err(defect(
                    path,
                    format!(
                        "{} is portrait 0, which is the table's null entry -- $F4 id 0 hides \
                         the window",
                        portrait.symbol
                    ),
                ));
            }
            let png = pack_dir.join(&portrait.png);
            if !png.is_file() {
                return Err(defect(
                    path,
                    format!(
                        "portrait {} ({}) declares {} which is not in the pack",
                        portrait.id, portrait.symbol, portrait.png
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn check_entry_ref(
    path: &Path,
    where_: &dyn Fn(&str) -> String,
    target: u16,
    entries: u32,
    index: usize,
    field: &str,
) -> Result<(), DataError> {
    if u32::from(target) >= entries {
        return Err(defect(
            path,
            where_(&format!(
                "segments[{index}] {field} is {target} but the tree has {entries} entries"
            )),
        ));
    }
    Ok(())
}
// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::io(path, e))?;
    serde_json::from_str(&text).map_err(|e| DataError::json(path, e))
}

/// A dialogue defect, reported against the file that holds it.
///
/// `DataError::Validation` needs a `MapId` and nothing here has one, so these
/// ride on the `Json` variant: the message reads
/// `could not parse <file>: <tree> entry <id>: <what>`. A `DataError::Dialogue`
/// variant would say it plainer -- error.rs belongs to another lane, so that is
/// a proposal in this lane's report, and this function is the only place it
/// would change.
fn defect(path: &Path, message: impl fmt::Display) -> DataError {
    DataError::Dialogue {
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}

/// `"0xFA"` -> `0xFA`. The pack writes code bytes as hex strings so a reader
/// sees the same notation the disassembly uses.
fn hex_byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let raw = String::deserialize(deserializer)?;
    let digits = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .ok_or_else(|| D::Error::custom(format!("control code {raw:?} is not 0x-prefixed")))?;
    u8::from_str_radix(digits, 16)
        .map_err(|e| D::Error::custom(format!("control code {raw:?} is not a byte: {e}")))
}
