//! The battle mode: the cartridge's battle loop, one frame at a time.
//!
//! Ported from `psiv-godot/src/battle/` with the S3 node. What lived in the
//! shell was a real game mode — `GameMode_Battle` (`ps4.asm:947`) with its own
//! routine table (`BattleRoutines` `ps4.asm:7543`, `BattleRoutines2`
//! `ps4.asm:1138`) — and it made decisions the shell could not own: which
//! actor answers, which commands are legal, when a beat ends, when the menu
//! reopens, what the battle pays and when the field takes the frame back.
//!
//! # One frame, in the shell's own order
//!
//! The shell's battle drive was already the cartridge's order, and this module
//! keeps it exactly:
//!
//! ```text
//! 1. the shared vblank/RNG tick          `Runtime::tick(Neutral)`
//! 2. the command menu                    a press edge, then a round
//! 3. beat playback                       dwell frames, confirm waits,
//!                                        the next queued event when one ends
//! 4. the battle's epilogue               the round that ended it pays out and
//!                                        its level-up pages are queued
//! 5. the presentation ends               the frame the field takes back
//! ```
//!
//! Step 1 is why a battle still ticks the runtime even though no field input
//! reaches it: `Main_Frame_Count` and the one shared seed advance in every game
//! mode, and the battle mixer reads both.
//!
//! # Dwell frames
//!
//! `$FFFFEE66` is `12 * (Battle_Speed + 1)` clamped to `0..=4`
//! (`Battle_OpenResultsMessage`'s own table), so the port's default speed 2
//! dwells 36 frames per beat — the number the oracle captures were taken at.
//!
//! # What is deliberately not here
//!
//! Presentation. This module says a beat is a `Hide` for fighter 6; the shell
//! decides that fighter 6's sprite node goes invisible. It says the frame
//! raised `Sound_Index` `$8B`; the shell plays it. Art, chrome, layout and
//! animation frames stay in `psiv-godot/src/battle/`.

mod menu;
mod narration;
mod presentation;
mod queue;
mod view;

pub use view::{
    BattleBeat, BattleStart, BattleView, BeatView, CommandMenuView, DamageView, EnemyStatus,
    MenuPage, MenuRow, MenuView, MessageKind, PartyStatus, SkillEntry, SkillSlotView, TargetKind,
    TechniqueEntry,
};

use std::collections::BTreeMap;

use psiv_core::Input;
use psiv_core::battle::{BattleEvent, FighterId, Outcome, RoundOrders, Side};

use crate::events::{BattleAnimationEvent, BattleTimeline};
use crate::{Runtime, RuntimeEvent};

use super::{Mode, Session};
use crate::pad::{Button, Pad};

/// Retail `$FFFFEE66`: `12 * (Battle_Speed + 1)`, clamped to `0..4`.
#[must_use]
pub const fn battle_dwell_frames(speed: u16) -> u16 {
    let speed = if speed > 4 { 4 } else { speed };
    12 * (speed + 1)
}

/// The dwell every beat gets at the port's default battle speed.
pub const BATTLE_DWELL_FRAMES: u16 = battle_dwell_frames(2);

/// The defeat page's dwell: `Battle_OpenDefeatedMsg` holds `$78` frames.
const DEFEAT_DWELL_FRAMES: u16 = 0x78;

/// `SFXID_Winners` (`ps4.asm:4697`): the victory jingle the results page
/// starts.
const SFX_VICTORY: u8 = 0x8B;

/// The beat being played and how far it has run.
#[derive(Debug, Clone, Copy)]
struct ActiveBeat {
    beat: BattleBeat,
    remaining: u16,
    total: u16,
    waits_for_confirm: bool,
}

/// The sub-window the command surface has open.
#[derive(Debug)]
enum Window {
    /// The per-character command window.
    Commands(Box<menu::CommandsMenu>),
    /// The mounted surface's skill window.
    VehicleSkills(menu::VehicleWindow),
}

/// What one battle frame produced.
#[derive(Debug, Default)]
pub struct BattleFrame {
    /// The view to draw, or `None` when the frame produced no battle at all —
    /// a battle that could not start, whose `fault` says why.
    pub view: Option<BattleView>,
    /// The battle this frame began, when it did.
    pub started: Option<BattleStart>,
    /// A diagnostic for the shell to log. The mode never stops for one.
    pub fault: Option<String>,
}

