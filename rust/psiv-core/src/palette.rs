//! The cartridge's palette arithmetic on 16-bit CRAM words: one owner for every
//! tone, flash and fade step a scene applies to `Palette_Table_Buffer`
//! (`$FFFFFB00`, 64 words) and its copy `Palette_Table_Buffer_2` (`$FFFFFB80`).
//!
//! A Genesis CRAM word is `----bbb-ggg-rrr-`: red in bits 1-3, green in 5-7,
//! blue in 9-11. Each routine here is transcribed from the US image with its own
//! width, masks and wrap: a component that a routine steps past zero borrows
//! from the bits above it exactly as `subq.w`/`subi.w` does, and the result is
//! kept, not clamped. The functions are pure; the runtime's CRAM model
//! (`psiv-runtime/src/cram.rs`) decides when each runs. Source notes and the
//! byte checks are in `docs/scenes/SCENE_PRESENTATION.md`, "CRAM state".

/// Words in `Palette_Table_Buffer`: four lines of sixteen.
pub const PALETTE_WORDS: usize = 64;

/// The word `Pal_IncreaseTone` leaves alone (`cmpi.w #$EEE, d1`).
const WHITE: u16 = 0x0EEE;

/// Whether a palette step that reads `Main_Frame_Count` runs this frame:
/// `move.w (Main_Frame_Count).w, d0` / `andi.w #3, d0` / `bne` in
/// `Pal_IncreaseTone` (`$058608`), `loc_58666` and both recovery flashes
/// (`$06D3BA`, `$06D40A`).
#[must_use]
pub const fn tone_phase(main_frame_count: u16) -> bool {
    main_frame_count & 3 == 0
}

/// `Pal_IncreaseTone` (`$058608..$058665`) on one word: each component up by
/// one level, a component that wraps out of its field becoming `$E`; `$EEE`
/// itself is skipped. `addq.w #2` / `andi.w #$E` / `bne` / `move.w #$E` for red,
/// the same with `#$20`/`#$E0` and `#$200`/`#$E00`; bits outside the three
/// fields are dropped by the `or.w` rebuild.
#[must_use]
pub const fn increase_tone(word: u16) -> u16 {
    if word == WHITE {
        return word;
    }
    let mut red = word.wrapping_add(0x0002) & 0x000E;
    if red == 0 {
        red = 0x000E;
    }
    let mut green = word.wrapping_add(0x0020) & 0x00E0;
    if green == 0 {
        green = 0x00E0;
    }
    let mut blue = word.wrapping_add(0x0200) & 0x0E00;
    if blue == 0 {
        blue = 0x0E00;
    }
    red | green | blue
}

/// `loc_58666` (`$058666..$0586CB`) on one word: for red, then green, then
/// blue, when the component differs from `target`'s, `subq.w #2`/`subi.w
/// #$20`/`subi.w #$200` on the whole word. Each test re-reads the word the
/// previous one wrote, and a component below its target borrows from the
/// fields above it.
#[must_use]
pub const fn restore_tone(word: u16, target: u16) -> u16 {
    let mut word = word;
    if word & 0x000E != target & 0x000E {
        word = word.wrapping_sub(0x0002);
    }
    if word & 0x00E0 != target & 0x00E0 {
        word = word.wrapping_sub(0x0020);
    }
    if word & 0x0E00 != target & 0x0E00 {
        word = word.wrapping_sub(0x0200);
    }
    word
}

/// `loc_6D3BA` (`$06D3BA..$06D409`), `Event_Recovery`'s brightening flash, on one
/// word: red and green each up one level unless already `$E`; blue is masked
/// and kept (`andi.w #$E00, d3` with no add).
#[must_use]
pub const fn flash_brighten(word: u16) -> u16 {
    let mut red = word & 0x000E;
    if red != 0x000E {
        red += 0x0002;
    }
    let mut green = word & 0x00E0;
    if green != 0x00E0 {
        green += 0x0020;
    }
    red | green | (word & 0x0E00)
}

