//! An event battle's entry: the frames from a scene's battle request to the
//! battle's opening roll.
//!
//! A scene that sets `Routine_Exit_Flags` bit 3 returns through
//! `RunEventBattle` (`ps4.asm:120803`): a `VInt_Prepare`, the event arm of
//! `FieldRoutine_Battle2` (`loc_58498`, fifteen band passes, or `loc_58482`'s
//! forty tone frames for the Profound Darkness, index `$1A`), and
//! `loc_585C8`'s `VInt_Prepare` into `GameMode_LoadBattle` (`ps4.asm:9911`).
//! That mode fills the palette with `$EEE`, loads the stage while the frame
//! count holds, fades the stage in from white (`loc_6B20`, one nibble per
//! frame toward the battle palette, the planes sweeping in alongside) and only
//! then calls `loc_B62A` (`ps4.asm:10043`), the battle's one opening roll. Every
//! encounter takes the same `GameMode_LoadBattle`; its own field arm is
//! `loc_584EE`, and the port still rolls it at once (#678).
//!
//! So the battle starts, and rolls, at the end of the entry, and the seed has
//! stepped on each of the entry's counted frames. Tape 45
//! (`tools/certify.py` `TAPE_45`, `Event_FractOozeFound`, battle 7) measures
//! the frames, counted from the frame the C ends the text (7799): 7800-7817
//! advance `Main_Frame_Count`, 7818-7854 are the stage's lag frames, 7855-7872
//! count again (the `$EEE` frame, the fade and the sweep's end), 7873-7875 lag,
//! and 7875's seed word is rotated by the roll. The screen is white from 7814,
//! steps from 7857 and is at the battle palette by 7870.
//!
//! The lag depends on what the stage decompresses; the port spends battle 7's
//! 37 frames on every event battle (source notes, #663).

use crate::{Runtime, RuntimeEvent};

/// Battle 7's frames, counted from the request frame: the counted frames before
/// the loads (the `RunEventBattle` frame, the fifteen passes, `loc_585C8` and
/// `GameMode_LoadBattle`'s first frame).
const BEFORE_LOADS: u16 = 18;
/// The stage's lag frames on tape 45 (7818-7854).
const LOAD_LAG: u16 = 37;
/// The counted frames from the `$EEE` frame through the sweep (7855-7872).
const FADE_COUNTED: u16 = 18;
/// The lag frames before the roll, the roll's own included (7873-7875).
const ROLL_LAG: u16 = 3;
/// `loc_58498`'s passes (`move.w #$E, d7`).
const BAND_PASSES: u16 = 15;
/// `loc_58482`'s tone frames (`move.w #$27, d7`).
const DARKNESS_TONE_FRAMES: u16 = 40;
/// The event battle index `RunEventBattle` sends to `loc_58482`.
const PROFOUND_DARKNESS: u16 = 0x1A;
/// The first frame the screen is all white: the passes' last frame.
const WHITE_FROM: u16 = 14;
/// `GameMode_LoadBattle`'s white: every palette word `$EEE`.
pub const ENTRY_WHITE: u16 = 0x0EEE;

/// The entry of one event battle, while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventBattleEntry {
    index: u16,
    /// Frames since the request frame.
    elapsed: u16,
}

impl EventBattleEntry {
    const fn new(index: u16) -> Self {
        Self { index, elapsed: 0 }
    }

    /// The field arm's frames: one per band pass or tone frame.
    const fn field_frames(self) -> u16 {
        if self.index == PROFOUND_DARKNESS {
            DARKNESS_TONE_FRAMES
        } else {
            BAND_PASSES
        }
    }

    /// The counted frames before the loads, for this arm.
    const fn before_loads(self) -> u16 {
        BEFORE_LOADS - BAND_PASSES + self.field_frames()
    }

    /// The first frame of the `$EEE` frame and the fade.
    const fn fade_from(self) -> u16 {
        self.before_loads() + LOAD_LAG + 1
    }

    /// The frame the battle starts and rolls.
    const fn roll_frame(self) -> u16 {
        self.fade_from() + FADE_COUNTED + ROLL_LAG - 1
    }

    /// Whether frame `elapsed` advances `Main_Frame_Count` and the seed.
    const fn counted(self, elapsed: u16) -> bool {
        elapsed <= self.before_loads()
            || (elapsed >= self.fade_from() && elapsed < self.fade_from() + FADE_COUNTED)
    }

    /// The entry's backdrop CRAM word. Before setup it covers the field;
    /// once the stage is loaded it is also the cleared VRAM's colour.
    fn screen(self, elapsed: u16) -> Option<u16> {
        let white_from = self.field_frames() - BAND_PASSES + WHITE_FROM;
        if elapsed < white_from {
            return None;
        }
        let fade_from = self.fade_from();
        if elapsed <= fade_from {
            return Some(ENTRY_WHITE);
        }
        Some(
            crate::BattleLoadView::new(
                crate::session::BattleStart::EventBattle(self.index),
                elapsed - fade_from,
            )
            .palette_word(0),
        )
    }
}

impl Runtime {
    /// A scene asked for event battle `index`: the entry runs before it starts.
    pub(crate) fn begin_event_battle_entry(&mut self, index: u16) -> RuntimeEvent {
        self.event_battle_entry = Some(EventBattleEntry::new(index));
        RuntimeEvent::EventBattleEntered { index }
    }

