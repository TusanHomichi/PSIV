//! Talking: the `talk`, `open_chest`, `interact` and `answer` controllers.
//!
//! A talk is a walk to a cell next to the object, a turn to face it, and one
//! Speak press. Which object that press reaches is the engine's decision (the
//! first active one in talk range wins), so the controller reads the answer back
//! from what the session opened ([`Routed`]) and halts when it is not the object
//! the route named. That is how a route's `verify` object indices get settled:
//! by playing them.

use psiv_core::{Cell, CollisionType, Direction};
use psiv_runtime::{Button, Routed};

use crate::cell_plan::Flood;
use crate::driver::Driver;
use crate::field::Settled;
use crate::halt::{Halt, HaltKind, Res};

/// What a Speak press opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// A dialogue with the object `npc`.
    Talk(usize),
    /// A shop or inn counter.
    Shop,
    /// An interaction area of the map started `event` (a door, a boss).
    Area(u16),
    /// A chest: its window is up.
    Chest,
}

/// Walks toward a talk target before giving up on one that keeps moving off.
const TALK_REPLANS: u32 = 8;

impl Driver {
    /// Walks next to object `npc` of the current map, faces it and presses
    /// Speak.
    ///
    /// # Errors
    ///
    /// [`HaltKind::WrongObject`] for an object that does not exist, one the
    /// press did not reach, or a press that reached nothing;
    /// [`HaltKind::Unreachable`] when no cell next to it can be reached;
    /// [`HaltKind::Stuck`] when the object keeps moving off while the party
    /// walks to it.
    pub fn talk_to(&mut self, npc: usize) -> Res<Opened> {
        self.settle(false)?;
        for _ in 0..TALK_REPLANS {
            let (stand, face) = self.standing_place(npc)?;
            self.go_to(self.map(), stand)?;
            // A wanderer can move while the party walks to it: press only when
            // the party still stands where it faces the object, and otherwise
            // plan again from here. Planning ticks no frame.
            if self.standing_place(npc)? == (self.cell(), face) {
                self.face(face)?;
                self.press_speak()?;
                return self.read_opened(Some(npc));
            }
        }
        Err(Halt::new(
            HaltKind::Stuck,
            format!("object {npc} moved off {TALK_REPLANS} times while the party walked to it"),
        ))
    }

    /// Where to stand, and which way to face, to talk to object `npc`: the
    /// nearest reachable cell with the object one cell ahead, or two cells ahead
    /// across a counter cell.
    fn standing_place(&self, npc: usize) -> Res<(Cell, Direction)> {
        let runtime = self.runtime();
        let map = runtime.map();
        let object = map.npcs().get(npc).ok_or_else(|| {
            Halt::new(
                HaltKind::WrongObject,
                format!(
                    "map {:#x} has {} objects; object {npc} does not exist",
                    self.map(),
                    map.npcs().len()
                ),
            )
        })?;
        let here = self.cell();
        let flood =
            Flood::new(map, here).map_err(|e| Halt::new(HaltKind::Unreachable, e.to_string()))?;
        let mut best: Option<(usize, Cell, Direction)> = None;
        for facing in Direction::ALL {
            let Some(next_to) = map.neighbor(object.cell, facing.opposite()) else {
                continue;
            };
            let mut candidates = vec![next_to];
            if map
                .collision_at(next_to)
                .is_some_and(|c| c == CollisionType::Shop)
                && let Some(behind) = map.neighbor(next_to, facing.opposite())
            {
                candidates.push(behind);
            }
            for stand in candidates {
                let Some(plan) = flood.cell_plan(stand) else {
                    continue;
                };
                let cost = plan.steps.len();
                if best.is_none_or(|(c, _, _)| cost < c) {
                    best = Some((cost, stand, facing));
                }
            }
        }
        best.map(|(_, stand, facing)| (stand, facing))
            .ok_or_else(|| {
                Halt::new(
                    HaltKind::Unreachable,
                    format!(
                        "no reachable cell next to object {npc} at ({},{}) on map {:#x}",
                        object.cell.x,
                        object.cell.y,
                        self.map()
                    ),
                )
            })
    }

    /// One Speak press at rest, with the routed windows forgotten first.
    fn press_speak(&mut self) -> Res {
        self.clear_routed();
        self.tap(Button::Speak)
    }