/// The battle loop's own state.
pub(crate) struct BattleMode {
    queue: queue::QueuedQueue,
    current: Option<ActiveBeat>,
    /// The outcome of the ending beat, until the results page is over.
    finish_outcome: Option<Outcome>,
    /// Set when the queue ran dry with an outcome: the epilogue runs on this
    /// frame.
    finish_request: Option<(Outcome, u16)>,
    reward_each: u16,
    reward_meseta: u16,
    defeated_name: Option<String>,
    message: String,
    message_kind: MessageKind,
    /// The fighter whose transient line is drawn, when it is not the default.
    transient: Option<FighterId>,
    /// Fighter id to display name, for every narration line.
    names: BTreeMap<u8, String>,
    /// Character id to display name, for the level-up and learning pages.
    character_names: BTreeMap<u8, String>,
    /// The party strip, in the event-driven order the shell kept it: each
    /// played beat updates its own numbers, not the round's final ones.
    party: Vec<PartyStatus>,
    /// The battle's enemies with the visibility the played beats decided.
    enemies: Vec<EnemyStatus>,
    /// Whether the command surface is open.
    command_open: bool,
    /// The COMD / MACR / RUN cursor (`Battle_Main_Option_Index`).
    cursor: usize,
    /// The sub-window that is open, if any.
    window: Option<Window>,
    /// Set once the epilogue has run: the presentation closes when idle.
    close_when_idle: bool,
    /// Set on the frame the presentation is over.
    close_ready: bool,
    /// This frame's retail sound cues, in the order the frame raised them.
    sounds: Vec<u8>,
    /// This frame's enemy attack animation cues.
    animations: Vec<BattleAnimationEvent>,
    /// The battle this mode began, until the first frame reports it.
    start: Option<BattleStart>,
    /// A fault raised this frame, for the shell to log.
    fault: Option<String>,
    /// Last frame's pad, so every accept and cancel is a press edge.
    previous_pad: Pad,
}

impl BattleMode {
    /// Begins a battle over `timeline`, which the runtime has already started.
    pub(crate) fn begin(runtime: &Runtime, timeline: BattleTimeline, start: BattleStart) -> Self {
        let mut mode = BattleMode {
            queue: queue::QueuedQueue::new(),
            current: None,
            finish_outcome: None,
            finish_request: None,
            reward_each: 0,
            reward_meseta: 0,
            defeated_name: None,
            message: String::new(),
            message_kind: MessageKind::None,
            transient: None,
            names: BTreeMap::new(),
            character_names: BTreeMap::new(),
            party: Vec::new(),
            enemies: Vec::new(),
            command_open: false,
            cursor: 0,
            window: None,
            close_when_idle: false,
            close_ready: false,
            sounds: Vec::new(),
            animations: Vec::new(),
            start: Some(start),
            fault: None,
            previous_pad: Pad::NEUTRAL,
        };
        mode.seat(runtime);
        mode.queue = queue::queue_timeline(timeline);
        mode.start_next_event();
        mode
    }

    /// The frame the battle began on: the state `begin` left behind, with the
    /// start cue still attached.
    pub(crate) fn start_frame(&mut self, runtime: &Runtime) -> BattleFrame {
        BattleFrame {
            view: Some(self.view(runtime, false)),
            // Taken, not copied: the frame a battle begins on is the only one
            // that reports it, whether the shell calls this and then the mode's
            // own `frame` in the same shell frame (the debug selectors) or a
            // frame later (an encounter inside the field frame).
            started: self.start.take(),
            fault: self.fault.take(),
        }
    }

    /// One battle frame.
    pub(crate) fn frame(&mut self, runtime: &mut Runtime, pad: Pad) -> BattleFrame {
        self.sounds.clear();
        self.animations.clear();
        self.fault = None;
        let previous = std::mem::replace(&mut self.previous_pad, pad);
        // Battles still consume the shared vblank/RNG stream. No field input
        // reaches `Runtime` while the battle owns the frame.
        if runtime.battle_active() {
            let _ = runtime.tick(Input::Neutral);
        }
        let orders = self.take_command(runtime, previous, pad);
        if let Some(orders) = orders {
            self.run_round(runtime, &orders);
        }
        self.advance(previous, pad);
        self.service_finish(runtime);
        let close_ready = std::mem::take(&mut self.close_ready);
        BattleFrame {
            view: Some(self.view(runtime, close_ready)),
            started: self.start.take(),
            fault: self.fault.take(),
        }
    }

