//! The route file: an ordered list of chapters, each a list of objectives the
//! runner resolves into pad input, closed by assertions.
//!
//! # Format (version 1)
//!
//! A route is JSON. Positions are standing cells `[x, y]` in the pack's cell
//! space (the same space `psiv_core::FieldMap` and the ledgers use). Maps are
//! numeric ids; the hex form is given in a `note` where it helps a reader.
//!
//! ```json
//! {
//!   "format": 1,
//!   "title": "Example",
//!   "chapters": [
//!     {
//!       "id": "academy-principal",
//!       "title": "Meet the principal",
//!       "source": "rust/psiv-runtime/examples/academy_route.rs",
//!       "random_battle_policy": "attack_all",
//!       "objectives": [
//!         { "do": "go_to", "map": 19, "cell": [38, 17] },
//!         { "do": "go_to_map", "map": 20 },
//!         { "do": "go_to", "map": 20, "cell": [15, 16] },
//!         { "do": "talk", "npc": 0, "verify": true, "note": "object index unconfirmed" },
//!         { "do": "answer", "yes": true },
//!         { "do": "expect", "flags_set": ["event:9"], "map": 20 }
//!       ],
//!       "closing": [
//!         { "flags_set": ["event:9"], "flags_clear": ["event:11"], "map": 20, "cell": [15, 16] }
//!       ]
//!     }
//!   ]
//! }
//! ```
//!
//! Each objective is an object with a `do` tag naming the objective, that
//! objective's fields, and two optional keys on any of them: `verify` (true
//! when the exact cell or object index is not confirmed against the game) and
//! `note`. Unknown keys are rejected so a typo cannot silently drop a field.
//!
//! | `do` | fields | meaning |
//! | --- | --- | --- |
//! | `go_to` | `map`, `cell` | stand on `cell` of `map`, crossing warps if needed |
//! | `step_onto` | `map`, `cell` | take the one step onto `cell`, where a map trigger starts a scene: a warp footprint (the scene wins before the warp fires, `RunEvents` runs first) or any other trigger cell; halts when the warp fires instead, or the party stands on the cell and no scene ran; the position is unknown until a later `expect` pins it |
//! | `go_to_map` | `map`, optional `via_warp` | arrive on `map` by the cheapest warp chain; `via_warp` forces the first warp (index in the current map's record) |
//! | `talk` | `npc`, optional `opens` | face and talk to object `npc` (index in the current map's object list); `opens` lists the cells an object stands on that the conversation sends away, for planning |
//! | `wait` | `frames` | press nothing for `frames` frames (1 to 600) while the field runs |
//! | `answer` | `yes` | answer an open Yes/No prompt |
//! | `interact` | `cell`, `face`, optional `opens` | stand on `cell`, face `up`/`down`/`left`/`right`, press confirm (doors, elevators, interaction areas); `opens` lists the cells that become walkable, for planning (collision 0, and any object standing on them is taken to move off: Tyler's grave blocks) |
//! | `open_chest` | `chest` | open chest `chest` of the current map (record index) |
//! | `buy` | `item`, `count`, optional `face` | buy at the shop counter the party faces |
//! | `sell` | `item`, optional `face` | sell one item |
//! | `rest_inn` | optional `face` | pay for a night at the inn counter the party faces |
//! | `equip` | `member`, `item` | equip through camp |
//! | `use_technique` | `caster`, `technique`, `target` | camp technique on a member |
//! | `use_item` | `item`, `target` | camp item on a member |
//! | `reorder` | `order` | set party order, first to last |
//! | `save` | `slot` | ordinary SAVE to slot `0..3` |
//! | `fight_scripted` | none | win the scripted battle that starts |
//! | `board` | optional `step`, `to` | step `up`/`down`/`left`/`right` onto a boarding row until the ship's destination menu opens (without `step`, wait for the menu a scene opens by itself), move its cursor to the world `to` (a `World_Index` or its name), press Speak and fly; the validator ends the party on that world's landing cell |
//! | `dismount` | none | press Action in the vehicle the party rides; halts when the standing cell is not open ground |
//! | `patrol` | `map`, `a`, `b`, `until`, optional `refuge` | walk between cells `a` and `b` of `map`, fighting what the chapter's policy says, until `until` (`party_level_at_least`, `money_at_least`) holds; `refuge` is an out-and-back list of steps run when a member has fallen or is below half HP after the camp cure; the validator takes the party to end on `b` |
//! | `expect` | any of `flags_set`, `flags_clear`, `map`, `cell`, `party`, `money_at_least`, `vehicle`, `items_held`, `items_absent`, `status_clear` | halt unless all hold |
//!
//! Names and ids: an item, technique or member is either its number or its
//! cartridge display name (`"RES"`, `"Chaz"`), compared case-insensitively. A
//! flag is `"bank:id"` with bank `event`, `chest`, `temp` or `town` and a
//! decimal or `0x` id.
//!
//! A chapter's `closing` list holds the assertions that must hold when its
//! objectives are done; `expect` is the same assertion usable mid-chapter. The
//! validator and the runner treat both identically and the validator uses
//! them as the route's claims about flags and position when it plans.
//! `random_battle_policy` names how random battles are fought in the chapter;
//! the runner defines the policies, the file only names one.