    /// Reads what the last Speak press opened.
    fn read_opened(&self, wanted: Option<usize>) -> Res<Opened> {
        for routed in self.routed() {
            match routed {
                Routed::ShopOpened { .. } => return Ok(Opened::Shop),
                Routed::Talk { npc_index, .. } => {
                    return match wanted {
                        Some(want) if want != *npc_index => Err(Halt::new(
                            HaltKind::WrongObject,
                            format!(
                                "Speak reached object {npc_index}, not the route's object {want}"
                            ),
                        )),
                        _ => Ok(Opened::Talk(*npc_index)),
                    };
                }
                Routed::NothingHere => {
                    return Err(Halt::new(
                        HaltKind::WrongObject,
                        "Speak reached nothing: the leader says there is nothing here",
                    ));
                }
                _ => {}
            }
        }
        if let Some((_, event)) = self.areas().first() {
            return Ok(Opened::Area(*event));
        }
        if self.runtime().loot_state().is_some() {
            return Ok(Opened::Chest);
        }
        Err(Halt::new(
            HaltKind::WrongObject,
            "the Speak press opened no dialogue, no shop and no interaction area",
        ))
    }

    /// The `talk` objective: reach the object, then let its conversation run
    /// until a prompt waits or the game hands control back.
    ///
    /// # Errors
    ///
    /// As [`Driver::talk_to`] and [`Driver::settle`]; a shop opening is
    /// unexpected.
    pub fn talk(&mut self, npc: usize) -> Res<Settled> {
        if self.talk_to(npc)? == Opened::Shop {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("object {npc} opened a shop, not a dialogue; the route should buy or sell"),
            ));
        }
        self.settle(true)
    }

    /// The `open_chest` objective: chest `chest` of the map is the object in
    /// the slot after the map's own objects.
    ///
    /// # Errors
    ///
    /// As [`Driver::talk`].
    pub fn open_chest(&mut self, chest: usize) -> Res {
        let slot = {
            let map = self.runtime().map();
            if chest >= map.chests().len() {
                return Err(Halt::new(
                    HaltKind::WrongObject,
                    format!(
                        "map {:#x} has {} chests; chest {chest} does not exist",
                        self.map(),
                        map.chests().len()
                    ),
                ));
            }
            map.chest_slot_base() + chest
        };
        self.talk(slot).map(|_| ())
    }

    /// The `interact` objective: stand on `cell`, face `face`, press Speak and
    /// let whatever it starts run.
    ///
    /// # Errors
    ///
    /// As [`Driver::go_to`] and [`Driver::settle`].
    pub fn interact(&mut self, cell: Cell, face: Direction) -> Res<Settled> {
        self.go_to(self.map(), cell)?;
        self.face(face)?;
        self.press_speak()?;
        self.settle(true)
    }

    /// Opens the shop or inn the party faces from where it stands.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the press opens no counter.
    pub fn open_counter(&mut self, face: Option<Direction>) -> Res {
        self.settle(false)?;
        if let Some(direction) = face {
            self.face(direction)?;
        }
        self.press_speak()?;
        match self.read_opened(None) {
            Ok(Opened::Shop) => Ok(()),
            Ok(Opened::Talk(npc)) => Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("Speak opened a dialogue with object {npc}, not a shop counter"),
            )),
            Ok(Opened::Area(event)) => Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("Speak started area event {event:#x}, not a shop counter"),
            )),
            Ok(Opened::Chest) => Err(Halt::new(
                HaltKind::UnexpectedState,
                "Speak opened a chest, not a shop counter",
            )),
            Err(halt) => Err(halt),
        }
    }

    /// The `answer` objective: bring the prompt up if it is not, then answer
    /// it with the pad and let the conversation continue to the next prompt or
    /// the end.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when no prompt opens.
    pub fn answer(&mut self, yes: bool) -> Res<Settled> {
        let mut ready = self
            .runtime()
            .dialogue_view()
            .is_some_and(|view| view.choice.is_some_and(|c| c.ready));
        if !ready {
            if self.settle(true)? != Settled::Choice {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    "no yes/no prompt is open to answer",
                ));
            }
            ready = true;
        }
        debug_assert!(ready);
        if yes {
            let on_no = self
                .runtime()
                .dialogue_view()
                .and_then(|v| v.choice)
                .is_some_and(|c| c.cursor != 0);
            if on_no {
                self.tap(Button::Up)?;
            }
            self.tap(Button::Speak)?;
        } else {
            // Retail's direct NO: Cancel answers NO wherever the cursor is.
            self.tap(Button::Cancel)?;
        }
        self.settle(true)
    }
}