    /// Reads the command surface and returns the orders it produced.
    ///
    /// The gate is the shell's own: `command_open && current.is_none() &&
    /// events.is_empty()`, plus the two finish flags, so a press during a beat
    /// cannot issue an order.
    fn take_command(&mut self, runtime: &Runtime, previous: Pad, pad: Pad) -> Option<RoundOrders> {
        if !self.command_open
            || self.current.is_some()
            || !self.queue.is_empty()
            || self.finish_outcome.is_some()
            || self.finish_request.is_some()
        {
            return None;
        }
        if let Some(mut window) = self.window.take() {
            // `Battle_VehSkills` (`ps4.asm:7463`): B leaves the mounted
            // window; the shell's own copy reset its cursor and reopened the
            // main options, which is what dropping the window does here.
            let (orders, keep) = match &mut window {
                Window::Commands(commands) => {
                    let orders = commands.input(previous, pad);
                    let keep = commands.open;
                    (orders, keep)
                }
                Window::VehicleSkills(skills) => {
                    if pad.is_pressed(previous, Button::Cancel) {
                        (None, false)
                    } else {
                        let orders = skills.input(runtime, previous, pad);
                        let keep = orders.is_none();
                        (orders, keep)
                    }
                }
            };
            if keep {
                self.window = Some(window);
            }
            if orders.is_some() {
                self.command_open = false;
            }
            return orders;
        }
        let vehicle = runtime.vehicle_active();
        match menu::top_input(&mut self.cursor, vehicle, previous, pad) {
            menu::TopChoice::Nothing => None,
            menu::TopChoice::Commands => {
                self.window = Some(Window::Commands(Box::new(menu::CommandsMenu::new(runtime))));
                None
            }
            menu::TopChoice::VehicleSkills => {
                self.window = Some(Window::VehicleSkills(menu::VehicleWindow { cursor: 0 }));
                None
            }
            menu::TopChoice::AttackAll => {
                self.command_open = false;
                Some(RoundOrders::attack_all())
            }
            menu::TopChoice::Run => {
                self.command_open = false;
                Some(RoundOrders::Run)
            }
        }
    }

    /// Resolves one round with `orders` and queues its timeline.
    fn run_round(&mut self, runtime: &mut Runtime, orders: &RoundOrders) {
        match runtime.battle_round_timeline(orders) {
            Ok(timeline) => self.enqueue(timeline),
            Err(error) => self.fail_round(&error.to_string()),
        }
    }

    /// Queues a timeline and starts its first event when nothing is playing.
    fn enqueue(&mut self, timeline: BattleTimeline) {
        self.queue.extend(queue::queue_timeline(timeline));
        if self.current.is_none() {
            self.start_next_event();
        }
    }

    /// The shell's `fail_round`: the engine refused a round, so the window
    /// reopens over an error page and the shell logs the fault.
    fn fail_round(&mut self, error: &str) {
        self.fault = Some(format!("battle round failed: {error}"));
        self.message = "Battle error!".into();
        self.message_kind = MessageKind::Wide;
        self.current = None;
        self.queue.clear();
        self.sounds.clear();
        self.finish_outcome = None;
        self.finish_request = None;
        self.command_open = true;
    }

    /// Advances one dwell frame and starts the next event when the beat ends.
    fn advance(&mut self, previous: Pad, pad: Pad) {
        if let Some(active) = self.current.as_mut() {
            if active.waits_for_confirm {
                if !menu::confirm_pressed(previous, pad) {
                    return;
                }
                active.remaining = 1;
            }
            if active.remaining > 1 {
                active.remaining -= 1;
                return;
            }
            self.current = None;
            self.start_next_event();
            return;
        }
        self.start_next_event();
    }

