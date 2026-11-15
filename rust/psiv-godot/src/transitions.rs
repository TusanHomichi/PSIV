//! Retail-timed presentation transitions.
//!
//! The cartridge does not tween a transparent black layer.  Its palette
//! helpers rewrite CRAM in seven increments between the full palette and
//! black, with the visible step held for two frames.  This small state machine
//! keeps that cadence in integer frames; the Godot bridge only turns the
//! current visual into one or more `ColorRect`s.

const PALETTE_LEVELS: u8 = 7;

/// A transition requested by the field presentation seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionKind {
    /// Doorway/map-change fade-through-black: 13 out, 54 black, 13 in.
    Doorway,
    /// Encounter flash: field to white, white hold, then battle reveal.
    BattleEntry,
    /// Cutscene initialization off a palette that is not live: fade to
    /// black and hold until the scene's first presentation window is ready.
    /// On a live one the scene's `InitVRAMAndCRAM` fades the CRAM itself.
    SceneStart,
    /// The title hand-off's centered screen wipe.  The Godot field has no
    /// title plane, so this is used as a matching black-to-field reveal.
    GameStart,
    /// A field load's black rows and `Pal_FadeIn` ramp: a cutscene's field
    /// reload, a teleport's destination load, and a scene's `FadeIn` off a
    /// palette that is not live (on a live one the CRAM model draws it).
    /// The runtime owns the blocking and the rows; this is the renderer's
    /// 13-step black ramp at their end.
    SceneFadeIn,
    /// `PalFadeOut_ClrSpriteTbl`: a teleport's (`MenuExit_Teleport`) over
    /// the field being left, and a scene's `FadeOut` off a palette that is
    /// not live.
    SceneFadeOut,
    /// One frame of a uniform screen the runtime chose, as a CRAM word: an
    /// event battle's white hold and fade
    /// (`psiv_runtime::Runtime::event_battle_entry_screen`). The shell sets it
    /// again on every frame the runtime still shows it.
    Screen(u16),
}

/// The solid colour used by the current palette ramp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionColor {
    Black,
    White,
    /// The colour a CRAM word shows, opaque.
    Cram(u16),
}

/// What the renderer needs for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransitionVisual {
    pub(crate) color: TransitionColor,
    /// Quantized CRAM level, 0 = transparent and 7 = opaque.
    pub(crate) level: u8,
    /// For the game-start wipe, the fraction of the centered window that is
    /// open. `None` means one full-screen rectangle.
    pub(crate) opening: Option<f32>,
}

/// Frame counter and retail timing for one active transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transition {
    kind: TransitionKind,
    age: u16,
    frames: Option<u16>,
}

impl Transition {
    pub(crate) fn new(kind: TransitionKind) -> Self {
        Self {
            kind,
            age: 0,
            frames: None,
        }
    }

    /// Runtime-supplied display interval; this changes no simulation clock.
    pub(crate) fn with_frames(kind: TransitionKind, frames: u16) -> Self {
        Self {
            kind,
            age: 0,
            frames: Some(frames),
        }
    }

    /// Advances one renderer frame. Returns true once the measured sequence
    /// has completed.
    pub(crate) fn tick(&mut self) -> bool {
        if !self.finished() {
            self.age = self.age.saturating_add(1);
        }
        self.finished()
    }

    pub(crate) fn finished(self) -> bool {
        self.age >= self.duration()
    }

    /// Returns the stepped visual for the current age. A `None` visual means
    /// the transition is active for timing purposes but has no cover to draw
    /// (the title plane is the one such missing presentation surface).
    pub(crate) fn visual(self) -> Option<TransitionVisual> {
        match self.kind {
            TransitionKind::Doorway => self.doorway_visual(),
            TransitionKind::BattleEntry => self.battle_visual(),
            TransitionKind::SceneStart => self.scene_start_visual(),
            TransitionKind::GameStart => self.game_start_visual(),
            TransitionKind::SceneFadeIn => self.scene_fade_in_visual(),
            TransitionKind::SceneFadeOut => self.scene_fade_out_visual(),
            TransitionKind::Screen(word) => (!self.finished()).then_some(TransitionVisual {
                color: TransitionColor::Cram(word),
                level: PALETTE_LEVELS,
                opening: None,
            }),
        }
    }

