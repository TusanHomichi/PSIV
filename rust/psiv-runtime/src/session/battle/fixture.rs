//! Certification fixtures for the battle command windows.
//!
//! `PSIV_DEBUG_BATTLE_WINDOW=<spec>` starts tape 07's first battle (Alys,
//! Chaz and Hahn against two Zoran Bults, formation `$8A`) with a command
//! window already open, so a capture compares the window with the oracle's
//! frame of the same state. The battle is real: a runtime battle starts on
//! the pack's formation with the three characters' retail stats, and the
//! mode's own menu code builds the window; only the way the window got
//! open is not a pad script.
//!
//! The spec is a window name and optional `key=value` seeds, comma-separated:
//! `strip,cursor=1`, `tech,cursor=0,blink=14/1`.
//!
//! | window | opens |
//! | --- | --- |
//! | `top` | the COMD/MACR/RUN options |
//! | `strip` | the first actor's command strip |
//! | `tech`, `skill`, `item` | that list window |
//!
//! | key | meaning |
//! | --- | --- |
//! | `cursor=N` | the icon (strip) or row (list; page `N / 4`) the cursor is on |
//! | `blink=T/P` | seeds `$FFFF41D2` and `$FFFF41D4`, which an oracle receipt reads out of RAM at the compared frame: the red cursor's phase is the count of frames the cursor routines have run since the battle began, and a fixture does not replay them |
//! | `status=F/B` | seats fighter slot `F` (1 Alys, 2 Chaz, 3 Hahn) with retail status byte `B`; repeat the key for more than one member |
//! | `round=defend` | after the window is set up, runs one round in which the whole party defends, so the battle plays its real beats (a pair of Zol slugs fuse in it) before the options come back |
//! | `timeline=attack` | plays what tape 34 shows mid-round: Alys defends (three beats, to clear the entry fade), then the left Zoran Bult's attack on her runs its sixteen-frame animation (the round's other actions are the oracle's, not this fixture's); every pane shows the defend icon and only the bodies that have acted are drawn |
//! | `age=N` | frames the strip has been open when the fixture starts, because the command cursor's two mapping frames alternate on the object's clock and a capture must land on the oracle's phase |

use psiv_core::CharId;
use psiv_core::battle::{BattleEvent, Command, FighterId, RoundOrders};

use crate::events::{BattleAnimationEvent, BattleTimeline};
use crate::{Runtime, Session};

use super::super::Mode;
use super::view::MenuPage;
use super::{BattleFrame, BattleMode, BattleStart, Window, blink::CursorBlink, menu};

/// A character's retail record at tape 07's frame 25000, read from the
/// oracle's work-RAM dump at `Character_Stats` (`$FFFFF500` plus `$80` per
/// character, layout `ps4.constants.asm:7`): the fields a command window or
/// the status strip reads.
struct Record {
    character: u8,
    level: u16,
    hp: u16,
    tp: u16,
    equipment: [u8; 4],
    techniques: &'static [u8],
    skills: &'static [u8],
    uses: u8,
}

/// Alys, Chaz and Hahn, in fighter-slot order, as the oracle's RAM dump holds
/// them.
const TAPE_07_PARTY: [Record; 3] = [
    Record {
        character: 1,
        level: 7,
        hp: 53,
        tp: 40,
        equipment: [0x03, 0x00, 0x06, 0x04],
        // Newest last: the window lists them newest first (FOI learned
        // first, SANER last).
        techniques: &[0x01, 0x1E, 0x1F],
        skills: &[0x06],
        uses: 5,
    },
    Record {
        character: 0,
        level: 1,
        hp: 25,
        tp: 10,
        equipment: [0x02, 0x02, 0x05, 0x04],
        techniques: &[0x18],
        skills: &[0x1F],
        uses: 3,
    },
    Record {
        character: 2,
        level: 1,
        hp: 21,
        tp: 25,
        equipment: [0x01, 0x0A, 0x07, 0x04],
        techniques: &[0x18, 0x14],
        skills: &[0x2F],
        uses: 5,
    },
];