    /// Starts the next queued event, or ends the command phase.
    fn start_next_event(&mut self) {
        if self.current.is_some() {
            return;
        }
        let Some(queued) = self.queue.pop_front() else {
            if let Some(outcome) = self.finish_outcome.take() {
                self.finish_request = Some((outcome, self.reward_each));
                self.command_open = false;
            } else if self.close_when_idle {
                self.close_ready = true;
                self.command_open = false;
            } else {
                self.command_open = true;
                self.message.clear();
                self.message_kind = MessageKind::None;
            }
            return;
        };
        self.sounds.extend(queued.sounds);
        self.animations.extend(queued.animations);
        let event = queued.event;
        if let BattleEvent::EnemyStatsReloaded { fighter, name, .. } = &event {
            self.names.insert(fighter.get(), name.clone());
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.fighter == fighter.get()) {
                enemy.name.clone_from(name);
            }
        }
        if let BattleEvent::Started { enemies, .. } = &event {
            for enemy in &mut self.enemies {
                enemy.visible = enemies.iter().any(|id| id.get() == enemy.fighter);
            }
        }
        if let BattleEvent::EnemyReplenished { fighter, .. } = &event
            && let Some(enemy) = self.enemies.iter_mut().find(|e| e.fighter == fighter.get())
        {
            enemy.visible = true;
        }
        self.update_live_party(&event);
        let narrate = narration::narration(&event, &self.names, &self.character_names);
        if let BattleEvent::UnsupportedAbility { actor, ability } = &event {
            self.fault = Some(format!(
                "battle renderer: engine emitted unsupported ability {ability} for fighter {}",
                actor.get()
            ));
        }
        if let BattleEvent::VehicleSkillEffectUnavailable { actor, skill } = &event {
            self.fault = Some(format!(
                "battle renderer: vehicle skill {skill} for fighter {} has no effect dispatcher",
                actor.get()
            ));
        }
        if let BattleEvent::Rewarded { meseta, .. } = &event {
            self.reward_meseta = *meseta;
        }
        if let BattleEvent::Rewarded {
            experience_each, ..
        } = &event
        {
            self.reward_each = *experience_each;
        }
        self.message = narrate.line;
        if narrate.beat == BattleBeat::End(Outcome::Defeat) {
            self.message = format!(
                "{} defeated...!",
                self.defeated_name.take().unwrap_or_else(|| "Party".into())
            );
        }
        self.transient = None;
        self.message_kind = match narrate.beat {
            BattleBeat::Start => MessageKind::None,
            BattleBeat::End(Outcome::ScriptedExit) => MessageKind::None,
            BattleBeat::End(Outcome::Escaped) | BattleBeat::Defense(_) => MessageKind::Transient,
            BattleBeat::End(Outcome::Victory) if matches!(&event, BattleEvent::Ended { .. }) => {
                self.message.clear();
                MessageKind::VictoryRewards
            }
            BattleBeat::End(_) | BattleBeat::LevelUp => MessageKind::Wide,
            BattleBeat::Reward => {
                self.message = "Victory!".into();
                MessageKind::Victory
            }
            // Attack/effect captures decode only the status strip here.
            BattleBeat::Attack(_) | BattleBeat::Damage { .. } | BattleBeat::Hide(_) => {
                MessageKind::None
            }
            BattleBeat::None if self.message.is_empty() => MessageKind::None,
            BattleBeat::None if self.message.chars().count() > 16 => MessageKind::Wide,
            BattleBeat::None => MessageKind::Transient,
        };
        if let BattleEvent::Died { fighter } = &event
            && fighter.side() == Side::Party
            && self.party_defeated()
        {
            self.message_kind = MessageKind::Wide;
            self.defeated_name = self.names.get(&fighter.get()).cloned();
        }
        if let BattleBeat::Defense(actor) = narrate.beat {
            self.transient = Some(actor);
        }
        match narrate.beat {
            BattleBeat::Hide(fighter) => {
                if fighter.side() == Side::Enemy
                    && let Some(enemy) =
                        self.enemies.iter_mut().find(|e| e.fighter == fighter.get())
                {
                    enemy.visible = false;
                }
            }
            BattleBeat::End(outcome) => self.finish_outcome = Some(outcome),
            _ => {}
        }
        let remaining = if narrate.beat == BattleBeat::End(Outcome::Defeat) {
            DEFEAT_DWELL_FRAMES
        } else if matches!(&event, BattleEvent::Died { fighter } if fighter.side() == Side::Party)
            && self.party_defeated()
        {
            0
        } else {
            BATTLE_DWELL_FRAMES
        };
        self.current = Some(ActiveBeat {
            beat: narrate.beat,
            remaining,
            total: remaining,
            waits_for_confirm: narration::waits_for_confirm(narrate.beat),
        });
        self.command_open = false;
    }

    /// The party strip follows the beats, not the round: the engine resolves a
    /// whole round at once, while the strip shows what the player has watched
    /// so far. This is the shell's own `update_live_party_hp`, moved.
    fn update_live_party(&mut self, event: &BattleEvent) {
        let (target, hp, tp) = match event {
            BattleEvent::Resolved {
                target,
                remaining_hp,
                ..
            }
            | BattleEvent::Healed {
                target,
                remaining_hp,
                ..
            }
            | BattleEvent::Revived {
                target,
                remaining_hp,
                ..
            } => (*target, Some(*remaining_hp), None),
            BattleEvent::TechniqueUsed {
                actor,
                remaining_tp,
                ..
            } => (*actor, None, Some(*remaining_tp)),
            _ => return,
        };
        if target.side() != Side::Party {
            return;
        }
        if let Some(member) = self.party.iter_mut().find(|m| m.fighter == target.get()) {
            if let Some(hp) = hp {
                member.hp = hp;
            }
            if let Some(tp) = tp {
                member.tp = tp;
            }
        }
    }

    /// Whether every party pane is down, which is the defeat that holds the
    /// last page instead of dwelling on it.
    fn party_defeated(&self) -> bool {
        !self.party.is_empty() && self.party.iter().all(|member| member.hp == 0)
    }

    /// The battle's epilogue: the round that ended it pays out, its level-up
    /// pages are queued, and the presentation closes once they are watched.
    fn service_finish(&mut self, runtime: &mut Runtime) {
        let Some((outcome, reward_each)) = self.finish_request.take() else {
            return;
        };
        if outcome == Outcome::Victory {
            self.sounds.push(SFX_VICTORY);
        }
        let reward = match outcome {
            Outcome::Victory => reward_each,
            Outcome::Escaped | Outcome::Defeat | Outcome::ScriptedExit => 0,
        };
        let levels = runtime.finish_battle_for_outcome(outcome, reward);
        // Queue level-up narration before marking idle-close, otherwise an
        // empty level-up vector could close the battle one frame early.
        self.enqueue(BattleTimeline {
            events: levels,
            sounds: Vec::new(),
            animations: Vec::new(),
        });
        self.close_when_idle = true;
        if self.current.is_none() && self.queue.is_empty() {
            self.start_next_event();
        }
    }
}