use std::fmt;

use psiv_core::{Cell, Flag, FlagBank};
use serde::{Deserialize, Serialize};

/// The only route format this crate reads.
pub const FORMAT: u32 = 1;

/// The most frames one `wait` objective may let pass.
pub const MAX_WAIT_FRAMES: u32 = 600;

/// A whole route file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// Must equal [`FORMAT`].
    pub format: u32,
    /// Human title.
    pub title: String,
    /// Where the route starts. Absent: the pack's first controllable moment
    /// (`manifest.game_start`) with its seeded flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Start>,
    /// Chapters in story order.
    pub chapters: Vec<Chapter>,
}

/// An explicit starting position and flag state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// Map id.
    pub map: u16,
    /// Standing cell.
    pub cell: Xy,
    /// Flags held at the start.
    #[serde(default)]
    pub flags_set: Vec<FlagRef>,
}

/// One chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    /// Unique short id (`academy-hahn`).
    pub id: String,
    /// Human title.
    pub title: String,
    /// The repository source the chapter's steps come from.
    pub source: String,
    /// Named random-battle policy for this chapter.
    pub random_battle_policy: String,
    /// What to do, in order.
    pub objectives: Vec<Step>,
    /// Assertions that must hold when the objectives are done.
    #[serde(default)]
    pub closing: Vec<Expectation>,
}

/// A facing for an `interact` objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    /// Toward lower y.
    Up,
    /// Toward higher y.
    Down,
    /// Toward lower x.
    Left,
    /// Toward higher x.
    Right,
}

impl Face {
    /// The engine direction.
    #[must_use]
    pub const fn direction(self) -> psiv_core::Direction {
        match self {
            Face::Up => psiv_core::Direction::Up,
            Face::Down => psiv_core::Direction::Down,
            Face::Left => psiv_core::Direction::Left,
            Face::Right => psiv_core::Direction::Right,
        }
    }
}

/// When a `patrol` stops: every given condition must hold.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Until {
    /// Every party member at or above this level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party_level_at_least: Option<u8>,
    /// The purse at or above this many meseta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub money_at_least: Option<u32>,
}

/// A standing cell `[x, y]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Xy(pub u16, pub u16);

impl Xy {
    /// The engine cell.
    #[must_use]
    pub const fn cell(self) -> Cell {
        Cell::new(self.0, self.1)
    }
}

/// A number or a cartridge name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NameOrId {
    /// By number.
    Id(u16),
    /// By display name.
    Name(String),
}

impl fmt::Display for NameOrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameOrId::Id(id) => write!(f, "#{id}"),
            NameOrId::Name(name) => write!(f, "\"{name}\""),
        }
    }
}

/// A flag written `"bank:id"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FlagRef(pub Flag);

impl TryFrom<String> for FlagRef {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let (bank, id) = text
            .split_once(':')
            .ok_or_else(|| format!("flag {text:?}: expected \"bank:id\""))?;
        let id = match id.strip_prefix("0x").or_else(|| id.strip_prefix("0X")) {
            Some(hex) => u16::from_str_radix(hex, 16),
            None => id.parse(),
        }
        .map_err(|_| format!("flag {text:?}: bad id"))?;
        let flag = match bank {
            "event" => Flag::event(id),
            "chest" => Flag::chest(id),
            "temp" => Flag::temp(id),
            "town" => Flag::town(id),
            other => return Err(format!("flag {text:?}: unknown bank {other:?}")),
        };
        Ok(FlagRef(flag))
    }
}

impl From<FlagRef> for String {
    fn from(flag: FlagRef) -> String {
        let bank = match flag.0.bank {
            FlagBank::Event => "event",
            FlagBank::Temp => "temp",
            FlagBank::Town => "town",
        };
        format!("{bank}:{:#x}", flag.0.id)
    }
}

