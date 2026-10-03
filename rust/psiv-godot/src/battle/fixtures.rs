//! The debug selectors' battle fixtures.
//!
//! `PSIV_DEBUG_BATTLE=0x88` draws tape 07's command-idle receipt and `0x89`
//! plays the newly-exact Worker Pod probe. Both are fixtures the shell owns:
//! the first has no runtime battle behind it at all, and the second runs a real
//! battle whose opening timeline the probe replaces. Split out of
//! `battle/mod.rs` with the S3 node so each file stays reviewable.

use godot::prelude::*;

use psiv_runtime::{BattleView, EnemyStatus, PartyStatus};

use super::{BattleSetup, EnemyPlacement, Field, PartyPlacement, build_setup};

/// Elapsed overlay-clock ticks that align the command-idle debug fixture with
/// the frame-25000 VDP receipt at the tick-200 screenshot. The captured
/// viewport texture always shows the previous render, so the pin accounts
/// for the one update the screenshot never sees: phase 20 plus the 170
/// rendered advances lands every piece on the receipt tuple (2,2,1) —
/// verified against the decoded plane/VRAM cells (piece0 frame 2 matches
/// the receipt 16x16 with zero RGB mismatches). Normal formations use
/// phase zero and therefore retain their ordinary first-frame behavior.
const ORACLE_ENEMY_PHASE_TICKS: usize = 20;

/// Tape 07 RAM receipt at frame 25000 (`0x41F0`) carries two Zoran Bults at
/// positions `$0E` and `$1A`. Their generated art record has half-width 3,
/// producing the decoded six-cell body runs at columns 11 and 23.
const ORACLE_ENEMY_POSITIONS: [u8; 2] = [0x0E, 0x1A];

impl Field {
    /// Presents the command-idle oracle fixture used by the visual loop.
    /// `PSIV_DEBUG_BATTLE=0x88` is intentionally a capture selector, not a
    /// raw formation id: the retail frame is tape 07's post-opening party
    /// (Chaz/Alys/Hahn) against two Zoran Bults on the Academy Basement art.
    /// The RAM receipt has no live animation event at the settled shot.
    /// No runtime battle is started, so this path cannot mutate game state:
    /// the fixture hands the node a static view.
    pub(crate) fn start_oracle_debug_battle(&mut self) {
        let setup = BattleSetup {
            map_id: 0x17,
            event_battle: Some(0),
            motavia_terrain: None,
            vehicle_index: None,
            vehicle_png: None,
            vehicle_frame: None,
            dark_force_2: false,
            enemy_animation_phase_ticks: ORACLE_ENEMY_PHASE_TICKS,
            party: vec![
                PartyPlacement {
                    // Tape 07's party is ordered Chaz/Alys/Hahn in the
                    // status strip, but the retail fighter slots are the
                    // independent center-out layout: Alys is slot 1 at the
                    // center, Chaz slot 2 at the left, Hahn slot 3 at the
                    // right.  Keep the character ids attached to those
                    // receipt-backed slots rather than the status order.
                    fighter_id: 2,
                    character: 0,
                    name: "Chaz".into(),
                    hp: 25,
                    tp: 10,
                },
                PartyPlacement {
                    fighter_id: 1,
                    character: 1,
                    name: "Alys".into(),
                    hp: 53,
                    tp: 40,
                },
                PartyPlacement {
                    fighter_id: 3,
                    character: 2,
                    name: "Hahn".into(),
                    hp: 21,
                    tp: 25,
                },
            ],
            enemies: vec![
                EnemyPlacement {
                    fighter_id: 6,
                    enemy_id: 10,
                    position: ORACLE_ENEMY_POSITIONS[0],
                    name: "ZORAN BULT".into(),
                },
                EnemyPlacement {
                    fighter_id: 7,
                    enemy_id: 10,
                    position: ORACLE_ENEMY_POSITIONS[1],
                    name: "ZORAN BULT".into(),
                },
            ],
        };
        let party = setup
            .party
            .iter()
            .map(|member| PartyStatus {
                fighter: member.fighter_id,
                name: member.name.clone(),
                hp: member.hp,
                max_hp: member.hp,
                tp: member.tp,
                status: 0,
            })
            .collect();
        let enemies = setup
            .enemies
            .iter()
            .map(|enemy| EnemyStatus {
                fighter: enemy.fighter_id,
                name: enemy.name.clone(),
                visible: true,
                enemy_id: enemy.enemy_id,
                position: None,
            })
            .collect();
        self.begin_battle_presentation(setup, "oracle tape-07 command idle receipt");
        self.set_battle_view(BattleView::command_idle(party, enemies));
    }