impl Runtime {
    /// Seats tape 07's party: Alys in fighter slot 1 (the center), Chaz in 2,
    /// Hahn in 3, each with the record the oracle held at frame 25000.
    fn seat_tape_07_party(&mut self, statuses: &[(u8, u8)]) -> Result<(), String> {
        let order: Vec<Option<CharId>> = TAPE_07_PARTY
            .iter()
            .map(|record| Some(CharId(record.character)))
            .chain([None, None])
            .collect();
        self.game
            .set_party([order[0], order[1], order[2], None, None]);
        for (index, record) in TAPE_07_PARTY.iter().enumerate() {
            let stats = self
                .game
                .roster_mut()
                .get_mut(CharId(record.character))
                .ok_or_else(|| format!("character {} is not seated", record.character))?;
            stats.level = record.level;
            stats.curr_hp = record.hp;
            stats.max_hp = record.hp;
            stats.curr_tp = record.tp;
            stats.max_tp = record.tp;
            stats.status = statuses
                .iter()
                .find(|(fighter, _)| usize::from(*fighter) == index + 1)
                .map_or(0, |(_, byte)| *byte);
            stats.equipment = record.equipment;
            stats.techniques = [0; 16];
            stats.techniques[..record.techniques.len()].copy_from_slice(record.techniques);
            stats.skills = [0; 8];
            stats.curr_skill_uses = [0; 8];
            stats.max_skill_uses = [0; 8];
            for (slot, id) in record.skills.iter().enumerate() {
                stats.skills[slot] = *id;
                stats.curr_skill_uses[slot] = record.uses;
                stats.max_skill_uses[slot] = record.uses;
            }
        }
        Ok(())
    }
}

/// What a fixture spec asks for.
struct Spec {
    window: Option<MenuPage>,
    strip: bool,
    cursor: usize,
    blink: Option<(i16, u16)>,
    age: u32,
    statuses: Vec<(u8, u8)>,
    defend_round: bool,
    attack_timeline: bool,
}

fn parse(spec: &str) -> Result<Spec, String> {
    let mut parts = spec.split(',');
    let kind = parts.next().unwrap_or_default().trim();
    let (window, strip) = match kind {
        "top" => (None, false),
        "strip" => (None, true),
        "tech" => (Some(MenuPage::Techniques), false),
        "skill" => (Some(MenuPage::Skills), false),
        "item" => (Some(MenuPage::Items), false),
        other => return Err(format!("unknown battle window {other:?}")),
    };
    let mut parsed = Spec {
        window,
        strip,
        cursor: 0,
        blink: None,
        age: 0,
        statuses: Vec::new(),
        defend_round: false,
        attack_timeline: false,
    };
    for part in parts {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| format!("{part:?} is not key=value"))?;
        let number = |text: &str| {
            text.trim()
                .parse::<i64>()
                .map_err(|_| format!("{key} value {text:?} is not a number"))
        };
        match key.trim() {
            "cursor" => parsed.cursor = number(value)?.max(0) as usize,
            "age" => parsed.age = number(value)?.max(0) as u32,
            "blink" => {
                let (timer, phase) = value
                    .split_once('/')
                    .ok_or_else(|| "blink wants timer/phase".to_string())?;
                parsed.blink = Some((number(timer)? as i16, number(phase)? as u16));
            }
            "timeline" => match value.trim() {
                "attack" => parsed.attack_timeline = true,
                other => return Err(format!("unknown timeline {other:?}")),
            },
            "round" => match value.trim() {
                "defend" => parsed.defend_round = true,
                other => return Err(format!("unknown round {other:?}")),
            },
            "status" => {
                let (fighter, byte) = value
                    .split_once('/')
                    .ok_or_else(|| "status wants fighter/byte".to_string())?;
                parsed
                    .statuses
                    .push((number(fighter)? as u8, number(byte)? as u8));
            }
            other => return Err(format!("unknown fixture key {other:?}")),
        }
    }
    Ok(parsed)
}