/// The assertions of an `expect` objective or a chapter's `closing`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    /// Flags that must be set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags_set: Vec<FlagRef>,
    /// Flags that must be clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags_clear: Vec<FlagRef>,
    /// The map the party must be on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<u16>,
    /// The standing cell (requires `map`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<Xy>,
    /// The party, leader first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<Vec<NameOrId>>,
    /// A minimum purse in meseta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub money_at_least: Option<u32>,
    /// The vehicle the party rides: `0` on foot, `1` Land Rover, `2` Ice
    /// Digger, `3` Hydrofoil. The validator plans every later walk for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<u16>,
    /// Items the pack holds, each in at least one slot: what a scene's
    /// `AddItem` gave (`Cutscene_LashiecDefeated` returns the Eclipse Torch).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items_held: Vec<NameOrId>,
    /// Items in no slot of the pack: what a scene's `RemoveItem` took.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items_absent: Vec<NameOrId>,
    /// Every party member's status byte reads zero: no ailment, nobody fallen
    /// and no android shut down. What `RecoverStats` leaves (an inn, the
    /// recovery tile).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub status_clear: bool,
}

/// What a step asks of the runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case", deny_unknown_fields)]
pub enum Objective {
    /// Stand on a cell, crossing warps as needed.
    GoTo {
        /// Target map.
        map: u16,
        /// Target standing cell.
        cell: Xy,
    },
    /// Take the one step onto a cell where a map trigger starts a scene. On a
    /// warp footprint the scene runs before the warp can fire: `RunEvents`
    /// precedes `RunMapTransitions` on foot (`ps4.asm:116768-116773`), so a
    /// scene registered on that cell owns the frame. `go_to` plans a warp
    /// footprint as a terminal, never as a goal; this plans the firing step as
    /// an ordinary one and halts when it fires the warp instead. On any other
    /// trigger cell the step is the plain walk that ends there: a `go_to` would
    /// plan again from where the scene left the party and fire it twice.
    StepOnto {
        /// Map the cell is on.
        map: u16,
        /// The cell stepped onto.
        cell: Xy,
    },
    /// Arrive on a map by the cheapest warp chain.
    GoToMap {
        /// Target map.
        map: u16,
        /// Force the first warp (index in the current map's record).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via_warp: Option<u32>,
    },
    /// Talk to an object of the current map.
    Talk {
        /// Index in the map's object list.
        npc: u32,
        /// Cells of the current map an object stands on that the conversation
        /// sends away (the Esper Mansion's door guards step aside): the
        /// validator plans with them open until the party leaves the map, as
        /// for an `interact`. The runner needs nothing: it walks the live map.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        opens: Vec<Xy>,
    },
    /// Press nothing for `frames` frames while the field runs: an object a
    /// scene sent walking (guards stepping aside) finishes before the next
    /// walk is planned around it.
    Wait {
        /// Frames to let pass, 1 to 600.
        frames: u32,
    },
    /// Answer an open Yes/No prompt.
    Answer {
        /// True for Yes.
        yes: bool,
    },
    /// Stand on a cell, face a direction and press confirm: doors, elevators
    /// and interaction areas.
    Interact {
        /// Standing cell on the current map.
        cell: Xy,
        /// Facing.
        face: Face,
        /// Cells of the current map that become walkable (collision 0) once
        /// the interaction completes, for doors and elevators whose opening
        /// is a live patch rather than a flag. The validator plans with them
        /// open until the party leaves the map.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        opens: Vec<Xy>,
    },
    /// Open a chest of the current map.
    OpenChest {
        /// Index in the map's chest list.
        chest: u32,
    },
    /// Buy at the shop the party faces.
    Buy {
        /// Item.
        item: NameOrId,
        /// How many.
        count: u16,
        /// Which way the counter lies from the standing cell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        face: Option<Face>,
    },
    /// Sell one item.
    Sell {
        /// Item.
        item: NameOrId,
        /// Which way the counter lies from the standing cell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        face: Option<Face>,
    },
    /// Pay for a night at the inn.
    RestInn {
        /// Which way the innkeeper lies from the standing cell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        face: Option<Face>,
    },
    /// Equip an item through camp.
    Equip {
        /// Party member.
        member: NameOrId,
        /// Item.
        item: NameOrId,
    },
    /// Cast a technique from camp.
    UseTechnique {
        /// Casting member.
        caster: NameOrId,
        /// Technique.
        technique: NameOrId,
        /// Target member.
        target: NameOrId,
    },
    /// Use an item from camp.
    UseItem {
        /// Item.
        item: NameOrId,
        /// Target member.
        target: NameOrId,
    },
    /// Set the party order.
    Reorder {
        /// Members first to last.
        order: Vec<NameOrId>,
    },
    /// Ordinary SAVE.
    Save {
        /// Slot `0..3`.
        slot: u8,
    },
    /// Win the scripted battle that starts.
    FightScripted,
    /// Step onto a boarding row, pick a world in the ship's destination menu
    /// and fly there. Without a `step` the menu is one a scene opens by
    /// itself (`Cutscene_FindingAirCastle` ends in the ship's menu): the
    /// objective waits for it.
    Board {
        /// The step that fires the boarding event; absent when a scene
        /// opens the menu.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<Face>,
        /// The destination: a `World_Index` (`0` Motavia, `1` Dezolis, `2`
        /// Rykros, `3` Zelan, `4` Kuran, `5` the Air Castle) or the pack's name
        /// for it.
        to: NameOrId,
    },
    /// Get out of the vehicle the party rides (the Action press on open
    /// ground).
    Dismount,
    /// Walk between two cells, fighting random battles, until a condition
    /// holds: the route's honest form of grinding.
    Patrol {
        /// Map walked.
        map: u16,
        /// One end.
        a: Xy,
        /// The other end.
        b: Xy,
        /// When to stop.
        until: Until,
        /// Objectives run when the party can no longer train: a member has
        /// fallen, or one is hurt below the camp's cure. They start on the
        /// patrol's map and end on it (an inn trip out and back); the patrol
        /// then resumes from wherever they end. Empty means keep fighting.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        refuge: Vec<Step>,
    },
    /// Assert state.
    Expect(Expectation),
}

