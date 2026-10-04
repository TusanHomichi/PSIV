//! The driver: one [`Session`], one pad per frame, and everything a controller
//! needs to watch the frame it just ran.
//!
//! Controllers (`walk`, `talk`, `shopping`, `camping`, `battle`) produce pads and
//! call [`Driver::tick`]. The driver is the only thing that touches
//! [`Session::frame`], so it is also the only thing that records the tape,
//! spends the frame budget, keeps the event ring for a halt report, and turns a
//! faulting frame into a [`Halt`]. It reads the runtime through its read-only
//! views and never calls a mutator.

use std::collections::VecDeque;
use std::path::PathBuf;

use psiv_core::{Cell, Direction};
use psiv_runtime::{
    BattleStart, BattleView, Button, DialogueSignal, Frame, Pad, Routed, Runtime, RuntimeEvent,
    SaveStore, SceneDialogueOpen, Session,
};
use serde_json::{Value, json};

use crate::halt::{Halt, HaltKind, Res};
use crate::policy::Policy;

/// Whether `event` is a fault the run halts on. `scene_began` is whether the
/// same frame started a scene: the cartridge runs `RunEvents` before
/// `RunMapTransitions` on every landing (`ps4.asm:116768-116773`), so a landing
/// that started a scene never reached the map-change table, and the runtime's
/// report of the type-1 cell it landed on as unmapped is no fault there (the
/// Mota Spaceport's boarding row, RUNNER_LOG C4).
fn is_scene_fault(event: &RuntimeEvent, scene_began: bool) -> bool {
    match event {
        RuntimeEvent::SceneFaulted { .. }
        | RuntimeEvent::SceneMissing { .. }
        | RuntimeEvent::SceneBattleFailed { .. }
        | RuntimeEvent::MapRefreshFailed { .. }
        | RuntimeEvent::UnpackedTarget { .. } => true,
        RuntimeEvent::WarpUnmapped { .. } => !scene_began,
        _ => false,
    }
}

/// How many runtime events a halt report keeps.
pub const EVENT_RING: usize = 50;

/// A line in the ring is cut to this many characters.
const EVENT_WIDTH: usize = 300;

/// Where the runner wrote or would write the route's own `save` objectives.
pub const ROUTE_SAVE_DIR: &str = "route";

/// One fought battle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleRecord {
    /// `encounter` or `event`.
    pub kind: &'static str,
    /// The formation id or the event battle index.
    pub id: u16,
    /// Frame the battle began on.
    pub start_frame: u64,
    /// Frame the field took the screen back on; `0` while it runs.
    pub end_frame: u64,
    /// Meseta the victory page showed.
    pub meseta: u16,
    /// Experience each member's page showed.
    pub experience: u16,
}

/// A session being played one frame at a time.
pub struct Driver {
    session: Session,
    pads: Vec<u8>,
    budget: u64,
    ring: VecDeque<String>,
    suppressed: u64,
    battle_view: Option<BattleView>,
    routed: Vec<Routed>,
    areas: Vec<(u32, u16)>,
    scenes_ended: u64,
    battles: Vec<BattleRecord>,
    in_battle: bool,
    /// Where the route's `save` objectives write, or `None` while replaying
    /// (a replay answers the request without touching the disk).
    /// The scratch save directory a driver without a save directory owns.
    scratch: Option<PathBuf>,
    policy: Option<Box<dyn Policy>>,
    recovery_due: bool,
    trace: bool,
}

/// Where the party stands: the vehicle's cell while mounted (the leader's own
/// state does not move then), the leader's cell on foot.
#[must_use]
pub fn standing_cell(runtime: &Runtime) -> Cell {
    runtime
        .vehicle_cell()
        .unwrap_or_else(|| runtime.state().cell())
}

/// Whether the party is between two standing cells.
#[must_use]
pub fn is_stepping(runtime: &Runtime) -> bool {
    runtime
        .vehicle_state()
        .map_or_else(|| runtime.state().is_stepping(), |v| v.is_moving())
}