    /// Scene fades sit below field windows. A uniform screen replaces every
    /// field surface, including a window whose text has already returned.
    pub(crate) fn z_index(self) -> i32 {
        match self.kind {
            TransitionKind::Screen(_) => super::dialogue::Z_INDEX + 100,
            TransitionKind::BattleEntry
            | TransitionKind::GameStart
            | TransitionKind::SceneFadeIn
            | TransitionKind::SceneFadeOut => 600,
            TransitionKind::Doorway | TransitionKind::SceneStart => 25,
        }
    }

    fn duration(self) -> u16 {
        if let Some(frames) = self.frames {
            return frames;
        }
        match self.kind {
            TransitionKind::Doorway => 80,
            TransitionKind::BattleEntry => 56,
            TransitionKind::SceneStart => 37,
            TransitionKind::GameStart => 18,
            TransitionKind::SceneFadeIn | TransitionKind::SceneFadeOut => 14,
            TransitionKind::Screen(_) => 1,
        }
    }

    fn doorway_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let level = match self.age {
            0 => return None,
            1..=13 => step_up(self.age),
            14..=67 => PALETTE_LEVELS,
            68..=79 => step_down(self.age - 67),
            _ => return None,
        };
        Some(black(level))
    }

    fn battle_visual(self) -> Option<TransitionVisual> {
        if self.finished() || self.age == 0 {
            return None;
        }
        let visual = match self.age {
            1..=14 => white(step_up(self.age)),
            15..=41 => white(PALETTE_LEVELS),
            42..=55 => white(step_down(self.age - 41)),
            _ => return None,
        };
        Some(visual)
    }

    fn scene_start_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let level = match self.age {
            0 => PALETTE_LEVELS,
            1..=13 => step_up(self.age),
            14..=36 => PALETTE_LEVELS,
            _ => return None,
        };
        Some(black(level))
    }

    fn game_start_visual(self) -> Option<TransitionVisual> {
        if self.finished() || self.age >= 8 {
            return None;
        }
        Some(TransitionVisual {
            color: TransitionColor::Black,
            level: PALETTE_LEVELS,
            opening: Some(f32::from(self.age) / 8.0),
        })
    }

    fn scene_fade_in_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        let display_enable = self.duration().saturating_sub(14);
        Some(black(PALETTE_LEVELS.saturating_sub(step_up(
            self.age.saturating_sub(display_enable),
        ))))
    }

    fn scene_fade_out_visual(self) -> Option<TransitionVisual> {
        if self.finished() {
            return None;
        }
        Some(black(step_up(self.age)))
    }
}

fn black(level: u8) -> TransitionVisual {
    TransitionVisual {
        color: TransitionColor::Black,
        level,
        opening: None,
    }
}

fn white(level: u8) -> TransitionVisual {
    TransitionVisual {
        color: TransitionColor::White,
        level,
        opening: None,
    }
}

/// Retail holds each palette level for two frames. The first changed frame is
/// level one; the 13th frame reaches level seven (black/full white).
fn step_up(frame: u16) -> u8 {
    (frame.div_ceil(2) as u8).min(PALETTE_LEVELS)
}

fn step_down(frame: u16) -> u8 {
    PALETTE_LEVELS.saturating_sub(step_up(frame))
}

