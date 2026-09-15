//! The title initializer, distinct from the manifest's post-opening spawn.

use std::path::Path;

use serde::Deserialize;

use crate::DataError;

/// ROM-derived state before `Event_GameStart` runs. Loaded from the existing
/// `game_start.json`; the manifest summary describes a later point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGame {
    /// Initial meseta.
    pub money: u32,
    /// The five drawable party slots, before the opening rearranges them.
    pub party: [Option<u8>; 5],
    /// Preloaded flags in the combined `$100..$1FF` bank.
    pub extended_event_flags: Vec<u16>,
    /// Towns already registered by the title initializer.
    pub town_flags: Vec<u16>,
    /// Initial text speed selector.
    pub message_speed: u16,
    /// Initial battle speed selector.
    pub battle_speed: u16,
    /// Initial planet selector.
    pub world_index: u16,
    /// Map resident when the title hands off to the opening.
    pub scene_map: u16,
    /// Event dispatched by START.
    pub event_index: u16,
}

#[derive(Deserialize)]
struct File {
    format_version: u32,
    kind: String,
    new_game_init: Init,
    title_handoff: Handoff,
}

#[derive(Deserialize)]
struct Init {
    money: u32,
    party_slots: [Slot; 6],
    flags: Flags,
    settings: Settings,
}

#[derive(Deserialize)]
struct Slot {
    empty: bool,
    id: Option<u8>,
}

#[derive(Deserialize)]
struct Flags {
    extended_event_flags: FlagList,
    town_flags: FlagList,
}

#[derive(Deserialize)]
struct FlagList {
    set: Vec<String>,
}

#[derive(Deserialize)]
struct Settings {
    message_speed: u16,
    battle_speed: u16,
    world_index: u16,
}

#[derive(Deserialize)]
struct Handoff {
    scene_map: crate::GameStartMap,
    event_index: Event,
}

#[derive(Deserialize)]
struct Event {
    value: u16,
}

impl NewGame {
    pub(crate) fn load(pack_dir: &Path) -> Result<Self, DataError> {
        let path = pack_dir.join("game_start.json");
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let file: File = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        let mismatch = |field, expected: &str, actual: String| DataError::ManifestMismatch {
            path: path.clone(),
            field,
            manifest: expected.to_owned(),
            record: actual,
        };
        if file.format_version != 1 || file.kind != "game_start" {
            return Err(mismatch(
                "kind/version",
                "game_start/1",
                format!("{}/{}", file.kind, file.format_version),
            ));
        }
        let init = file.new_game_init;
        for (i, slot) in init.party_slots.iter().enumerate() {
            if slot.empty != slot.id.is_none()
                || slot.id.is_some_and(|id| id >= 11)
                || (i == 5 && !slot.empty)
            {
                return Err(mismatch(
                    "party_slots",
                    "five valid character slots and an empty sixth slot",
                    format!("slot {i}: {:?}", slot.id),
                ));
            }
        }
        let parse_flags = |flags: FlagList, field, min, max| -> Result<Vec<u16>, DataError> {
            flags
                .set
                .into_iter()
                .map(|text| {
                    text.strip_prefix("0x")
                        .and_then(|hex| u16::from_str_radix(hex, 16).ok())
                        .filter(|id| (min..max).contains(id))
                        .ok_or_else(|| mismatch(field, "hex flag in its declared bank", text))
                })
                .collect()
        };
        Ok(Self {
            money: init.money,
            party: std::array::from_fn(|i| init.party_slots[i].id),
            extended_event_flags: parse_flags(
                init.flags.extended_event_flags,
                "extended_event_flags",
                0x100,
                0x200,
            )?,
            town_flags: parse_flags(init.flags.town_flags, "town_flags", 0, 128)?,
            message_speed: init.settings.message_speed,
            battle_speed: init.settings.battle_speed,
            world_index: init.settings.world_index,
            scene_map: file.title_handoff.scene_map.id,
            event_index: file.title_handoff.event_index.value,
        })
    }
}
