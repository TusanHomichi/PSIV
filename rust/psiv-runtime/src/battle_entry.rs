//! Shared battle entry before the opening roll (`GameMode_LoadBattle`,
//! `ps4.asm:9911`). The field arm runs first, then the stage loads under `$EEE`,
//! `loc_6B20` fades from white, and `loc_B62A` rolls the opening priority.
//!
//! Tape 45's event battle 7 requests on 7799, counts 7800-7817, lags
//! 7818-7854, counts 7855-7872 and rolls on the third lag frame, 7875.
//! Tape 07's encounter fires on 24777, counts 24778-24796, lags 24797-24819,
//! counts 24820-24837 and rolls on the third lag frame, 24840. The two
//! pre-load profiles are measured independently: encounters enter through
//! `FieldRoutine_Battle`, scenes through `RunEventBattle`. Both stop the
//! music and spend a `VInt_Prepare` before `FieldRoutine_Battle2`.
//!
//! Stage decompression time varies. These profiles use tape 45's 37 load-lag
//! frames for event battles and tape 07's 23 for encounters (source notes).

use crate::{Runtime, RuntimeEvent};

/// The opaque plane-A rows `loc_58498` has overwritten this frame. Pixel
/// bounds are half-open on the 320x224 retail surface; the shell only blits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventBattleBand {
    /// First covered pixel row, inclusive.
    pub top: u16,
    /// First uncovered row below the band, exclusive.
    pub bottom: u16,
    /// Solid fill colour as a Genesis CRAM word.
    pub word: u16,
}

/// Battle 7's frames, counted from the request frame: the counted frames before
/// the loads (the `RunEventBattle` frame, the fifteen passes, `loc_585C8` and
/// `GameMode_LoadBattle`'s first frame).
const BEFORE_LOADS: u16 = 18;
/// The stage's lag frames on tape 45 (7818-7854).
const EVENT_LOAD_LAG: u16 = 37;
/// Tape 07, 24797-24819.
const ENCOUNTER_LOAD_LAG: u16 = 23;
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
/// Event entry's first submitted white screen; Godot displays it next frame.
const WHITE_FROM: u16 = 14;
/// `GameMode_LoadBattle`'s white: every palette word `$EEE`.
pub const ENTRY_WHITE: u16 = 0x0EEE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Event(u16),
    Encounter(u16),
}

/// The entry of one battle, while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BattleEntry {
    kind: EntryKind,
    /// Frames since the request frame.
    elapsed: u16,
}

impl BattleEntry {
    const fn new(index: u16) -> Self {
        Self {
            kind: EntryKind::Event(index),
            elapsed: 0,
        }
    }

    const fn encounter(formation: u16) -> Self {
        Self {
            kind: EntryKind::Encounter(formation),
            elapsed: 0,
        }
    }

    const fn start(self) -> crate::session::BattleStart {
        match self.kind {
            EntryKind::Event(index) => crate::session::BattleStart::EventBattle(index),
            EntryKind::Encounter(formation) => crate::session::BattleStart::Encounter(formation),
        }
    }

    /// The field arm's frames: one per band pass or tone frame.
    const fn field_frames(self) -> u16 {
        if matches!(self.kind, EntryKind::Event(PROFOUND_DARKNESS)) {
            DARKNESS_TONE_FRAMES
        } else {
            BAND_PASSES
        }
    }

    const fn encounter_frame(self) -> u16 {
        matches!(self.kind, EntryKind::Encounter(_)) as u16
    }

    const fn load_lag(self) -> u16 {
        match self.kind {
            EntryKind::Event(_) => EVENT_LOAD_LAG,
            EntryKind::Encounter(_) => ENCOUNTER_LOAD_LAG,
        }
    }

    /// `d6 = 13`, `d5 = 2`, then `d6 -= 1`, `d5 += 2`, fifteen
    /// times (`$058498..$0584EC`). The fifteenth write extends off-screen.
    const fn band(self) -> Option<EventBattleBand> {
        if !matches!(self.kind, EntryKind::Event(index) if index != PROFOUND_DARKNESS)
            || self.elapsed == 0
            || self.elapsed > BAND_PASSES
        {
            return None;
        }
        Some(EventBattleBand {
            top: 104_u16.saturating_sub((self.elapsed - 1) * 8),
            bottom: if self.elapsed >= 14 {
                224
            } else {
                120 + (self.elapsed - 1) * 8
            },
            word: ENTRY_WHITE,
        })
    }

    /// The counted frames before the loads, for this arm.
    const fn before_loads(self) -> u16 {
        BEFORE_LOADS - BAND_PASSES + self.field_frames() + self.encounter_frame()
    }

    /// The first frame of the `$EEE` frame and the fade.
    const fn fade_from(self) -> u16 {
        self.before_loads() + self.load_lag() + 1
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
        let white_from =
            self.field_frames() - BAND_PASSES + WHITE_FROM + 2 * self.encounter_frame();
        if elapsed < white_from {
            return None;
        }
        let fade_from = self.fade_from();
        if elapsed <= fade_from {
            return Some(ENTRY_WHITE);
        }
        Some(crate::BattleLoadView::new(self.start(), elapsed - fade_from).palette_word(0))
    }
}

impl Runtime {
    /// A scene asked for event battle `index`: the entry runs before it starts.
    pub(crate) fn begin_battle_entry(&mut self, index: u16) -> RuntimeEvent {
        self.battle_entry = Some(BattleEntry::new(index));
        RuntimeEvent::EventBattleEntered { index }
    }