    /// One frame of an event battle's entry, when one runs: the field and the
    /// scene are parked, a counted frame steps `Main_Frame_Count` and the seed,
    /// and the roll frame starts the battle.
    pub(crate) fn tick_event_battle_entry(&mut self) -> Option<Vec<RuntimeEvent>> {
        let entry = self.event_battle_entry.as_mut()?;
        entry.elapsed += 1;
        let entry = *entry;
        if entry.counted(entry.elapsed) {
            self.frames = self.frames.wrapping_add(1);
            self.rng.step();
        }
        if entry.elapsed == entry.fade_from() {
            let start = crate::session::BattleStart::EventBattle(entry.index);
            self.prepared_battle = self
                .prepare_battle_load(start, self.battle_party())
                .ok()
                .map(|battle| (start, battle));
            if self.prepared_battle.is_some() {
                self.effects.cram.enable_display();
            }
        }
        if entry.elapsed < entry.roll_frame() {
            return Some(Vec::new());
        }
        self.event_battle_entry = None;
        Some(vec![self.start_requested_battle(entry.index)])
    }

    /// The stage is loaded before the fade and stays settled through the lag
    /// frames before `loc_B62A`; none of these frames rolls the opening.
    #[must_use]
    pub fn battle_load_view(&self) -> Option<crate::BattleLoadView> {
        let entry = self.event_battle_entry?;
        let steps = entry.elapsed.checked_sub(entry.fade_from())?;
        self.prepared_battle.as_ref()?;
        Some(crate::BattleLoadView::new(
            crate::session::BattleStart::EventBattle(entry.index),
            steps,
        ))
    }

    /// The uniform screen an event battle's entry shows this frame, as a CRAM
    /// word: the white hold and the fade from white. `None` when no entry runs
    /// or while its field arm draws.
    #[must_use]
    pub fn event_battle_entry_screen(&self) -> Option<u16> {
        let entry = self.event_battle_entry?;
        entry.screen(entry.elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(index: u16) -> Vec<(u16, bool, Option<u16>)> {
        let mut entry = EventBattleEntry::new(index);
        let mut out = Vec::new();
        while entry.elapsed < entry.roll_frame() {
            entry.elapsed += 1;
            out.push((
                entry.elapsed,
                entry.counted(entry.elapsed),
                entry.screen(entry.elapsed),
            ));
        }
        out
    }

    /// Tape 45, from the C on 7799: the roll on 7875 (76 frames on), 36 of
    /// them counted (`Main_Frame_Count` 7282 to 7318), counted 7800-7817 and
    /// 7855-7872.
    #[test]
    fn battle_7s_entry_counts_the_tapes_frames() {
        let frames = frames(7);
        assert_eq!(frames.len(), 76, "the roll frame is 7875");
        let counted: Vec<u16> = frames.iter().filter(|f| f.1).map(|f| f.0).collect();
        assert_eq!(counted.len(), 36);
        let expected: Vec<u16> = (1..=18).chain(56..=73).collect();
        assert_eq!(counted, expected);
    }

    /// The screen: the field arm's frames draw nothing here, white from 7814
    /// through 7856, then a nibble a frame: 7857 and 7858 show `$CCC` (the
    /// odd nibble is not displayed), 7869 and on black.
    #[test]
    fn battle_7s_entry_screen_is_the_hold_then_the_fade() {
        let screen: Vec<Option<u16>> = frames(7).iter().map(|f| f.2).collect();
        let at = |frame: u16| screen[usize::from(frame - 7801)];
        assert_eq!(at(7813), None, "the band's frames are #665's");
        assert!((7814..=7856).all(|frame| at(frame) == Some(ENTRY_WHITE)));
        assert_eq!(at(7857), Some(0xDDD));
        assert_eq!(at(7858), Some(0xCCC));
        assert_eq!(at(7863), Some(0x777));
        assert_eq!(at(7870), Some(0));
        assert!((7870..=7875).all(|frame| at(frame) == Some(0)));
        // Negative control: the screens are the displayed levels of the
        // tape's frames (205, 172, 139, 98, 65, 32, 0 red), not an alpha ramp.
        let red = |word: u16| psiv_data::color::cram_rgb(word)[0];
        let reds: Vec<u8> = (7857..=7870).map(|f| red(at(f).unwrap())).collect();
        assert_eq!(
            reds,
            [205, 205, 172, 172, 139, 139, 98, 98, 65, 65, 32, 32, 0, 0]
        );
    }

    /// The Profound Darkness takes `loc_58482`'s forty tone frames instead of
    /// the fifteen passes; `GameMode_LoadBattle` after them is the same.
    #[test]
    fn the_profound_darkness_spends_forty_tone_frames() {
        let darkness = frames(PROFOUND_DARKNESS);
        let battle = frames(7);
        assert_eq!(darkness.len(), battle.len() + 25);
        assert_eq!(darkness.iter().filter(|f| f.1).count(), 36 + 25);
        assert_eq!(darkness[..38].iter().filter(|f| f.2.is_some()).count(), 0);
        assert_eq!(
            &darkness[25..],
            &battle[..]
                .iter()
                .map(|f| (f.0 + 25, f.1, f.2))
                .collect::<Vec<_>>()[..]
        );
    }
}