/// The cover left once a runtime-owned palette fade-in
/// (`VariablePaletteFade` or `TwoLinePaletteFade` up) starts. A scene's
/// `LoadMap` emits `MapChanged`, which starts the 80-frame doorway cover;
/// the fade that follows reveals the map from black through the CRAM
/// itself, so the doorway's hold would mask it. Every other cover stays.
pub(crate) fn after_palette_fade_in(cover: Option<Transition>) -> Option<Transition> {
    cover.filter(|transition| transition.kind != TransitionKind::Doorway)
}

#[cfg(test)]
mod tests {
    use super::{Transition, TransitionColor, TransitionKind, after_palette_fade_in};

    /// A runtime fade-in ends a doorway cover at any age, and leaves the
    /// other covers, and no cover, as they were.
    #[test]
    fn a_palette_fade_in_ends_only_the_doorway_cover() {
        for age in [0, 1, 13, 40, 70] {
            let mut doorway = Transition::new(TransitionKind::Doorway);
            age_to(&mut doorway, age);
            assert_eq!(after_palette_fade_in(Some(doorway)), None, "age {age}");
        }
        for kind in [
            TransitionKind::BattleEntry,
            TransitionKind::SceneStart,
            TransitionKind::GameStart,
            TransitionKind::SceneFadeIn,
            TransitionKind::SceneFadeOut,
        ] {
            let mut cover = Transition::new(kind);
            age_to(&mut cover, 3);
            assert_eq!(after_palette_fade_in(Some(cover)), Some(cover), "{kind:?}");
        }
        assert_eq!(after_palette_fade_in(None), None);
    }

    fn age_to(transition: &mut Transition, age: u16) {
        for _ in 0..age {
            transition.tick();
        }
    }