/// `loc_6D40A` (`$06D40A..$06D467`), the flash back, on one word: red and green
/// each down one level while they differ from `target`'s; blue is masked and
/// kept. The step is on the masked component, so one already at zero becomes
/// `$FFFE`/`$FFE0` and its high bits survive the `or.w` rebuild.
#[must_use]
pub const fn flash_restore(word: u16, target: u16) -> u16 {
    let mut red = word & 0x000E;
    if red != target & 0x000E {
        red = red.wrapping_sub(0x0002);
    }
    let mut green = word & 0x00E0;
    if green != target & 0x00E0 {
        green = green.wrapping_sub(0x0020);
    }
    red | green | (word & 0x0E00)
}

/// One pass of `PalFadeOut_ClrSpriteTbl` (`$04223E`) and `Pal_VariableFadeOut`
/// (`$05B168`) on one word: each of the four nibbles down by one unless zero
/// (`moveq #3, d5` / `rol.w #4`).
#[must_use]
pub const fn fade_out_step(word: u16) -> u16 {
    let mut d0 = word;
    let mut nibble = 0;
    while nibble < 4 {
        // `andi.w #$F, d1` / `beq.s` / `subq.w #1, d1`: a zero nibble stays.
        let d1 = (d0 & 0x000F).saturating_sub(1);
        d0 &= 0xFFF0;
        d0 |= d1;
        d0 = d0.rotate_left(4);
        nibble += 1;
    }
    d0
}

/// One pass of `Pal_FadeIn` (`$0421D4`) and `Pal_VariableFadeIn` (`$05B1B0`) on
/// one word: each nibble that differs from `target`'s goes up by one. A nibble
/// at `$F` carries into the next one (`addq.w #1, d2` / `or.w d2, d0` with no
/// mask), as the cartridge does. The flag is the pass's `st d5`.
#[must_use]
pub const fn fade_in_step(word: u16, target: u16) -> (u16, bool) {
    let mut d0 = word;
    let mut d1 = target;
    let mut changed = false;
    let mut nibble = 0;
    while nibble < 4 {
        let d2 = d0 & 0x000F;
        if d2 != d1 & 0x000F {
            changed = true;
            d0 = (d0 & 0xFFF0) | (d2 + 1);
        }
        d0 = d0.rotate_right(4);
        d1 = d1.rotate_right(4);
        nibble += 1;
    }
    (d0, changed)
}

/// One stage of `Pal_VariableFadeToRed` (`$05B2C0`) on one word: blue down one
/// level while `$F00` is non-zero, then green while `$F0` is; red is kept. The
/// test masks four bits and the step is two levels' worth of one, so an odd
/// field borrows.
#[must_use]
pub const fn fade_to_red_step(word: u16) -> u16 {
    let mut word = word;
    if word & 0x0F00 != 0 {
        word = word.wrapping_sub(0x0200);
    }
    if word & 0x00F0 != 0 {
        word = word.wrapping_sub(0x0020);
    }
    word
}

/// One stage of `Pal_VariableFadeFromRed` (`$05B30C`) on one word: blue, then
/// green, up one level while it differs from `target`'s.
#[must_use]
pub const fn fade_from_red_step(word: u16, target: u16) -> u16 {
    let mut word = word;
    if word & 0x0F00 != target & 0x0F00 {
        word = word.wrapping_add(0x0200);
    }
    if word & 0x00F0 != target & 0x00F0 {
        word = word.wrapping_add(0x0020);
    }
    word
}

/// The stages `Pal_VariableFadeToRed`/`FromRed` run: `moveq #7, d0` / `dbf`.
pub const RED_FADE_STAGES: u16 = 8;

/// Frames a `dbf` wait on a count of `count` spends: one more than the count.
#[must_use]
pub const fn dbf_frames(count: u8) -> u16 {
    count as u16 + 1
}