/// One objective with its optional annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    /// The objective.
    #[serde(flatten)]
    pub objective: Objective,
    /// True when the exact cell or object index is not confirmed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub verify: bool,
    /// Why, or what to confirm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Deserialize)]
struct StepWire {
    #[serde(default)]
    verify: bool,
    #[serde(default)]
    note: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Value,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Step, D::Error> {
        use serde::de::Error;
        let wire = StepWire::deserialize(deserializer)?;
        let objective = Objective::deserialize(wire.rest).map_err(D::Error::custom)?;
        Ok(Step {
            objective,
            verify: wire.verify,
            note: wire.note,
        })
    }
}

impl Route {
    /// Parses a route from JSON text.
    ///
    /// # Errors
    ///
    /// The JSON error, or a message when `format` is not [`FORMAT`].
    pub fn parse(text: &str) -> Result<Route, String> {
        let route: Route = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if route.format != FORMAT {
            return Err(format!(
                "route format {} is not the supported {FORMAT}",
                route.format
            ));
        }
        Ok(route)
    }

    /// Every step marked `verify`, as `(chapter id, objective index)`.
    #[must_use]
    pub fn unverified(&self) -> Vec<(&str, usize)> {
        self.chapters
            .iter()
            .flat_map(|c| {
                c.objectives
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.verify)
                    .map(move |(i, _)| (c.id.as_str(), i))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{
      "format": 1, "title": "Example",
      "chapters": [{
        "id": "c", "title": "C", "source": "x", "random_battle_policy": "attack_all",
        "objectives": [
          {"do": "go_to", "map": 19, "cell": [38, 17]},
          {"do": "talk", "npc": 0, "verify": true, "note": "n"},
          {"do": "rest_inn", "face": "up"},
          {"do": "expect", "flags_set": ["event:9", "chest:0x0d"], "map": 20}
        ],
        "closing": [{"flags_clear": ["temp:3"], "money_at_least": 5}]
      }]}"#;

    #[test]
    fn example_round_trips() {
        let route = Route::parse(EXAMPLE).unwrap();
        assert_eq!(route.unverified(), vec![("c", 1)]);
        let again = Route::parse(&serde_json::to_string(&route).unwrap()).unwrap();
        assert_eq!(route, again);
    }

    #[test]
    fn typos_and_bad_flags_are_rejected() {
        let typo = EXAMPLE.replace("\"cell\": [38, 17]", "\"cel\": [38, 17]");
        assert!(Route::parse(&typo).is_err());
        let bad = EXAMPLE.replace("event:9", "nowhere:9");
        assert!(Route::parse(&bad).unwrap_err().contains("unknown bank"));
        let ver = EXAMPLE.replace("\"format\": 1", "\"format\": 2");
        assert!(Route::parse(&ver).unwrap_err().contains("format 2"));
    }
}
