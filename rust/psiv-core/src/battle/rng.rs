//! Roll sources.
//!
//! The kernel never owns a generator choice. Every formula takes a [`Rolls`]
//! and masks the raw word exactly where the cartridge does — `& 7` for a damage
//! draw, `& $3F` for a hit roll, `& $7F` for a drop — because the mask is part
//! of the formula, not part of the generator.
//!
//! That indirection is deliberate: **battle rolls in retail come from
//! `UpdateRNGSeed2` (`$04239E`), whose entropy is the VDP's H/V counter** —
//! the raster beam position at the instant of the call. No headless
//! reimplementation reproduces that without cycle-accurate emulation of the
//! whole frame, and `Battle_CalculateDamage` calls it sixteen times in a tight
//! loop, so the samples are not even independent of each other. Choosing what
//! to substitute is an open design question; this module refuses to answer it
//! and lets the caller supply the source.

/// A source of raw 16-bit rolls.
///
/// Implementations return the whole word. Masking belongs to the formula.
pub trait Rolls {
    /// The next raw roll.
    fn next_roll(&mut self) -> u16;
}

/// A fixed sequence of rolls, for tests and for replaying a captured stream.
///
/// Cycles when it runs out rather than panicking, so a one-element source is
/// the natural way to say "every draw is this value". [`SliceRolls::drawn`]
/// reports how many were taken, which is how a test checks that a formula drew
/// exactly as many times as the cartridge does.
#[derive(Debug, Clone)]
pub struct SliceRolls<'a> {
    values: &'a [u16],
    drawn: usize,
}

impl<'a> SliceRolls<'a> {
    /// Wraps a sequence. An empty slice yields zeros forever.
    #[must_use]
    pub const fn new(values: &'a [u16]) -> SliceRolls<'a> {
        SliceRolls { values, drawn: 0 }
    }

    /// How many rolls have been taken.
    #[must_use]
    pub const fn drawn(&self) -> usize {
        self.drawn
    }
}

impl Rolls for SliceRolls<'_> {
    fn next_roll(&mut self) -> u16 {
        let value = if self.values.is_empty() {
            0
        } else {
            self.values[self.drawn % self.values.len()]
        };
        self.drawn += 1;
        value
    }
}

/// The reseed constant `UpdateRNGSeed` installs when the seed's low word is
/// zero.
pub const RESEED: u32 = 0x2A6D_365B;

/// `UpdateRNGSeed` — retail `$04236C` (`ps4.asm:86070`).
///
/// The portable generator: a multiply-by-41 with a word fold. Encounter rolls
/// use it, and it is the obvious substitute for battle rolls if the design
/// session picks "same distributions, different stream".
///
/// ```text
///     move.l  (RNG_Seed).w, d1
///     tst.w   d1                  ; tests the LOW word ($FFFFEF0E)
///     bne.s   +
///     move.l  #$2A6D365B, d1      ; reseed only when that word is zero
/// +   move.l  d1, d0              ; d1 = d1 * 41, by shifts and adds
///     add.l   d1, d1
///     add.l   d1, d1
///     add.l   d0, d1
///     asl.l   #3, d1
///     add.l   d0, d1
///     move.w  d1, d0              ; d0.w = lo(41x)
///     swap    d1                  ; d1 = lo:hi
///     add.w   d1, d0              ; d0.w = lo + hi
///     move.w  d0, d1              ; d1 = lo:(lo+hi)
///     swap    d1                  ; d1 = (lo+hi):lo
///     move.l  d1, (RNG_Seed).w
/// ```
///
/// # A correction to the scout's summary
///
/// `docs/BATTLE_SCOUT.md` §2 renders the result as
/// `(lo+hi) << 16 | hi(41x)`. Tracing the instructions gives
/// `(lo+hi) << 16 | **lo**(41x)`: after `swap d1` the register holds
/// `lo:hi`, so `move.w d0,d1` overwrites the *low* half (the `hi` word) with
/// the sum, leaving `lo` in the high half for the final `swap` to bring down.
/// The scout's inline comments mislabel the halves at those two lines; its
/// transcribed opcodes are right. This implementation follows the
/// instructions, verified against `ps4.asm:86070`.
///
/// # What a consumer reads
///
/// The routine brackets itself with `movem.l d0-d1` / `movem.l (sp)+, d0-d1`,
/// so **it returns nothing in a register** — its only output is the seed in
/// RAM. Consumers then read `(RNG_Seed).w`, the *word* at `$FFFFEF0C`, which
/// on a big-endian 68000 is the high half of the longword: the sum. That is
/// what [`Rolls::next_roll`] returns here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lcg41 {
    seed: u32,
}