/// How many passes `Pal_VariableFadeOut` makes over `words` before its
/// `tst.w d7` finds every word zero: the largest nibble, or one pass when
/// the palette is already black.
#[must_use]
pub fn variable_fade_out_passes(words: &[u16]) -> u16 {
    let mut words = words.to_vec();
    let mut passes = 0u16;
    loop {
        passes += 1;
        let mut any = 0u16;
        for word in &mut words {
            *word = fade_out_step(*word);
            any |= *word;
        }
        if any == 0 {
            return passes;
        }
    }
}

/// How many passes `Pal_VariableFadeIn` makes from black toward `target`: one
/// per level of the largest nibble, and the last that changes nothing.
#[must_use]
pub fn variable_fade_in_passes(target: &[u16]) -> u16 {
    let mut words = vec![0u16; target.len()];
    let mut passes = 0u16;
    loop {
        passes += 1;
        let mut changed = false;
        for (word, &goal) in words.iter_mut().zip(target) {
            let (next, moved) = fade_in_step(*word, goal);
            *word = next;
            changed |= moved;
        }
        if !changed {
            return passes;
        }
        // A carry out of a `$F` nibble can keep a word from ever matching;
        // the cartridge would spin forever, the model stops at the bound.
        if passes == u16::MAX {
            return passes;
        }
    }
}

/// Frames `Pal_VariableFadeOut` spends with `delay` in `$FFFFED52`: each pass
/// waits `delay + 1` VBlanks (`move.b ($FFFFED52).w, d0` / `dbf`).
#[must_use]
pub fn variable_fade_out_frames(words: &[u16], delay: u8) -> u16 {
    variable_fade_out_passes(words).saturating_mul(dbf_frames(delay))
}

/// Frames `Pal_VariableFadeIn` spends toward `target`: `VDP_EnableDisplay`'s
/// one `VInt_Prepare`, then `delay + 1` per pass.
#[must_use]
pub fn variable_fade_in_frames(target: &[u16], delay: u8) -> u16 {
    1 + variable_fade_in_passes(target).saturating_mul(dbf_frames(delay))
}

/// The `(first, second)` palette words each pass of a
/// [`crate::PresentationOp::PaletteCycleFromTable`] writes, read from `words`, the
/// big-endian words from its `source_rom_addr` on (the pack's copy). `None`
/// when `words` ends before a read the cartridge makes, or when a pass finds
/// no non-negative word to write (the cartridge would loop forever).
#[must_use]
pub fn palette_cycle_writes(words: &[u16], second_skip: u8, steps: u8) -> Option<Vec<(u16, u16)>> {
    if !second_skip.is_multiple_of(2) {
        return None;
    }
    let skip = usize::from(second_skip / 2);
    let mut writes = Vec::with_capacity(usize::from(steps));
    let mut next = 0usize;
    for _ in 0..steps {
        let mut restarted = false;
        let first = loop {
            let word = *words.get(next)?;
            next += 1;
            if word & 0x8000 == 0 {
                break word;
            }
            if restarted {
                return None;
            }
            restarted = true;
            next = 0;
        };
        let second = *words.get(next + skip)?;
        writes.push((first, second));
    }
    Some(writes)
}

