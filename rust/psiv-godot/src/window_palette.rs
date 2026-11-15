//! The palette lines the native windows draw with, as data.
//!
//! Every native window draws art baked in CRAM line 2's `Pal_Init_Line_3`
//! colours (`$296300`, a copy of `Pal_Init`'s line 2): the window strip and
//! the dialogue and menu fonts (`dialogue/window.json`'s palette), the scroll
//! arrow, the dialogue and shop portraits and the STATUS portraits (each
//! pack record's `cram_line` is 2). A map load puts those words on line 2,
//! and a scene's tone, flash or fade changes them there like any other line.
//! The Hunter's Guild board draws the same font tiles on lines 1 and 3 too:
//! `Event_HuntersGuild` writes a collected job's name with `$A000` and a job
//! that is not open with `$E000` (`psiv_tools/guild_pack.py`).
//!
//! [`WindowLine`] binds a window's art to the line its indices are drawn on:
//! the sixteen words the art is baked in and the sixteen it draws with this
//! frame. The window shader (`cram_art::window_line_material`) finds each
//! pixel's index among the baked colours and draws the live word at that
//! index. [`WindowFamily`] names each window drawn that way and the nodes
//! that draw it; each [`WindowNode`] names its line. The shell
//! (`Field::apply_window_line`) attaches one material per line to every
//! node on it and sets the line from the shared CRAM.
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
    /// The Hunter's Guild list window (`job_board.rs`): its frame, cursor
    /// boxes and rows, each row on the line its state names.
    JobBoard,
    /// The ship's destination windows and the arrival's place name
    /// (`destination.rs`), but not the planetary map under them, which the
    /// screen draws from its own palette state on lines 0 and 1.
    Destination,
}

/// A node that draws window art, and so carries its line's material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowNode {
    /// `DialogueWindow`.
    Dialogue,
    /// `CampMenu`.
    Camp,
    /// `ShopWindow`.
    Shop,
    /// `JobBoardScreen`'s `Window` layer: the frame, the cursor boxes and
    /// the open jobs' names (`$C000`).
    JobBoard,
    /// `JobBoardScreen`'s `Done` layer: collected jobs' names (`$A000`).
    JobDone,
    /// `JobBoardScreen`'s `Closed` layer: the names of jobs that are not
    /// open (`$E000`).
    JobClosed,
    /// `DestinationScreen`'s `Windows` layer.
    Destination,
}

impl WindowFamily {
    /// Every family drawn from the live CRAM.
    pub(crate) const ALL: [Self; 6] = [
        Self::Dialogue,
        Self::Choice,
        Self::Camp,
        Self::Shop,
        Self::JobBoard,
        Self::Destination,
    ];

    /// The nodes that draw this family. The choice's quads are part of the
    /// dialogue window's draw list, so the message box's material draws them.
    pub(crate) const fn nodes(self) -> &'static [WindowNode] {
        match self {
            Self::Dialogue | Self::Choice => &[WindowNode::Dialogue],
            Self::Camp => &[WindowNode::Camp],
            Self::Shop => &[WindowNode::Shop],
            Self::JobBoard => &[
                WindowNode::JobBoard,
                WindowNode::JobDone,
                WindowNode::JobClosed,
            ],
            Self::Destination => &[WindowNode::Destination],
        }
    }
}

impl WindowNode {
    /// The CRAM line the node's art indices are drawn on: the tile word's
    /// bits 13-14 (`$C000` line 2, `$A000` line 1, `$E000` line 3).
    pub(crate) const fn line(self) -> usize {
        match self {
            Self::JobDone => 1,
            Self::JobClosed => 3,
            Self::Dialogue | Self::Camp | Self::Shop | Self::JobBoard | Self::Destination => {
                WINDOW_LINE
            }
        }
    }
}

/// The CRAM words that show `colours` (the inverse of
/// `psiv_data::color::cram_rgb`), or `None` unless there are sixteen and
/// each is on the ramps. The guild's lines are carried as colours; this is
/// what they draw while no live palette is known.
pub(crate) fn cram_words(colours: &[[u8; 3]]) -> Option<[u16; 16]> {
    let colours: &[[u8; 3]; 16] = colours.try_into().ok()?;
    let mut words = [0u16; 16];
    for (word, &colour) in words.iter_mut().zip(colours) {
        *word = psiv_data::color::cram_word(colour)?;
    }
    Some(words)
}

/// What CRAM `line` holds for the window art while no live palette is
/// known, when that is not the baked line 2: the guild map's lines 1 and 3,
/// which `guild/guild.json` carries as the colours a collected job and a job
/// that is not open are drawn in. No other window draws off line 2.
pub(crate) fn unlit_line(line: usize, guild: Option<&psiv_data::GuildBoard>) -> Option<[u16; 16]> {
    let guild = guild?;
    match line {
        1 => cram_words(&guild.done_line),
        3 => cram_words(&guild.closed_line),
        _ => None,
    }
}

/// The words one window node draws its art with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowLine {
    /// The words the art is baked in.
    pub(crate) baked: [u16; 16],
    /// The words it draws with.
    pub(crate) live: [u16; 16],
}

impl WindowLine {
    /// Art baked in `baked` whose indices are drawn on CRAM `line`: that
    /// line of `words` (`Palette_Table_Buffer`) when the palette is `live`;
    /// `unlit` when it is not, or the baked words, unchanged, when no
    /// `unlit` is given. `None` when fewer than sixteen baked words are
    /// known or `line` is not 0-3.
    pub(crate) fn new(
        line: usize,
        words: &[u16; 64],
        live: bool,
        baked: &[u16],
        unlit: Option<[u16; 16]>,
    ) -> Option<Self> {
        let baked: [u16; 16] = baked.get(..16)?.try_into().ok()?;
        let start = line.checked_mul(16).filter(|&start| start < 64)?;
        let live = if live {
            words[start..start + 16].try_into().ok()?
        } else {
            unlit.unwrap_or(baked)
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