impl Session {
    /// Debug-selector family: starts tape 07's battle on `formation` with the
    /// command window `spec` names open (`PSIV_DEBUG_BATTLE_WINDOW`).
    ///
    /// The battle opens with no beats queued, so the options are up at once,
    /// like the command-idle capture `PSIV_DEBUG_BATTLE=0x88` draws.
    pub fn debug_battle_window(&mut self, formation: u16, spec: &str) -> BattleFrame {
        let refuse = |message: String| BattleFrame {
            view: None,
            started: None,
            fault: Some(format!("debug battle window {spec:?} refused: {message}")),
        };
        let spec = match parse(spec) {
            Ok(spec) => spec,
            Err(error) => return refuse(error),
        };
        if let Err(error) = self.runtime.seat_tape_07_party(&spec.statuses) {
            return refuse(error);
        }
        let party = self.runtime.battle_party();
        if let Err(error) = self.runtime.start_battle(formation, party) {
            return refuse(error.to_string());
        }
        let mut mode = BattleMode::begin(
            &self.runtime,
            BattleTimeline {
                events: Vec::new(),
                sounds: Vec::new(),
                animations: Vec::new(),
            },
            BattleStart::Encounter(formation),
        );
        mode.debug_open(&self.runtime, &spec);
        if spec.attack_timeline {
            mode.debug_attack_beats();
        }
        if spec.defend_round {
            let orders = RoundOrders::Commands(vec![Command::Defend; 5]);
            mode.run_round(&mut self.runtime, &orders);
        }
        let frame = mode.start_frame(&self.runtime);
        self.mode = Mode::Battle(Box::new(mode));
        frame
    }
}

impl BattleMode {
    /// Queues tape 34's two beats: Alys defends and the left Zoran Bult
    /// attacks her, with the attack's decoded animation on the second.
    fn debug_attack_beats(&mut self) {
        let fighter = |id| FighterId::new(id).expect("fixture fighter id");
        let (alys, zoran) = (fighter(1), fighter(6));
        // The command phase is over: every pane shows DEFEND and the party
        // rows are clear until a body acts (`ps4.asm:8419`, `2099`).
        self.command_bytes = [5, 5, 5, 0, 0];
        self.acted = Some(Vec::new());
        self.enqueue(BattleTimeline {
            // Two lead-in defend beats keep the attack's first frame past
            // the shell's battle-entry fade, so a capture can see it.
            events: vec![
                BattleEvent::Defended { actor: alys },
                BattleEvent::Defended { actor: alys },
                BattleEvent::Defended { actor: alys },
                BattleEvent::Attacked {
                    actor: zoran,
                    targets: vec![alys],
                },
            ],
            sounds: Vec::new(),
            animations: vec![BattleAnimationEvent {
                event_index: 3,
                actor: zoran,
                enemy_id: 10,
                sfx_id: 0xD8,
                frame_duration: Some(2),
                frame_count: Some(8),
                total_frames: Some(16),
                frame_durations: Some(vec![2; 8]),
                movement_proven: true,
                sprite_sheet_proven: true,
                flash_timing_proven: true,
            }],
        });
        self.refresh_panes();
    }

    /// Opens the window a fixture spec names, on the frame the battle began.
    fn debug_open(&mut self, runtime: &Runtime, spec: &Spec) {
        if let Some((timer, phase)) = spec.blink {
            self.blink = CursorBlink::from_timers(timer, phase);
        }
        if !spec.strip && spec.window.is_none() {
            return;
        }
        let mut commands = menu::CommandsMenu::with_command_bytes(runtime, self.command_bytes);
        commands.debug_open(
            spec.window.unwrap_or(MenuPage::Actions),
            spec.cursor,
            spec.age,
        );
        self.command_bytes = commands.command_bytes();
        self.window = Some(Window::Commands(Box::new(commands)));
        self.refresh_panes();
    }
}