/// The three words each step of `Event_ZioFortBarrier`'s ramp (`loc_6ED4E`,
/// `$06ED4E..$06ED6B`) writes to `$2(a1)`, `$4(a1)` and `$6(a1)`: `move.w
/// (a2)+`, then `$C(a2)` and `$1A(a2)` from the advanced pointer, so step `n`
/// reads table words `n`, `n + 7` and `n + 14`. `None` when `words` is short.
#[must_use]
pub fn palette_ramp_writes(words: &[u16], steps: u8) -> Option<Vec<[u16; 3]>> {
    (0..usize::from(steps))
        .map(|n| Some([*words.get(n)?, *words.get(n + 7)?, *words.get(n + 14)?]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values are worked by hand from the instruction listings cited
    // on each function, not produced by the functions.

    #[test]
    fn increase_tone_steps_each_component_and_caps_at_e() {
        // r=2,g=4,b=6 -> 4,6,8.
        assert_eq!(increase_tone(0x0642), 0x0864);
        // r=$C -> $E; g=$E stays $E (wrap -> cap); b=0 -> 2.
        assert_eq!(increase_tone(0x00EC), 0x02EE);
        // Already white is skipped, even its odd bits are not touched.
        assert_eq!(increase_tone(0x0EEE), 0x0EEE);
        // Black climbs to $222.
        assert_eq!(increase_tone(0x0000), 0x0222);
        // A high bit is dropped by the rebuild.
        assert_eq!(increase_tone(0x8000), 0x0222);
    }

    #[test]
    fn restore_tone_borrows_from_the_whole_word() {
        // $EEE toward $246: every component one level down.
        assert_eq!(restore_tone(0x0EEE, 0x0246), 0x0CCC);
        // Equal components are left alone.
        assert_eq!(restore_tone(0x0246, 0x0246), 0x0246);
        // Red at 0 with a target of 2: $0000 - 2 = $FFFE. Green of $FFFE is $E0
        // (target 0): - $20 = $FFDE. Blue of $FFDE is $E00: - $200 = $FDDE.
        assert_eq!(restore_tone(0x0000, 0x0002), 0xFDDE);
    }

    #[test]
    fn the_flashes_leave_blue_alone() {
        assert_eq!(flash_brighten(0x0246), 0x0268);
        assert_eq!(flash_brighten(0x0EEE), 0x0EEE);
        assert_eq!(flash_brighten(0x0E0C), 0x0E2E);
        assert_eq!(flash_restore(0x0268, 0x0246), 0x0246);
        // Masked red 0 toward 2: $FFFE, ored with green $20 and blue $200.
        assert_eq!(flash_restore(0x0220, 0x0002), 0xFFFE);
    }

    #[test]
    fn fades_step_nibbles() {
        assert_eq!(fade_out_step(0x0EEE), 0x0DDD);
        assert_eq!(fade_out_step(0x0102), 0x0001);
        assert_eq!(fade_in_step(0x0000, 0x0246), (0x0111, true));
        assert_eq!(fade_in_step(0x0246, 0x0246), (0x0246, false));
        // A $F nibble that differs carries into the nibble above, which then
        // differs too: $F -> $10, ror -> $0001, its 1 -> 2, ror -> $2000,
        // two more rotations -> $0020.
        assert_eq!(fade_in_step(0x000F, 0x0000), (0x0020, true));
        assert_eq!(fade_to_red_step(0x0EEE), 0x0CCE);
        assert_eq!(fade_to_red_step(0x000E), 0x000E);
        assert_eq!(fade_from_red_step(0x000E, 0x0EEE), 0x022E);
    }

    #[test]
    fn variable_fade_lengths_follow_the_largest_nibble() {
        // $EEE: fourteen passes out; fifteen in (the last changes nothing).
        assert_eq!(variable_fade_out_passes(&[0x0EEE, 0x0000]), 14);
        assert_eq!(variable_fade_in_passes(&[0x0EEE, 0x0000]), 15);
        // Black: one pass each way.
        assert_eq!(variable_fade_out_passes(&[0; 4]), 1);
        assert_eq!(variable_fade_in_passes(&[0; 4]), 1);
        // With $ED52 = 1, two frames a pass; fade-in adds the display frame.
        assert_eq!(variable_fade_out_frames(&[0x0246], 1), 12);
        assert_eq!(variable_fade_in_frames(&[0x0246], 1), 1 + 7 * 2);
    }

    #[test]
    fn the_zio_ramp_reads_three_interleaved_series() {
        let table: Vec<u16> = (0..21).collect();
        let writes = palette_ramp_writes(&table, 7).unwrap();
        assert_eq!(writes[0], [0, 7, 14]);
        assert_eq!(writes[6], [6, 13, 20]);
        assert_eq!(palette_ramp_writes(&table[..20], 7), None);
    }

    #[test]
    fn the_tone_phase_is_every_fourth_frame() {
        assert!(tone_phase(0));
        assert!(!tone_phase(1));
        assert!(tone_phase(0xFFFC));
        assert_eq!(dbf_frames(7), 8);
    }
}