/// The pad that holds one direction.
#[must_use]
pub fn dir_pad(direction: Direction) -> Pad {
    Pad::new(match direction {
        Direction::Up => Button::Up,
        Direction::Down => Button::Down,
        Direction::Left => Button::Left,
        Direction::Right => Button::Right,
    })
}

impl Driver {
    /// A driver over `session`. `save_dir` is where the route's own `save`
    /// objectives write (`None` for a replay).
    #[must_use]
    pub fn new(mut session: Session, save_dir: Option<PathBuf>) -> Driver {
        // The session owns save I/O. A route's `save` objective must behave
        // the same in a run and a replay, so a driver with no directory (a
        // replay, a test) gets a private scratch store it removes on drop.
        let (store_dir, scratch) = match &save_dir {
            Some(dir) => (dir.join(ROUTE_SAVE_DIR), None),
            None => {
                let dir = scratch_dir();
                (dir.clone(), Some(dir))
            }
        };
        session.set_save_store(SaveStore::new(store_dir));
        Driver {
            session,
            pads: Vec::new(),
            budget: u64::MAX,
            ring: VecDeque::new(),
            suppressed: 0,
            battle_view: None,
            routed: Vec::new(),
            areas: Vec::new(),
            scenes_ended: 0,
            battles: Vec::new(),
            in_battle: false,
            scratch,
            policy: None,
            recovery_due: false,
            trace: std::env::var_os("PSIV_CAMPAIGN_TRACE").is_some(),
        }
    }

    /// Chooses how battles are fought from here on.
    pub fn set_policy(&mut self, policy: Box<dyn Policy>) {
        self.policy = Some(policy);
    }