    /// Presents real formation `$0F7` with the newly exact Worker Pod attack
    /// injected as an ordered probe.  The formation and enemy placement come
    /// from the pack; only the command result is deterministic debug tape.
    ///
    /// The probe runs through the session's debug seam, so the beats are the
    /// battle mode's own; `BattleTimeline::debug_newly_exact_probe` supplies
    /// the events in place of the round the engine would have resolved.
    pub(crate) fn start_newly_exact_debug_battle(&mut self) {
        const FORMATION: u16 = 0x00F7;
        let setup = {
            let Some(files) = self.battle_files.as_ref() else {
                godot_error!("newly-exact debug battle needs battle files");
                return;
            };
            let Some(runtime) = self.runtime() else {
                godot_error!("newly-exact debug battle needs a runtime");
                return;
            };
            let Some(mut setup) = build_setup(files, runtime, FORMATION) else {
                return;
            };
            // Keep the live proof on the already verified Academy battle
            // background. The enemy identities and positions still come from
            // formation $0F7; this only avoids making the screenshot depend on
            // the current field map's random-battle background binding.
            setup.map_id = 0x17;
            setup.event_battle = Some(0);
            setup
        };
        let enemy_specs: Vec<_> = setup
            .enemies
            .iter()
            .map(|enemy| (enemy.fighter_id, enemy.enemy_id))
            .collect();
        let timeline = psiv_runtime::BattleTimeline::debug_newly_exact_probe(&enemy_specs);
        if timeline.animations.is_empty() {
            godot_error!("formation {FORMATION:#05x} has no newly-exact debug member");
            return;
        }
        let Some(frame) = self
            .session
            .as_mut()
            .map(|session| session.debug_battle_probe(FORMATION, timeline))
        else {
            return;
        };
        if let Some(fault) = &frame.fault {
            godot_error!("{fault}");
        }
        let Some(view) = frame.view else {
            return;
        };
        self.begin_battle_presentation(
            setup,
            "formation 0x0f7 newly-exact Worker Pod attack probe",
        );
        self.set_battle_view(view);
    }

    /// Presents tape 07's first battle (formation `$8A`) with the command
    /// window `spec` names open: `PSIV_DEBUG_BATTLE_WINDOW`, the certified
    /// pairs' clone side. The session builds the battle and the window
    /// (`Session::debug_battle_window`); this only picks the art for it, on the
    /// same Academy Basement binding and overlay phase as the command-idle
    /// fixture, so every pair against tape 07 shares one enemy clock.
    ///
    /// `PSIV_DEBUG_BATTLE_FORMATION` (hex, default `$8A`) and
    /// `PSIV_DEBUG_BATTLE_MAP` (hex field map id, default the Academy
    /// Basement binding) choose another formation and its background, for the
    /// pairs that need one (the Zol slugs' Passageway).
    pub(crate) fn start_window_debug_battle(&mut self, spec: &str) {
        let hex = |name: &str| {
            std::env::var(name).ok().and_then(|value| {
                u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok()
            })
        };
        let formation = hex("PSIV_DEBUG_BATTLE_FORMATION").unwrap_or(0x008A);
        let map = hex("PSIV_DEBUG_BATTLE_MAP");
        let Some(frame) = self
            .session
            .as_mut()
            .map(|session| session.debug_battle_window(formation, spec))
        else {
            return;
        };
        if let Some(fault) = &frame.fault {
            godot_error!("{fault}");
        }
        let Some(view) = frame.view else {
            return;
        };
        let setup = {
            let (Some(files), Some(runtime)) = (self.battle_files.as_ref(), self.runtime()) else {
                godot_error!("battle window fixture needs battle files and a runtime");
                return;
            };
            let Some(mut setup) = build_setup(files, runtime, formation) else {
                return;
            };
            match map {
                Some(map) => {
                    setup.map_id = map;
                    setup.event_battle = None;
                }
                None => {
                    setup.map_id = 0x17;
                    setup.event_battle = Some(0);
                }
            }
            setup.enemy_animation_phase_ticks = std::env::var("PSIV_DEBUG_BATTLE_PHASE")
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(ORACLE_ENEMY_PHASE_TICKS);
            setup
        };
        self.begin_battle_presentation(setup, "tape-07 battle with a command window open");
        self.set_battle_view(view);
    }
}