    /// An encounter fired: retain its formation while the entry runs.
    pub(crate) fn begin_encounter_entry(&mut self, formation: u16) -> RuntimeEvent {
        self.battle_entry = Some(BattleEntry::encounter(formation));
        RuntimeEvent::EncounterRolled { formation }
    }

    /// One frame of a battle's entry, when one runs: the field and the
    /// scene are parked, a counted frame steps `Main_Frame_Count` and the seed,
    /// and the roll frame starts the battle.
    pub(crate) fn tick_battle_entry(&mut self) -> Option<Vec<RuntimeEvent>> {
        let entry = self.battle_entry.as_mut()?;
        entry.elapsed += 1;
        let entry = *entry;
        if matches!(entry.kind, EntryKind::Event(_))
            && (1..=entry.field_frames()).contains(&entry.elapsed)
        {
            // The loop calls the palette helper before its DMA/VInt wait.
            self.effects.cram.increase_tone(self.frames);
        }
        if let Some(band) = entry.band() {
            self.dialogue.cover_held_page(band.top, band.bottom);
        }
        if entry.counted(entry.elapsed) {
            self.frames = self.frames.wrapping_add(1);
            self.rng.step();
        }
        if entry.elapsed == entry.fade_from() {
            let start = entry.start();
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
        self.battle_entry = None;
        Some(vec![match entry.kind {
            EntryKind::Event(index) => self.start_requested_battle(index),
            EntryKind::Encounter(formation) => RuntimeEvent::EncounterBattleReady { formation },
        }])
    }

    /// Whether an entry owns the frame, including the field arm before white.
    #[must_use]
    pub fn battle_entering(&self) -> bool {
        self.battle_entry.is_some()
    }

    /// The stage is loaded before the fade and stays settled through the lag
    /// frames before `loc_B62A`; none of these frames rolls the opening.
    #[must_use]
    pub fn battle_load_view(&self) -> Option<crate::BattleLoadView> {
        let entry = self.battle_entry?;
        let steps = entry.elapsed.checked_sub(entry.fade_from())?;
        let (_, prepared) = self.prepared_battle.as_ref()?;
        Some(crate::BattleLoadView::new(
            entry.start(),
            steps,
            prepared.present_enemies(),
        ))
    }

    /// The field arm's current plane-A band, over the field and its windows.
    #[must_use]
    pub fn event_battle_entry_band(&self) -> Option<EventBattleBand> {
        self.battle_entry?.band()
    }

    /// The uniform screen a battle's entry shows this frame, as a CRAM
    /// word: the white hold and the fade from white. `None` when no entry runs
    /// or while its field arm draws.
    #[must_use]
    pub fn battle_entry_screen(&self) -> Option<u16> {
        let entry = self.battle_entry?;
        entry.screen(entry.elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(index: u16) -> Vec<(u16, bool, Option<u16>)> {
        entry_frames(BattleEntry::new(index))
    }

    fn entry_frames(mut entry: BattleEntry) -> Vec<(u16, bool, Option<u16>)> {
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

    /// Tape 07: fire 24777, roll 24840. Counted frames are 24778-24796
    /// and 24820-24837; both lag intervals leave the frame counter alone.
    #[test]
    fn encounters_count_tape_07s_frames_before_the_roll() {
        let frames = entry_frames(BattleEntry::encounter(0x8A));
        assert_eq!(frames.len(), 63);
        let counted: Vec<u16> = frames.iter().filter(|f| f.1).map(|f| f.0).collect();
        assert_eq!(counted, (1..=19).chain(43..=60).collect::<Vec<_>>());
        // Screens are submitted one frame before Godot captures them.
        let displayed = |frame: u16| frames[usize::from(frame - 24779)].2;
        assert_eq!(displayed(24793), None);
        assert!((24794..=24821).all(|f| displayed(f) == Some(ENTRY_WHITE)));
        assert_eq!(displayed(24822), Some(0xDDD));
        assert_eq!(displayed(24823), Some(0xCCC));
        assert_eq!(displayed(24824), Some(0xBBB));
        assert_eq!(displayed(24835), Some(0));
    }

    #[test]
    fn the_event_arm_writes_fifteen_expanding_bands_but_darkness_only_tones() {
        for index in 0..=0x1A {
            let mut entry = BattleEntry::new(index);
            assert_eq!(entry.band(), None, "the request frame keeps the page");
            for pass in 1..=16 {
                entry.elapsed = pass;
                let expected = if index == PROFOUND_DARKNESS || pass == 16 {
                    None
                } else {
                    // Source register trace, in pixel rows, clipped to the
                    // visible surface (the 15th write extends past it).
                    let rows = [
                        (104, 120),
                        (96, 128),
                        (88, 136),
                        (80, 144),
                        (72, 152),
                        (64, 160),
                        (56, 168),
                        (48, 176),
                        (40, 184),
                        (32, 192),
                        (24, 200),
                        (16, 208),
                        (8, 216),
                        (0, 224),
                        (0, 224),
                    ];
                    let (top, bottom) = rows[usize::from(pass - 1)];
                    Some(EventBattleBand {
                        top,
                        bottom,
                        word: 0xEEE,
                    })
                };
                assert_eq!(entry.band(), expected, "battle {index}, pass {pass}");
            }
        }
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
        assert_eq!(at(7813), None, "the field arm is still drawing its band");
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
