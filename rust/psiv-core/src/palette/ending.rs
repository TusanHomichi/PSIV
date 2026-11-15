//! `Cutscene_Ending`'s palette loops that step on `Main_Frame_Count & 7`:
//! the staff roll's single-word fade out and back in (`loc_79C8E`,
//! `loc_79CCA`) and the credits' table ramp (`loc_7A0A6`). Each answers which
//! frame of the loop, counted from its first, makes each write, and how many
//! frames the loop spends: its `VInt_Prepare` calls. The ROM words come from
//! the caller (the pack); nothing here holds a copy of them.

/// Whether a loop that tests `andi.w #7` steps on this frame. The staff roll
/// reads the low byte (`move.b (Main_Frame_Count+1).w, d1`), the credits the
/// word; the low three bits are the same.
#[must_use]
pub const fn eighth_phase(main_frame_count: u16) -> bool {
    main_frame_count & 7 == 0
}

/// The first frame, counted from a loop's first, whose count is a stepping
/// one ([`eighth_phase`]).
const fn first_eighth(main_frame_count: u16) -> u16 {
    main_frame_count.wrapping_neg() & 7
}

/// A staff-roll word loop: each stored word and the frame, counted from the
/// loop's first, whose loop body stores it, then the frames the loop spends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordLoop {
    /// `(frame, word)`, in frame order.
    pub writes: Vec<(u16, u16)>,
    /// The loop's `VInt_Prepare` calls: the frame whose step ends it, which
    /// spends none. A loop that never ends (the cartridge spins) is cut at
    /// `u16::MAX`, with the writes before it.
    pub frames: u16,
}

/// `loc_79C8E` (`$079C8E..$079CAD`) from `word` on a loop whose first frame
/// has count `main_frame_count`: on each stepping frame `subi.w #step` (word
/// width, borrowing through bit 15); a zero result ends the loop without a
/// store (`beq loc_79CAE` before the `move.w d1, (a0)`).
#[must_use]
pub fn staff_roll_fade_out(word: u16, main_frame_count: u16, step: u16) -> WordLoop {
    word_loop(word, main_frame_count, |word| {
        let next = word.wrapping_sub(step);
        (next != 0).then_some(next)
    })
}

/// `loc_79CCA` (`$079CCA..$079CED`): `addi.w #step` on each stepping frame; a
/// sum with `target_bit` set ends the loop without a store (`btst #$C, d1` /
/// `bne loc_79CEE`). The test reads the data register, whose upper word the
/// sum never reaches: a bit past 15 never ends the loop.
#[must_use]
pub fn staff_roll_fade_in(word: u16, main_frame_count: u16, step: u16, target_bit: u8) -> WordLoop {
    word_loop(word, main_frame_count, |word| {
        let next = word.wrapping_add(step);
        let ends = next
            .checked_shr(u32::from(target_bit))
            .is_some_and(|bit| bit & 1 != 0);
        (!ends).then_some(next)
    })
}

fn word_loop(mut word: u16, main_frame_count: u16, next: impl Fn(u16) -> Option<u16>) -> WordLoop {
    let mut writes = Vec::new();
    let mut at = u32::from(first_eighth(main_frame_count));
    while at < u32::from(u16::MAX) {
        let frame = at as u16;
        match next(word) {
            Some(stored) => {
                word = stored;
                writes.push((frame, stored));
            }
            None => {
                return WordLoop {
                    writes,
                    frames: frame,
                };
            }
        }
        at += 8;
    }
    WordLoop {
        writes,
        frames: u16::MAX,
    }
}

/// `loc_7A0A6`'s timing on a loop whose first frame has count
/// `main_frame_count`: the frame, counted from the loop's first, of each of
/// its `steps` steps (the first `steps` stepping frames), and the frames the
/// loop spends, the last step's frame included (every pass ends in
/// `VInt_Prepare`; the `cmpi.w #$E, d6` / `bcc` exit is tested before the
/// next). No steps spend no frame.
#[must_use]
pub fn credits_ramp_frames(main_frame_count: u16, steps: u8) -> (Vec<u16>, u16) {
    let first = first_eighth(main_frame_count);
    let at: Vec<u16> = (0..u16::from(steps)).map(|n| first + 8 * n).collect();
    let frames = at.last().map_or(0, |last| last + 1);
    (at, frames)
}

/// One step of `loc_7A0A6`: the primary words, and the secondary words once
/// the step reaches the secondary table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RampStep {
    /// `primary_words` words from primary word `d5`.
    pub primary: Vec<u16>,
    /// `secondary_words` words from secondary word `2 * (d6 - from)`.
    pub secondary: Option<Vec<u16>>,
}

/// The words `loc_7A0A6`'s `steps` steps copy (`trap #1` with `moveq #7,
/// d7`, then `moveq #1, d7`): step `d6` (from 0) reads the primary table at
/// word `d5 = steps - d6` (`move.w d5, d1` / `add.w d1, d1`: a byte offset),
/// and from step `from` on (`subi.w #9, d1` / `bmi`) the secondary table at
/// byte `4 * (d6 - from)` (`add.w d1, d1` twice), whatever the copy's length.
/// `None` when a read runs past either table.
#[must_use]
pub fn credits_ramp_rows(
    primary: &[u16],
    primary_words: u8,
    secondary: &[u16],
    secondary_words: u8,
    from: u8,
    steps: u8,
) -> Option<Vec<RampStep>> {
    (0..steps)
        .map(|d6| {
            let d5 = usize::from(steps - d6);
            let primary = primary.get(d5..d5 + usize::from(primary_words))?.to_vec();
            let secondary = match d6.checked_sub(from) {
                Some(late) => {
                    let start = 2 * usize::from(late);
                    Some(
                        secondary
                            .get(start..start + usize::from(secondary_words))?
                            .to_vec(),
                    )
                }
                None => None,
            };
            Some(RampStep { primary, secondary })
        })
        .collect()
}