impl Lcg41 {
    /// Starts from an explicit seed.
    #[must_use]
    pub const fn new(seed: u32) -> Lcg41 {
        Lcg41 { seed }
    }

    /// The current seed longword.
    #[must_use]
    pub const fn seed(&self) -> u32 {
        self.seed
    }

    /// Advances the seed and returns the word a consumer would read.
    pub const fn step(&mut self) -> u16 {
        // `tst.w d1` tests the low word, so a zero *low half* triggers the
        // reseed even when the high half is set.
        let mut x = self.seed;
        if x as u16 == 0 {
            x = RESEED;
        }
        // The shift-and-add chain is exactly `x * 41` in 32-bit wrapping
        // arithmetic; written as a multiply because the intermediate values
        // are never observed.
        let product = x.wrapping_mul(41);
        let lo = product as u16;
        let hi = (product >> 16) as u16;
        let sum = lo.wrapping_add(hi);
        self.seed = ((sum as u32) << 16) | lo as u32;
        sum
    }
}

impl Default for Lcg41 {
    fn default() -> Lcg41 {
        Lcg41::new(RESEED)
    }
}

impl Rolls for Lcg41 {
    fn next_roll(&mut self) -> u16 {
        self.step()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_source_cycles_and_counts() {
        let mut rolls = SliceRolls::new(&[1, 2, 3]);
        let taken: Vec<u16> = (0..7).map(|_| rolls.next_roll()).collect();
        assert_eq!(taken, vec![1, 2, 3, 1, 2, 3, 1]);
        assert_eq!(rolls.drawn(), 7);

        let mut empty = SliceRolls::new(&[]);
        assert_eq!(empty.next_roll(), 0);
    }

    #[test]
    fn the_lcg_matches_a_hand_derived_step() {
        // Seed 1. Low word is 1, so no reseed.
        //   41 * 1        = $00000029      lo = $0029, hi = $0000
        //   sum           = $0029
        //   new seed      = $0029:$0029
        //   consumer word = $0029 = 41
        let mut lcg = Lcg41::new(1);
        assert_eq!(lcg.step(), 0x0029);
        assert_eq!(lcg.seed(), 0x0029_0029);
    }

    #[test]
    fn a_zero_low_word_takes_the_reseed_path() {
        // Seed 0. The low word is zero, so d1 becomes $2A6D365B = 711_800_411.
        //   41 * 711_800_411 = 29_183_816_851
        //   mod 2^32         =  3_414_013_075 = $CB7D_B493
        //   lo = $B493 (46_227), hi = $CB7D (52_093)
        //   sum = 98_320 = $18010, truncated to 16 bits = $8010
        //   new seed = $8010:$B493
        let mut lcg = Lcg41::new(0);
        assert_eq!(lcg.step(), 0x8010);
        assert_eq!(lcg.seed(), 0x8010_B493);
    }

    #[test]
    fn the_reseed_tests_the_low_half_only() {
        // A seed with a set high half but a zero low half still reseeds, which
        // is what `tst.w` means and is easy to get backwards.
        let mut zero_low = Lcg41::new(0xDEAD_0000);
        let mut plain_zero = Lcg41::new(0);
        assert_eq!(zero_low.step(), plain_zero.step());

        // Whereas a zero high half does not.
        let mut zero_high = Lcg41::new(0x0000_0001);
        assert_ne!(zero_high.step(), plain_zero.step());
    }

    #[test]
    fn the_lcg_is_deterministic_and_never_sticks() {
        let mut lcg = Lcg41::default();
        let first: Vec<u16> = (0..32).map(|_| lcg.step()).collect();
        let mut again = Lcg41::default();
        let second: Vec<u16> = (0..32).map(|_| again.step()).collect();
        assert_eq!(first, second);

        // A generator that fell into a fixed point would be a silent disaster.
        assert!(
            first.windows(2).any(|pair| pair[0] != pair[1]),
            "the sequence must actually move"
        );
    }
}
