//! The palette line the native windows draw with, as data.
//!
//! Every native window draws art baked in CRAM line 2's `Pal_Init_Line_3`
//! colours (`$296300`, a copy of `Pal_Init`'s line 2): the window strip and
//! the dialogue and menu fonts (`dialogue/window.json`'s palette), the scroll
//! arrow, the dialogue and shop portraits and the STATUS portraits (each
//! pack record's `cram_line` is 2). A map load puts those words on line 2,
//! and a scene's tone, flash or fade changes them there like any other line.
//!
//! [`WindowLine`] binds a window to that line: the sixteen words its art is
//! baked in and the sixteen it draws with this frame. The window shader
//! (`cram_art::window_line_material`) finds each pixel's index among the
//! baked colours and draws the live word at that index. [`WindowFamily`]
//! names each window drawn that way and the node whose material carries the
//! line. The shell (`Field::apply_window_line`) attaches one material to
//! every [`WindowNode`] and sets its line from the shared CRAM.
//!
//! This module touches no Godot type, so the binding is tested without the
//! engine (`rust/psiv-godot/tests/window_palette.rs`).

/// The CRAM line the window art is baked on.
pub(crate) const WINDOW_LINE: usize = 2;

/// `Pal_Init_Line_3` (`$296300`): the line-2 words the window art is baked
/// in (`dialogue/window.json`'s palette).
pub(crate) const WINDOW_LINE_ROM: u32 = 0x0029_6300;

/// A native window whose art is baked in line 2's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowFamily {
    /// The message box: frame, glyphs, portrait and scroll arrow.
    Dialogue,
    /// The YES/NO prompt over the message box (`dialogue/choice.rs`).
    Choice,
    /// The field camp and chest windows, their text and STATUS portraits.
    Camp,
    /// The shop and inn windows, their text and the innkeeper's portrait.
    Shop,
}

/// The node that draws a [`WindowFamily`], and so carries its material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowNode {
    /// `DialogueWindow`.
    Dialogue,
    /// `CampMenu`.
    Camp,
    /// `ShopWindow`.
    Shop,
}

impl WindowFamily {
    /// Every family drawn on line 2.
    pub(crate) const ALL: [Self; 4] = [Self::Dialogue, Self::Choice, Self::Camp, Self::Shop];

    /// The node that draws this family. The choice's quads are part of the
    /// dialogue window's draw list, so the message box's material draws them.
    pub(crate) const fn node(self) -> WindowNode {
        match self {
            Self::Dialogue | Self::Choice => WindowNode::Dialogue,
            Self::Camp => WindowNode::Camp,
            Self::Shop => WindowNode::Shop,
        }
    }
}

/// The words one window draws line 2 with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowLine {
    /// The words the art is baked in.
    pub(crate) baked: [u16; 16],
    /// The words it draws with.
    pub(crate) live: [u16; 16],
}

impl WindowLine {
    /// Line [`WINDOW_LINE`] of `words` (`Palette_Table_Buffer`) when the
    /// palette is `live`; the baked words, unchanged, when it is not. `None`
    /// when fewer than sixteen baked words are known.
    pub(crate) fn new(words: &[u16; 64], live: bool, baked: &[u16]) -> Option<Self> {
        let baked: [u16; 16] = baked.get(..16)?.try_into().ok()?;
        let start = WINDOW_LINE * 16;
        let live = if live {
            words[start..start + 16].try_into().ok()?
        } else {
            baked
        };
        Some(Self { baked, live })
    }

    /// The same baked words drawn with `live` instead: the debug override's
    /// controlled change (`Field::debug_window_line`).
    pub(crate) const fn with_live(self, live: [u16; 16]) -> Self {
        Self {
            baked: self.baked,
            live,
        }
    }

    /// The colour the window shader draws a pixel of baked art in: the live
    /// colour at the first index 1-15 whose baked colour the pixel is, else
    /// the pixel as it is, alpha kept. Index 0 is the VDP's transparent
    /// colour and is never looked up. The shader's rule, as data, for
    /// `tests/window_palette.rs`; the library's own tests do not call it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn colour(&self, rgba: [u8; 4]) -> [u8; 4] {
        let rgb = [rgba[0], rgba[1], rgba[2]];
        (1..16)
            .find(|&index| psiv_data::color::cram_rgb(self.baked[index]) == rgb)
            .map_or(rgba, |index| {
                let [r, g, b] = psiv_data::color::cram_rgb(self.live[index]);
                [r, g, b, rgba[3]]
            })
    }
}