    /// Fights the battle the session is in with the current policy.
    ///
    /// # Errors
    ///
    /// As [`Driver::tick`]; or a halt when no policy was set.
    pub fn fight(&mut self) -> Res {
        let Some(mut policy) = self.policy.take() else {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                "a battle began but no policy is set",
            ));
        };
        let result = crate::battle::fight(self, policy.as_mut());
        self.recovery_due = policy.recovers();
        self.policy = Some(policy);
        result
    }

    /// Whether a battle ended and the camp has not yet cured the party; read
    /// once by [`Driver::settle`] when the party next stands at rest.
    pub(crate) fn take_recovery_due(&mut self) -> bool {
        std::mem::take(&mut self.recovery_due)
    }

    /// The session, read-only: views and nothing else.
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The runtime's read-only views.
    #[must_use]
    pub fn runtime(&self) -> &psiv_runtime::Runtime {
        self.session.runtime()
    }

    /// Frames played so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.pads.len() as u64
    }

    /// The tape so far: one joypad byte per frame.
    #[must_use]
    pub fn pads(&self) -> &[u8] {
        &self.pads
    }

    /// Gives the next objective `frames` frames to finish in.
    pub fn set_budget(&mut self, frames: u64) {
        self.budget = frames;
    }

    /// Frames the current objective may still spend.
    #[must_use]
    pub fn budget_left(&self) -> u64 {
        self.budget
    }

    /// The battles fought so far, in order.
    #[must_use]
    pub fn battles(&self) -> &[BattleRecord] {
        &self.battles
    }

    /// The windows the session opened since the last [`Driver::clear_routed`].
    #[must_use]
    pub fn routed(&self) -> &[Routed] {
        &self.routed
    }

    /// The interaction areas (`area index`, `event`) the party stood in front
    /// of and triggered since the last [`Driver::clear_routed`]: an event the
    /// map's own areas start, which is no object's dialogue.
    #[must_use]
    pub fn areas(&self) -> &[(u32, u16)] {
        &self.areas
    }

    /// How many scenes have ended since the run began: a walk that a scene
    /// interrupted reads it to tell the scene took the party.
    #[must_use]
    pub fn scenes_ended(&self) -> u64 {
        self.scenes_ended
    }

    /// Forgets the routed windows and areas, at the start of an objective.
    pub fn clear_routed(&mut self) {
        self.routed.clear();
        self.areas.clear();
    }

    /// The last battle view the session produced; `None` outside a battle.
    #[must_use]
    pub fn battle_view(&self) -> Option<&BattleView> {
        self.battle_view.as_ref()
    }

    /// Adds a line to the event ring.
    pub fn note(&mut self, line: impl Into<String>) {
        let mut line: String = line.into();
        if line.chars().count() > EVENT_WIDTH {
            line = line.chars().take(EVENT_WIDTH).collect::<String>() + "...";
        }
        if self.ring.len() == EVENT_RING {
            self.ring.pop_front();
        }
        let line = format!("f{}: {line}", self.frames());
        if self.trace {
            eprintln!("{line}");
        }
        self.ring.push_back(line);
    }

    /// Runs one frame with `pad`.
    ///
    /// # Errors
    ///
    /// A [`Halt`] when the budget is spent, a scene or battle faulted, an
    /// ability is unsupported, the party was defeated, or the save the camp
    /// asked for could not be written.
    pub fn tick(&mut self, pad: Pad) -> Res {
        if self.budget == 0 {
            return Err(Halt::new(
                HaltKind::BudgetExhausted,
                "the objective used its whole frame budget",
            ));
        }
        self.budget -= 1;
        let frame = self.session.frame(pad);
        self.pads.push(pad.bits());
        self.absorb(frame)
    }

    /// `count` neutral frames.
    ///
    /// # Errors
    ///
    /// As [`Driver::tick`].
    pub fn neutral(&mut self, count: u32) -> Res {
        for _ in 0..count {
            self.tick(Pad::NEUTRAL)?;
        }
        Ok(())
    }

    /// One fresh press of `button`: down for a frame, released for the next.
    ///
    /// # Errors
    ///
    /// As [`Driver::tick`].
    pub fn tap(&mut self, button: Button) -> Res {
        self.tick(Pad::new(button))?;
        self.tick(Pad::NEUTRAL)
    }

    fn absorb(&mut self, frame: Frame) -> Res {
        let mut fault: Option<Halt> = None;
        let mut raise = |kind: HaltKind, detail: String| {
            if fault.is_none() {
                fault = Some(Halt::new(kind, detail));
            }
        };
        for routed in &frame.routed {
            self.note(format!("routed {routed:?}"));
            self.routed.push(*routed);
            if let Routed::SceneDialogue {
                entry,
                outcome: SceneDialogueOpen::UnknownTree,
            } = routed
            {
                raise(
                    HaltKind::SceneFault,
                    format!("scene dialogue entry {entry:#x}: its tree is absent from the pack"),
                );
            }
        }
        if let Some(start) = frame.scene_started {
            self.note(format!("dialogue fired scene {start:?}"));
            if !start.started {
                raise(
                    HaltKind::SceneFault,
                    format!("dialogue event {:#x} has no transcribed scene", start.event),
                );
            }
        }
        let scene_began = frame
            .events
            .iter()
            .chain(&frame.menu_events)
            .any(|event| matches!(event, RuntimeEvent::SceneStarted { .. }));
        for event in frame.events.iter().chain(&frame.menu_events) {
            match event {
                RuntimeEvent::StepCompleted { .. } | RuntimeEvent::ScenePresentation { .. } => {
                    self.suppressed += 1;
                    continue;
                }
                RuntimeEvent::SceneStartedFromInteraction { area, event } => {
                    self.areas.push((*area, *event));
                }
                RuntimeEvent::SceneEnded => self.scenes_ended += 1,
                other if is_scene_fault(other, scene_began) => {
                    raise(HaltKind::SceneFault, format!("{event:?}"));
                }
                _ => {}
            }
            self.note(format!("{event:?}"));
        }
        for signal in frame.signals.iter().chain(&frame.window_signals) {
            match signal {
                DialogueSignal::Fault(line) => {
                    raise(HaltKind::SceneFault, format!("dialogue fault: {line}"));
                }
                other => self.note(format!("dialogue {other:?}")),
            }
        }
        if let Some(battle) = frame.battle {
            if let Some(view) = battle.view {
                self.track_rewards(&view);
                self.battle_view = Some(view);
            }
            if let Some(start) = battle.started {
                self.note(format!("battle began {start:?}"));
                let (kind, id) = match start {
                    BattleStart::Encounter(id) => ("encounter", id),
                    BattleStart::EventBattle(id) => ("event", id),
                };
                self.battles.push(BattleRecord {
                    kind,
                    id,
                    start_frame: self.frames(),
                    end_frame: 0,
                    meseta: 0,
                    experience: 0,
                });
            }
            if let Some(text) = battle.fault {
                self.note(format!("battle fault: {text}"));
                let kind = if text.contains("unsupported ability")
                    || text.contains("no effect dispatcher")
                {
                    HaltKind::UnsupportedAbility
                } else {
                    HaltKind::SceneFault
                };
                raise(kind, text);
            }
        }
        if let Some(failure) = &frame.camp_save_error {
            raise(
                HaltKind::SaveFailed,
                format!(
                    "camp SAVE to slot {} failed: {}",
                    failure.slot, failure.error
                ),
            );
        }
        let active = self.session.battle_active();
        if self.in_battle && !active {
            let end = self.frames();
            if let Some(record) = self.battles.last_mut() {
                record.end_frame = end;
            }
        }
        self.in_battle = active;
        if !active {
            self.battle_view = None;
        }
        if self.session.runtime().game_over() {
            raise(
                HaltKind::LostBattle,
                "the party was defeated and the game is over".to_owned(),
            );
        }
        fault.map_or(Ok(()), Err)
    }

    fn track_rewards(&mut self, view: &BattleView) {
        if let Some(record) = self.battles.last_mut() {
            if view.reward_meseta != 0 {
                record.meseta = view.reward_meseta;
            }
            if view.reward_each != 0 {
                record.experience = view.reward_each;
            }
        }
    }

    /// The mode the session is in, for the report.
    #[must_use]
    pub fn mode_name(&self) -> &'static str {
        let runtime = self.session.runtime();
        if self.session.battle_active() {
            "battle"
        } else if self.session.destination_view().is_some() {
            "destination"
        } else if self.session.shop_view().is_some() {
            "shop"
        } else if self.session.camp_view().is_some() {
            "camp"
        } else if runtime.dialogue_open() {
            "dialogue"
        } else if runtime.scene_active() {
            "scene"
        } else {
            "field"
        }
    }

    /// Whether the party stands in the field with nothing open: the moment a
    /// walking or interacting controller may press a direction.
    #[must_use]
    pub fn at_rest(&self) -> bool {
        let runtime = self.session.runtime();
        self.mode_name() == "field"
            && !is_stepping(runtime)
            && runtime.loot_state().is_none()
            && runtime.field_notice().is_none()
    }

    /// The party as the camp would list it.
    #[must_use]
    pub fn party_json(&self) -> Value {
        let camp = self.session.runtime().camp_state();
        Value::Array(
            camp.party
                .iter()
                .map(|member| {
                    json!({
                        "slot": member.party_slot,
                        "name": member.name,
                        "level": member.level,
                        "hp": member.current_hp,
                        "max_hp": member.max_hp,
                        "tp": member.current_tp,
                        "max_tp": member.max_tp,
                        "status": member.status,
                    })
                })
                .collect(),
        )
    }

    /// The report's state block: where the run is, in views only.
    #[must_use]
    pub fn snapshot(&self) -> Value {
        let runtime = self.session.runtime();
        let cell = standing_cell(runtime);
        json!({
            "frame": self.frames(),
            "map": runtime.map_id().0,
            "cell": [cell.x, cell.y],
            "facing": format!("{:?}", runtime.state().facing()),
            "mode": self.mode_name(),
            "party": self.party_json(),
            "money": runtime.game().money(),
            "surroundings": self.surroundings(),
            "nearby_warps": self.nearby_warps(),
            "events": self.ring.iter().collect::<Vec<_>>(),
            "events_suppressed": self.suppressed,
            "view": self.view_json(),
        })
    }

    /// The cells around the party, one string a row: `@` the party, `n` an
    /// object, otherwise the cell's collision value as a hex digit (`0` open
    /// ground, `1` a map-change cell, `8` solid, `c` a counter, ...), a space
    /// off the map. Eleven columns by nine rows, the party in the middle.
    fn surroundings(&self) -> Vec<String> {
        let runtime = self.session.runtime();
        let map = runtime.map();
        let here = standing_cell(runtime);
        (-4_i32..=4)
            .map(|dy| {
                (-5_i32..=5)
                    .map(|dx| {
                        let at =
                            map.normalize_signed(i32::from(here.x) + dx, i32::from(here.y) + dy);
                        match at {
                            None => ' ',
                            Some(cell) if cell == here => '@',
                            Some(cell) if map.npc_at(cell).is_some() => 'n',
                            Some(cell) => map.collision_at(cell).map_or('?', |c| {
                                char::from_digit(u32::from(c.to_raw()), 16).unwrap_or('?')
                            }),
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// The warps whose trigger area is within a few cells of the party, as
    /// `index source-rect trigger -> target`, for a walk that cannot fire one.
    fn nearby_warps(&self) -> Vec<String> {
        let runtime = self.session.runtime();
        let map = runtime.map();
        let here = standing_cell(runtime);
        map.warps()
            .iter()
            .enumerate()
            .filter(|(_, w)| {
                let r = w.source;
                (i32::from(r.x) - i32::from(here.x)).abs() <= 8
                    && (i32::from(r.y) - i32::from(here.y)).abs() <= 8
            })
            .map(|(i, w)| {
                format!(
                    "{i}: {:?} {:?} -> map {:#x}",
                    w.source, w.trigger, w.target_map.0
                )
            })
            .collect()
    }

    fn view_json(&self) -> Value {
        let runtime = self.session.runtime();
        let mut view = serde_json::Map::new();
        if let Some(dialogue) = runtime.dialogue_view() {
            view.insert("dialogue".into(), json!(format!("{dialogue:?}")));
        }
        if let Some(shop) = self.session.shop_view() {
            view.insert("shop".into(), json!(format!("{shop:?}")));
        }
        if let Some(camp) = self.session.camp_view() {
            view.insert(
                "camp".into(),
                json!({"page": format!("{:?}", camp.page), "message": camp.message}),
            );
        }
        if let Some(battle) = self.battle_view.as_ref() {
            view.insert("battle".into(), json!(format!("{battle:?}")));
        }
        if let Some(loot) = runtime.loot_state() {
            view.insert("loot".into(), json!(format!("{loot:?}")));
        }
        if let Some(notice) = runtime.field_notice() {
            view.insert("field_notice".into(), json!(format!("{notice:?}")));
        }
        Value::Object(view)
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        if let Some(dir) = self.scratch.take() {
            // Only the directory this driver created; ignore a failed cleanup.
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// A fresh, private directory for a driver's scratch save store.
fn scratch_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("psiv-campaign-{}-{n}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use psiv_core::Cell;
    use psiv_runtime::RuntimeEvent;

    use super::is_scene_fault;

    fn unmapped() -> RuntimeEvent {
        RuntimeEvent::WarpUnmapped {
            cell: Cell { x: 30, y: 19 },
        }
    }

    /// `RunEvents` runs before `RunMapTransitions`: an unmapped type-1 cell in
    /// the frame that started a scene is the event's cell, not a pack defect.
    #[test]
    fn an_unmapped_cell_is_no_fault_when_the_frame_started_a_scene() {
        assert!(!is_scene_fault(&unmapped(), true));
    }

    /// Negative control: the same event with no scene in the frame is still the
    /// pack defect it names, and the unconditional faults ignore the scene.
    #[test]
    fn an_unmapped_cell_with_no_scene_and_the_other_faults_still_halt() {
        assert!(is_scene_fault(&unmapped(), false));
        for began in [false, true] {
            assert!(is_scene_fault(
                &RuntimeEvent::UnpackedTarget {
                    map: psiv_core::MapId(0x18D)
                },
                began
            ));
            assert!(!is_scene_fault(&RuntimeEvent::SceneEnded, began));
        }
    }
}