    #[test]
    fn doorway_matches_thirteen_fade_hold_and_thirteen_fade_in() {
        let mut transition = Transition::new(TransitionKind::Doorway);
        assert!(transition.visual().is_none());

        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 1);
        age_to(&mut transition, 12);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 54);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 6);
        age_to(&mut transition, 11);
        assert_eq!(transition.visual().unwrap().level, 1);
        age_to(&mut transition, 1);
        assert!(transition.finished());
        assert!(transition.visual().is_none());
    }

    #[test]
    fn battle_is_white_flash_hold_then_white_reveal() {
        let mut transition = Transition::new(TransitionKind::BattleEntry);
        age_to(&mut transition, 1);
        let first = transition.visual().unwrap();
        assert_eq!(first.color, TransitionColor::White);
        assert_eq!(first.level, 1);
        age_to(&mut transition, 13);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 27);
        assert_eq!(transition.visual().unwrap().level, 7);
        age_to(&mut transition, 1);
        assert_eq!(transition.visual().unwrap().level, 6);
        age_to(&mut transition, 13);
        assert_eq!(transition.visual().unwrap().level, 0);
        age_to(&mut transition, 1);
        assert!(transition.finished());
    }

    #[test]
    fn scene_edges_match_the_observed_black_ramp() {
        let mut start = Transition::new(TransitionKind::SceneStart);
        assert_eq!(start.visual().unwrap().level, 7);
        age_to(&mut start, 1);
        assert_eq!(start.visual().unwrap().level, 1);
        age_to(&mut start, 12);
        assert_eq!(start.visual().unwrap().level, 7);
        age_to(&mut start, 24);
        assert!(start.finished());
    }

    /// A cutscene's return blacks the screen out once: the runtime's
    /// `FieldReload { setup, fade }` is one `SceneFadeIn` of `setup + fade`
    /// frames. The principal's retail return is 55 mode-8 frames, black
    /// through 10734 and a 13-frame ramp to full at 10747 (docs/field/
    /// TRANSITIONS_DECODED.md); the fade's 16 frames end in that ramp.
    #[test]
    fn a_cutscene_return_is_one_black_hold_then_one_ramp() {
        let mut ret = Transition::with_frames(TransitionKind::SceneFadeIn, 39 + 16);
        let mut levels = vec![ret.visual().unwrap().level];
        while !ret.tick() {
            levels.push(ret.visual().map_or(0, |visual| visual.level));
        }
        assert_eq!(levels.len(), 55, "the return spends the measured frames");
        assert!(levels[..42].iter().all(|&level| level == 7), "black hold");
        assert_eq!(levels[42], 6, "first ramp step");
        assert_eq!(levels[54], 0, "clear on the last frame");
        let ramp = &levels[42..];
        assert!(ramp.windows(2).all(|pair| pair[1] <= pair[0]), "{ramp:?}");
        let darkest_again = levels[43..].iter().position(|&level| level == 7);
        assert_eq!(
            darkest_again, None,
            "no second blackout after the ramp starts"
        );
    }

    /// Tape 36's teleport to MILE, frame by frame from 8390 (the frame the
    /// menu exits, after the last window closed): the field being left steps
    /// every two frames to black at 8403 (`SceneFadeOut`, on
    /// `RuntimeEvent::TeleportFadeOut`), the overworld loads on 8404 and its
    /// 61 rows hold black, ramp from 8452 and clear at 8464 (`SceneFadeIn` of
    /// the rows, on `RuntimeEvent::Teleported`); control returns at 8465.
    #[test]
    fn a_teleport_fades_out_then_spends_the_destinations_load_rows() {
        let mut levels = Vec::new();
        let mut fade_out = Transition::new(TransitionKind::SceneFadeOut);
        let mut load = None;
        for frame in 8390..8465 {
            // The shell ticks the cover before the frame's events start one.
            if frame > 8390 {
                if let Some(load) = load.as_mut() {
                    Transition::tick(load);
                } else {
                    fade_out.tick();
                }
            }
            if frame == 8404 {
                assert!(fade_out.finished(), "the fade-out is over at the load");
                load = Some(Transition::with_frames(TransitionKind::SceneFadeIn, 61));
            }
            let current = load.unwrap_or(fade_out);
            levels.push(current.visual().map_or(0, |visual| visual.level));
        }
        let at = |frame: usize| levels[frame - 8390];
        assert_eq!(at(8390), 0);
        assert_eq!((at(8391), at(8392), at(8393)), (1, 1, 2));
        assert_eq!(at(8403), 7, "black");
        assert!(
            (8403..=8451).all(|frame| at(frame) == 7),
            "the load's black rows"
        );
        assert_eq!((at(8452), at(8453), at(8454)), (6, 6, 5));
        assert_eq!(at(8464), 0, "clear on the load's last row");
        let mut load = load.unwrap();
        assert!(load.tick(), "finished at 8465");
    }

    /// A runtime screen covers the frame it is set on, opaque in its own
    /// colour, and is gone after one tick unless the shell sets it again.
    #[test]
    fn a_screen_lasts_the_one_frame_it_is_set_on() {
        let mut screen = Transition::new(TransitionKind::Screen(0xCCC));
        let visual = screen.visual().unwrap();
        assert_eq!(visual.color, TransitionColor::Cram(0xCCC));
        assert_eq!(visual.level, 7);
        assert!(screen.z_index() > crate::dialogue::Z_INDEX);
        assert!(
            Transition::new(TransitionKind::SceneFadeOut).z_index() < crate::dialogue::Z_INDEX,
            "a scene fade keeps its existing position below windows"
        );
        assert!(screen.tick(), "finished after its frame");
        assert!(screen.visual().is_none());
    }

    #[test]
    fn game_start_opens_the_center_window_for_eight_frames() {
        let mut transition = Transition::new(TransitionKind::GameStart);
        assert_eq!(transition.visual().unwrap().opening, Some(0.0));
        age_to(&mut transition, 4);
        assert_eq!(transition.visual().unwrap().opening, Some(0.5));
        age_to(&mut transition, 4);
        assert!(transition.visual().is_none());
        assert!(!transition.finished());
        age_to(&mut transition, 10);
        assert!(transition.finished());
    }
}