impl Session {
    /// Starts the battle this frame's events asked for, if they asked for one.
    ///
    /// An `EncounterRolled` is the runtime's own battle to start; a
    /// `SceneBattleStarted` is one the scene runner already started, and the
    /// event carries its opening timeline. A battle that cannot start leaves
    /// the field in control and reports the reason as a fault for the shell to
    /// log — the same "no battle this frame" the shell's own error paths
    /// produced.
    pub(crate) fn begin_battle(&mut self, events: &[RuntimeEvent]) -> Option<BattleFrame> {
        let start = events.iter().find_map(|event| match event {
            RuntimeEvent::EncounterRolled { formation } => Some(BattleStart::Encounter(*formation)),
            RuntimeEvent::SceneBattleStarted { index, .. } => {
                Some(BattleStart::EventBattle(*index))
            }
            _ => None,
        })?;
        let timeline = match start {
            BattleStart::Encounter(formation) => match self.start_encounter(formation) {
                Ok(timeline) => timeline,
                Err(fault) => {
                    return Some(BattleFrame {
                        view: None,
                        started: None,
                        fault: Some(fault),
                    });
                }
            },
            BattleStart::EventBattle(_) => {
                let Some(RuntimeEvent::SceneBattleStarted {
                    events,
                    sounds,
                    animations,
                    ..
                }) = events
                    .iter()
                    .find(|event| matches!(event, RuntimeEvent::SceneBattleStarted { .. }))
                else {
                    return None;
                };
                BattleTimeline {
                    events: events.clone(),
                    sounds: sounds.clone(),
                    animations: animations.clone(),
                }
            }
        };
        let mut mode = BattleMode::begin(&self.runtime, timeline, start);
        let frame = mode.start_frame(&self.runtime);
        self.mode = Mode::Battle(Box::new(mode));
        Some(frame)
    }

    /// Starts an encounter battle, or says why it cannot.
    ///
    /// The empty-party refusal is the shell's own (`encounter rolled formation
    /// {formation:#05x} with an empty party`): a battle with nobody in it can
    /// never end, so it must not start.
    pub(crate) fn start_encounter(&mut self, formation: u16) -> Result<BattleTimeline, String> {
        let party = self.runtime.battle_party();
        if party.is_empty() {
            return Err(format!(
                "encounter rolled formation {formation:#05x} with an empty party"
            ));
        }
        self.runtime
            .start_battle_timeline(formation, party)
            .map_err(|error| format!("could not start battle {formation:#05x}: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The retail battle-speed table: `12 * (Battle_Speed + 1)`, clamped to
    /// `0..=4`. Moved here from `psiv-godot/src/battle/layout.rs` with the
    /// timing it describes.
    #[test]
    fn battle_speed_uses_the_retail_dwell_table() {
        assert_eq!(
            (0..=4).map(battle_dwell_frames).collect::<Vec<_>>(),
            [12, 24, 36, 48, 60]
        );
        assert_eq!(battle_dwell_frames(99), 60);
    }
}
