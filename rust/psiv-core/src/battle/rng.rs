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
/// `docs/battle/BATTLE_SCOUT.md` §2 renders the result as
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

impl Lcg41 {
    /// The word a consumer reads at `RNG_Seed` (`$FFFFEF0C`).
    ///
    /// On a big-endian 68000 that address is the *high* half of the longword,
    /// which after [`Lcg41::step`] holds `lo + hi`. Both cartridge generators
    /// read this word; [`Rng2`] also rotates it.
    #[must_use]
    pub const fn seed_word(&self) -> u16 {
        (self.seed >> 16) as u16
    }

    /// Replaces the word at `$FFFFEF0C`, leaving the low half alone.
    pub const fn set_seed_word(&mut self, word: u16) {
        self.seed = ((word as u32) << 16) | (self.seed & 0xFFFF);
    }

    /// `ror (RNG_Seed).w` — rotates the seed word right by one bit.
    ///
    /// A rotate, not a shift: the bit that falls off the bottom re-enters at
    /// the top, so sixteen calls return the word to where it started. That
    /// period is exactly the length of a damage roll's draw loop, which is why
    /// [`Rng2`] is worth understanding before trusting it.
    pub const fn rotate_seed_word(&mut self) {
        self.set_seed_word(self.seed_word().rotate_right(1));
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

/// The stand-in for the VDP H/V counter read, `$C00008`.
///
/// # Why there is a stand-in at all
///
/// `UpdateRNGSeed2`'s entropy is the raster beam position at the instant of the
/// call. `docs/RUNTIME_DESIGN.md` "RNG design" ratifies replacing that one term
/// with a deterministic surrogate and keeping everything else — the `ror`, the
/// frame-count term, the masks, the sixteen-draw loop — as the cartridge wrote
/// it.
///
/// # Why this particular value
///
/// A word read of `$C00008` returns the V counter in the high byte and the H
/// counter in the low. Battle logic runs from the main loop after the vertical
/// interrupt returns, i.e. during active display, so a representative sample is
/// mid-frame and mid-scanline: `$40` and `$57`.
///
/// The high byte is decorative. **Only the low three bits of
/// `hv + frame_count` reach the damage sum**, because every draw is masked with
/// `& 7`, and they matter more than they look — see [`Rng2`].
pub const HV_SURROGATE: u16 = 0x4057;

/// `UpdateRNGSeed2` — retail `$04239E` (`ps4.asm:86097`), the battle mixer.
///
/// ```text
///     move.w  $8(a5), d0                  ; a5 = $C00000, so this is $C00008,
///                                         ;   the VDP H/V counter
///     add.w   (Main_Frame_Count).w, d0
///     sub.w   (RNG_Seed).w, d0            ; the word BEFORE the rotate
///     ror     (RNG_Seed).w
///     rts
/// ```
///
/// Borrows the [`Lcg41`] rather than owning a seed, because the cartridge has
/// **one** 32-bit seed for the whole game and both generators work over it: the
/// field engine's encounter and wander draws and battle's rolls are the same
/// word. Constructing a `Rng2` over the runtime's shared [`Lcg41`] is what
/// keeps that true.
///
/// # The frame count is not optional
///
/// The H/V term is a constant here, so within one frame every draw shares the
/// same additive constant and the only thing that varies across the sixteen
/// draws of a damage roll is the rotating seed word. Sixteen rotations of a
/// sixteen-bit word return it to the start, so the whole sum `S` is a function
/// of `(seed_word, (hv + frame_count) & 7)`.
///
/// Across all 65,536 seed words the mean of `S` is **exactly 56** for every one
/// of the eight residues — the same mean the ideal distribution has. The
/// *spread* is not:
///
/// | `(hv + frame) & 7` | sd of `S` | range of `S` |
/// |---|---|---|
/// | 0 | 10.00 | 0..86 |
/// | 1 | 8.25 | 16..88 |
/// | 2 | 8.25 | 32..83 |
/// | **3** | **2.00** | **48..64** |
/// | 4 | 8.25 | 29..80 |
/// | 5 | 8.25 | 24..96 |
/// | 6 | 10.00 | 26..112 |
/// | 7 | 14.00 | 0..112 |
///
/// (Ideal, for comparison: mean 56, sd 9.17, range 0..112.)
///
/// Residue 3 collapses damage variance almost to nothing. On hardware the H/V
/// term's own low bits move between calls and no residue persists; here, the
/// **frame counter is what keeps them moving**. A caller that passes a fixed
/// `frame_count` can pin the battle in one regime for its whole duration, and
/// if that regime is 3 the damage rolls go nearly deterministic.
///
/// So: pass the runtime's real `Main_Frame_Count`. Tests that want a fixed
/// stream should use [`SliceRolls`] instead of freezing this one.
#[derive(Debug)]
pub struct Rng2<'a> {
    seed: &'a mut Lcg41,
    frame_count: u16,
    hv: u16,
}

impl<'a> Rng2<'a> {
    /// Wraps the shared seed with an explicit frame count and H/V surrogate.
    pub const fn new(seed: &'a mut Lcg41, frame_count: u16, hv: u16) -> Rng2<'a> {
        Rng2 {
            seed,
            frame_count,
            hv,
        }
    }

    /// Wraps the shared seed with [`HV_SURROGATE`].
    pub const fn with_surrogate(seed: &'a mut Lcg41, frame_count: u16) -> Rng2<'a> {
        Rng2::new(seed, frame_count, HV_SURROGATE)
    }

    /// The residue that decides this generator's spread — see [`Rng2`].
    #[must_use]
    pub const fn residue(&self) -> u16 {
        self.hv.wrapping_add(self.frame_count) & 7
    }
}

impl Rolls for Rng2<'_> {
    fn next_roll(&mut self) -> u16 {
        // The subtraction reads the seed word *before* the rotate; getting
        // that order backwards shifts every draw by one rotation.
        let value = self
            .hv
            .wrapping_add(self.frame_count)
            .wrapping_sub(self.seed.seed_word());
        self.seed.rotate_seed_word();
        value
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
    fn the_seed_word_is_the_high_half_and_rotates() {
        let mut lcg = Lcg41::new(0x8001_1234);
        assert_eq!(lcg.seed_word(), 0x8001, "the word at $FFFFEF0C");
        lcg.rotate_seed_word();
        // ror by one: the low bit wraps to the top. $8001 -> $C000.
        assert_eq!(lcg.seed_word(), 0xC000);
        assert_eq!(lcg.seed() & 0xFFFF, 0x1234, "the low half is untouched");

        // Sixteen rotations are the identity, which is why a damage roll's
        // sixteen draws see every rotation exactly once.
        let mut wheel = Lcg41::new(0xBEEF_0000);
        for _ in 0..16 {
            wheel.rotate_seed_word();
        }
        assert_eq!(wheel.seed_word(), 0xBEEF);
    }

    #[test]
    fn rng2_subtracts_the_word_before_rotating_it() {
        let mut lcg = Lcg41::new(0x0003_0000);
        let mut rng = Rng2::new(&mut lcg, 10, 100);
        // First draw uses seed word 3: 100 + 10 - 3 = 107.
        assert_eq!(rng.next_roll(), 107);
        // Then the word is $0003 rotated right -> $8001, so: 110 - $8001.
        assert_eq!(rng.next_roll(), 110u16.wrapping_sub(0x8001));
        // Ends the borrow so the seed can be inspected.
        let _ = rng;
        assert_eq!(lcg.seed_word(), 0x8001u16.rotate_right(1));
    }

    #[test]
    fn rng2_shares_the_field_engine_seed() {
        // The point of borrowing rather than owning: a battle roll moves the
        // same word an encounter roll reads. A battle that ran its own seed
        // would silently desync the field stream.
        let mut lcg = Lcg41::new(0x1234_5678);
        {
            let mut rng = Rng2::with_surrogate(&mut lcg, 0);
            for _ in 0..3 {
                rng.next_roll();
            }
        }
        assert_eq!(lcg.seed_word(), 0x1234u16.rotate_right(3));
        assert_eq!(lcg.seed() & 0xFFFF, 0x5678, "the LCG's low half survives");
    }

    /// The damage sum for one seed word under one residue, computed the way
    /// `Battle_CalculateDamage` does.
    fn damage_sum(seed_word: u16, residue: u16) -> u32 {
        let mut lcg = Lcg41::new(u32::from(seed_word) << 16);
        let mut rng = Rng2::new(&mut lcg, 0, residue);
        (0..16).map(|_| u32::from(rng.next_roll() & 7)).sum()
    }

    #[test]
    fn the_surrogate_never_moves_the_mean_but_does_move_the_spread() {
        // Documented in `Rng2`. The mean is exactly 56 for every residue —
        // the ideal value — so the surrogate choice cannot bias damage.
        for residue in 0..8u16 {
            let total: u64 = (0..=u16::MAX)
                .map(|w| u64::from(damage_sum(w, residue)))
                .sum();
            assert_eq!(
                total,
                56 * 65536,
                "residue {residue} must average exactly 56"
            );
        }

        // The spread is another matter, and residue 3 is the trap.
        let span = |residue: u16| {
            let sums = (0..=u16::MAX).map(|w| damage_sum(w, residue));
            sums.fold((u32::MAX, 0u32), |(lo, hi), s| (lo.min(s), hi.max(s)))
        };
        assert_eq!(span(3), (48, 64), "residue 3 collapses the range");
        assert_eq!(span(7), (0, 112), "residue 7 reaches the full range");
        assert_eq!(
            HV_SURROGATE & 7,
            7,
            "the chosen surrogate lands on the widest residue at frame 0"
        );
    }

    #[test]
    fn only_two_seed_words_are_degenerate() {
        // A word whose sixteen rotations are all equal makes every draw equal.
        // $0000 and $FFFF are the only two, and they are degenerate on
        // hardware too — the H/V jitter is what breaks them there.
        let degenerate: Vec<u16> = (0..=u16::MAX)
            .filter(|w| (0..16).all(|k| w.rotate_right(k) == *w))
            .collect();
        assert_eq!(degenerate, vec![0x0000, 0xFFFF]);
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
