//! Map transitions: a warp's map load — effects, objects, party and camera —
//! and the battle-return refresh that keeps live objects.
//!
//! Two cartridge routines load a map, and they differ in which `Map_Load_Flags`
//! bit spares the field objects — and with them `Vehicle_Index` — from being
//! cleared:
//!
//! ```text
//! GameMode_LoadFieldMap  bits 0 (after a battle) or 2 (after a cutscene)   (:107505)
//! RefreshMap             bit 3, "keep objects (map refresh during events)" (:121767)
//! ```
//!
//! Everything else clears the object memory, the tile-animation memory and
//! `Vehicle_Index` (`clr.w (Vehicle_Index).w`, `ps4.asm:107517` and `:121777`):
//! walking through a map-change tile therefore parks the machine, and the
//! party steps out of town on foot. Each routine then consumes the flags it
//! read — `andi.b #$80` keeps only bit 7, "skip palette fade in" (`:107594`,
//! the tail of the load), which the field load spends itself (`bclr #7`,
//! `:107628`: no fade-in when it was set), and `RefreshMap` zeroes the byte
//! (`:121835`).
//!
//! `Runtime::map_load_flags` is that byte. A scene writes it with its own
//! `SetMapLoadFlags` op (`ps4.asm`'s `bset`/`bclr` pairs), and the battle
//! return is a field load with bit 0 set at battle entry (`ps4.asm:118057`,
//! `120803` → `loc_585C8`), which is why a fight does not dismount the party.
//! A zero-returning cutscene is a field load with bit 2 set by
//! `FieldRoutine_Cutscene` (`ps4.asm:120746`); that path is
//! [`Runtime::reload_field_after_cutscene`].

use psiv_core::{Cell, Direction, Flag, GameState, MapId, WarpTrigger};

use crate::bridge::{self, build_bespoke, build_wander, clear_bespoke_entry_flags};
use crate::effects;
use crate::geometry::{camera_for_record, driver_of};
use crate::map_load_adjust::{FieldLoad, adjust_field_load};
use crate::vehicle;
use crate::{BridgeError, Runtime, RuntimeEvent};

/// The map a field load read, after `GameMode_LoadFieldMap`'s redirects: the
/// payload of [`RuntimeEvent::MapChanged`]. Only this module builds one, at
/// the end of a load, so the event can only name the map now loaded, never
/// the one a warp or a scene asked for. Its field is private, so no other
/// module, in this crate or outside it, can construct one:
///
/// ```compile_fail,E0451
/// let _ = psiv_runtime::LoadedMap { map: psiv_core::MapId(0x160) };
/// ```
///
/// A reader takes the id:
///
/// ```
/// fn loaded(event: &psiv_runtime::RuntimeEvent) -> Option<psiv_core::MapId> {
///     match event {
///         psiv_runtime::RuntimeEvent::MapChanged { map, .. } => Some(map.id()),
///         _ => None,
///     }
/// }
/// # let _ = loaded;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadedMap {
    map: MapId,
}

impl LoadedMap {
    /// The map now loaded.
    #[must_use]
    pub fn id(self) -> MapId {
        self.map
    }

    /// The event the load reports, through `trigger`'s transition.
    pub(crate) fn event(self, trigger: WarpTrigger) -> RuntimeEvent {
        RuntimeEvent::MapChanged { map: self, trigger }
    }
}

/// Which map-load routine's `Map_Load_Flags` test a load follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MapLoad {
    /// `GameMode_LoadFieldMap`: bits 0 and 2 keep the objects and the vehicle.
    /// A warp, a teleport and the battle return take this path.
    Field,
    /// `RefreshMap`: bit 3 keeps the objects and the vehicle. A scene's
    /// `LoadMap` op takes this path.
    Refresh,
}

/// Bit 0 of `Map_Load_Flags`, set when a battle begins.
pub(crate) const LOAD_FLAG_AFTER_BATTLE: u8 = 0b0000_0001;
/// Bit 2, set by `FieldRoutine_Cutscene` on a zero return (`ps4.asm:120746`).
pub(crate) const LOAD_FLAG_AFTER_CUTSCENE: u8 = 0b0000_0100;
/// Bit 7: "skip palette fade in" (`ps4.asm:107628`).
pub(crate) const LOAD_FLAG_SKIP_FADE: u8 = 0b1000_0000;

/// What a cutscene-return reload decided: the two flag-dependent frame terms.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CutsceneReload {
    /// The music branch wrote `Saved_Sound_Index`: one more `VInt_Prepare`.
    pub(crate) music_written: bool,
    /// Bit 7 was set: no `Pal_FadeIn`.
    pub(crate) skip_fade: bool,
}

/// `AdjustMusicIDs` (`US ROM $0545F0`), after the field loader has already
/// skipped zero, battle bit 0, and a raw id equal to `Saved_Sound_Index`.
/// `None` is the routine's nonzero return: do not write the saved word.
pub(super) fn adjusted_map_music(map: MapId, raw: u8, game: &GameState, saved: u8) -> Option<u8> {
    if raw == 0 || raw == saved {
        return None;
    }
    if map.0 == 1 && game.is_set(Flag::event(0x9E)) {
        return (saved != 0x9D).then_some(0x9D);
    }
    if game.vehicle_index() != 0 {
        return Some(0x8D);
    }
    if matches!(map.0, 0x24 | 0x25 | 0x28 | 0x29) && game.is_set(Flag::event(0x33)) {
        return (saved != 0x84).then_some(0x84);
    }
    if map.0 == 0x3F && game.is_set(Flag::event(0x42)) && game.is_clear(Flag::event(0x67)) {
        return Some(0xA5);
    }
    if matches!(map.0, 0x8C | 0x171) {
        return Some(0xB1);
    }
    Some(raw)
}

impl Runtime {
    /// Applies the cartridge's flag test to the live vehicle and consumes the
    /// bits, as the load routine's tail does. Returns whether bit 7 was set:
    /// a field load then skips its `Pal_FadeIn` (`ps4.asm:107628-107630`).
    fn apply_map_load_flags(&mut self, kind: MapLoad) -> bool {
        let skip_fade = self.map_load_flags & LOAD_FLAG_SKIP_FADE != 0;
        let spare = match kind {
            MapLoad::Field => self.map_load_flags & 0b0000_0101 != 0,
            MapLoad::Refresh => self.map_load_flags & 0b0000_1000 != 0,
        };
        if !spare && self.vehicle.is_some() {
            // `clr.w (Vehicle_Index).w`; the party walks out of a warp or a
            // refresh on foot, as the cartridge parks the machine.
            self.vehicle = None;
            self.game.set_vehicle_index(0);
        }
        // `andi.b #$80` then `bclr #7` (`:107594`, `:107628`): both routines
        // leave the byte zero.
        self.map_load_flags = 0;
        skip_fade
    }

    /// Applies the cartridge's battle-return map load before revealing the
    /// field. Presentation calls this after results; headless callers receive
    /// the same refresh automatically on their next field tick.
    pub(crate) fn return_to_field(&mut self) -> Vec<RuntimeEvent> {
        if self.battle.is_some() || !std::mem::take(&mut self.battle_field_refresh_pending) {
            return Vec::new();
        }
        match self.refresh_field_after_battle() {
            Ok(()) => vec![RuntimeEvent::MapRefreshed],
            Err(error) => vec![RuntimeEvent::MapRefreshFailed {
                error: error.to_string(),
            }],
        }
    }

    /// The map-data walk of a `GameMode_LoadFieldMap` whose flags spare the
    /// objects (bit 0 after a battle, bit 2 after a cutscene): `LoadMapObjects`,
    /// `LoadTreasureChests` and the party placement all return without
    /// touching the object memory (`loc_53546` `:110819-110821`,
    /// `LoadMapObjects` `:110925-110930`, `LoadTreasureChests`
    /// `:110981-110983`), so the live objects, wander clocks and scene cast
    /// stay, while `MapDataManager` runs again over them.
    ///
    /// The retained objects keep what a scene wrote into them, so the
    /// dialogue ids it set (`$14(a4)`, `SceneEffect::NpcDialogueSet`) survive
    /// the walk, which rewrites only the objects its own entries name.
    fn rewalk_map_retaining_objects(&mut self, target: MapId) -> Result<(), BridgeError> {
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        let mut effects = effects::evaluate(record, &mut self.game);
        for (&index, &id) in &self.effects.dialogue_overrides {
            effects.dialogue_overrides.entry(index).or_insert(id);
        }
        let mut map = bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        self.map = map;
        self.effects = effects;
        Ok(())
    }

    /// `GameMode_LoadFieldMap` after a zero-returning cutscene
    /// (`FieldRoutine_Cutscene`, `ps4.asm:120745-120746`), in the cartridge's
    /// order: the flag test, the music word, the map-data walk, the camera
    /// from `Character_1`, and the load tail's resets. Returns whether the
    /// music branch spent its extra `VInt_Prepare` (`:107619-107626`) and
    /// whether bit 7 skipped the fade.
    pub(crate) fn reload_field_after_cutscene(&mut self) -> Result<CutsceneReload, BridgeError> {
        let load = self.redirect_kept_load();
        let target = load.map;
        let raw_music = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?
            .music
            .id;
        // `bset #2, (Map_Load_Flags).w`: the cutscene's own writes (bit 0 from
        // `Cutscene_MeetingKyra`, bit 7 from the scenes that skip the fade)
        // are already in the byte.
        self.map_load_flags |= LOAD_FLAG_AFTER_CUTSCENE;
        let music_written = self.write_map_music(target, raw_music);
        // Bit 2 spares the objects and the vehicle; the byte is spent.
        let skip_fade = self.apply_map_load_flags(MapLoad::Field);
        self.rewalk_map_retaining_objects(target)?;
        self.saved_map_index_2 = load.previous_map;
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        // `loc_53854` re-centres every camera on `Character_1`.
        let leader = match self.vehicle.as_ref() {
            Some(vehicle) => vehicle::driver_of(vehicle),
            None => driver_of(self.party.leader()),
        };
        self.camera =
            camera_for_record(leader, &self.map, record).map_err(BridgeError::Rejected)?;
        self.camera_glide = None;
        self.scene_camera_locked = false;
        // `loc_518D2`: the load tail clears the status counters and re-arms
        // the ten free steps, and `GameMode_LoadFieldMap` starts the standing
        // tile over.
        self.prev_standing = None;
        self.field_status.clock.reset();
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        // `FieldRoutine_PlaceName` runs after every field load.
        self.apply_travel_entry();
        Ok(CutsceneReload {
            music_written,
            skip_fade,
        })
    }

    /// The load's music branch (`ps4.asm:107536-107547`): a map with music,
    /// bit 0 clear and a raw id other than `Saved_Sound_Index` asks
    /// `AdjustMusicIDs`, and a zero answer writes the saved word and raises
    /// `$FFFFECED`. Returns whether it wrote.
    fn write_map_music(&mut self, map: MapId, raw: u8) -> bool {
        if self.map_load_flags & LOAD_FLAG_AFTER_BATTLE != 0 {
            return false;
        }
        match adjusted_map_music(map, raw, &self.game, self.saved_sound_index) {
            Some(id) => {
                self.saved_sound_index = id;
                true
            }
            None => false,
        }
    }

    /// `GameMode_LoadFieldMap`'s four redirects (`ps4.asm:107526-107529`) on
    /// a load that keeps the objects: the cutscene return
    /// (`FieldRoutine_Cutscene` sets game mode 8, `:120746-120747`) and the
    /// battle return reload the current map through the same routine, so a
    /// flag the scene or the battle set can change which map loads. The party
    /// placement is skipped on these loads, so of what a redirect writes only
    /// `Field_Map_Index` and `Field_Map_Index_2` matter. Returns the load;
    /// the caller writes `Field_Map_Index_2` only once the map has loaded, so
    /// a load that fails leaves it as it was.
    fn redirect_kept_load(&self) -> FieldLoad {
        let leader = self.party.leader();
        adjust_field_load(
            &self.game,
            FieldLoad {
                map: self.map.id(),
                cell: leader.cell(),
                facing: leader.facing(),
                previous_map: self.saved_map_index_2,
            },
        )
    }

    pub(crate) fn refresh_field_after_battle(&mut self) -> Result<(), BridgeError> {
        // GameMode_LoadFieldMap with Map_Load_Flags bit 0: the map-data
        // walk runs, while object initialization, party placement and camera
        // initialization do not. The loader's redirects run first, as on
        // every `GameMode_LoadFieldMap`.
        let load = self.redirect_kept_load();
        self.rewalk_map_retaining_objects(load.map)?;
        self.saved_map_index_2 = load.previous_map;
        // The battle set bit 0 when it began (`bset #0`, `ps4.asm:118057` and
        // `120803`), so this load keeps the objects — and the party stays
        // mounted, because the flag test below finds the bit still set.
        self.map_load_flags |= LOAD_FLAG_AFTER_BATTLE;
        self.apply_map_load_flags(MapLoad::Field);
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        Ok(())
    }

    pub(crate) fn change_map(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
    ) -> Result<LoadedMap, BridgeError> {
        self.change_map_from(target, cell, facing, self.map.id().0)
    }

    /// Loads `target` the way a scene's `LoadMap` op does: `RefreshMap`, whose
    /// object-keeping bit is 3.
    pub(crate) fn change_map_refresh(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
        previous_map: u16,
        clear_load_flags: u8,
    ) -> Result<LoadedMap, BridgeError> {
        // The scene's own `bclr`/`bset` writes precede the load call.
        self.map_load_flags &= !clear_load_flags;
        self.change_map_kind(target, cell, facing, previous_map, MapLoad::Refresh)
    }

    pub(crate) fn change_map_from(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
        previous_map: u16,
    ) -> Result<LoadedMap, BridgeError> {
        self.change_map_kind(target, cell, facing, previous_map, MapLoad::Field)
    }

    fn change_map_kind(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
        previous_map: u16,
        kind: MapLoad,
    ) -> Result<LoadedMap, BridgeError> {
        // `GameMode_LoadFieldMap` redirects four maps from the event flags
        // before it reads the record (`ps4.asm:107526-107529`); `RefreshMap`
        // does not.
        let FieldLoad {
            map: target,
            cell,
            facing,
            previous_map,
        } = match kind {
            MapLoad::Field => adjust_field_load(
                &self.game,
                FieldLoad {
                    map: target,
                    cell,
                    facing,
                    previous_map,
                },
            ),
            MapLoad::Refresh => FieldLoad {
                map: target,
                cell,
                facing,
                previous_map,
            },
        };
        let keep_old_music =
            matches!(kind, MapLoad::Field) && self.map_load_flags & LOAD_FLAG_AFTER_BATTLE != 0;
        // The routine's flag test comes first in the cartridge: the object
        // memory and `Vehicle_Index` are cleared — or spared — before a byte
        // of map data is read (`ps4.asm:107507-107521`, `:121769-121780`).
        self.apply_map_load_flags(kind);
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        // RefreshMap never writes this word. GameMode_LoadFieldMap skips the
        // battle bit and uses AdjustMusicIDs after the raw comparison.
        if matches!(kind, MapLoad::Field)
            && !keep_old_music
            && let Some(id) =
                adjusted_map_music(target, record.music.id, &self.game, self.saved_sound_index)
        {
            self.saved_sound_index = id;
        }
        // MapDataManager runs inside map load, and its walk is STATEFUL:
        // flag_clear writes (from the pack's decoded data) land mid-walk so
        // later entries' gates see them — the Xanafalgue-respawn mechanism
        // tape 18 measured. Cleared flags resurrect gated objects on OTHER
        // maps at their next build; this map's own build below already sees
        // the post-clear state.
        let effects = effects::evaluate(record, &mut self.game);
        let map = bridge::field_map_entered(record, &effects, &self.game)?;
        clear_bespoke_entry_flags(&mut self.game, record);
        self.effects = effects;
        // LoadMapObjects creates a fresh cast. Scene despawns only change
        // the live objects; persistent removals come from MapDataManager's
        // extracted flag gates above (including recruited party members).
        self.party
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.wander = build_wander(&map, record)?;
        self.bespoke = build_bespoke(&map, record)?;
        // Map entry places the view rather than scrolling it in, so the camera
        // starts framed on the party wherever the warp dropped them.
        self.camera = camera_for_record(driver_of(self.party.leader()), &map, record)
            .map_err(BridgeError::Rejected)?;
        self.map = map;
        if let Some(vehicle) = self.vehicle.as_mut() {
            if !vehicle.enter_map(&self.map, cell, facing) {
                return Err(BridgeError::Rejected(format!(
                    "vehicle destination ({}, {}) is outside map {}",
                    cell.x, cell.y, target.0
                )));
            }
            self.camera = camera_for_record(vehicle::driver_of(vehicle), &self.map, record)
                .map_err(BridgeError::Rejected)?;
        }
        // The first field control tick checks RunEvents before accepting a
        // step. loc_518D2 also resets the encounter countdown on EVERY load.
        self.prev_standing = None;
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        self.saved_map_index_2 = previous_map;
        self.apply_travel_entry();
        Ok(LoadedMap { map: target })
    }
}

#[cfg(test)]
mod map_music_tests {
    use super::*;

    #[test]
    fn adjust_music_ids_retains_retail_priority_and_skip_cases() {
        let mut game = GameState::new();
        assert_eq!(adjusted_map_music(MapId(0), 0, &game, 0x8D), None);
        assert_eq!(adjusted_map_music(MapId(0), 0x84, &game, 0x84), None);
        assert_eq!(adjusted_map_music(MapId(0), 0x80, &game, 0x84), Some(0x80));

        game.set_vehicle_index(1);
        assert_eq!(adjusted_map_music(MapId(0), 0x80, &game, 0x84), Some(0x8D));
        game.set(Flag::event(0x9E)).unwrap();
        assert_eq!(adjusted_map_music(MapId(1), 0x81, &game, 0x84), Some(0x9D));
        assert_eq!(adjusted_map_music(MapId(1), 0x81, &game, 0x9D), None);

        game.set_vehicle_index(0);
        game.set(Flag::event(0x33)).unwrap();
        for map in [0x24, 0x25, 0x28, 0x29] {
            assert_eq!(adjusted_map_music(MapId(map), 0x80, &game, 0), Some(0x84));
            assert_eq!(adjusted_map_music(MapId(map), 0x80, &game, 0x84), None);
        }
        game.set(Flag::event(0x42)).unwrap();
        assert_eq!(adjusted_map_music(MapId(0x3F), 0x80, &game, 0), Some(0xA5));
        game.set(Flag::event(0x67)).unwrap();
        assert_eq!(adjusted_map_music(MapId(0x3F), 0x80, &game, 0), Some(0x80));
        for map in [0x8C, 0x171] {
            assert_eq!(adjusted_map_music(MapId(map), 0x80, &game, 0), Some(0xB1));
        }
    }
}

#[cfg(test)]
mod kept_load_redirects {
    use super::*;
    use psiv_core::StepFrames;
    use psiv_data::GameData;
    use std::path::Path;

    const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
    const RESHEL_1: u16 = 0x14D;

    /// A runtime on `Reshel1` at Dezolis warp 0's arrival, with Dark Force 2
    /// fought when `dark_force_2`.
    fn on_reshel(dark_force_2: bool) -> Option<Runtime> {
        let pack = Path::new(PACK);
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack absent; skipping");
            return None;
        }
        let mut rt = Runtime::new(
            GameData::load(pack).unwrap(),
            RESHEL_1,
            Cell::new(30, 49),
            Direction::Up,
            StepFrames::default(),
        )
        .unwrap();
        if dark_force_2 {
            rt.game.set(Flag::event(0x9E)).unwrap();
        }
        Some(rt)
    }

    /// `AdjustReshelMap` runs on the battle return's load: with `$9E` set
    /// after the map loaded, the return reads `Reshel1 + 1`; without it (the
    /// negative control) the map stays.
    #[test]
    fn the_battle_return_load_follows_the_redirects() {
        for (flag, want) in [(true, RESHEL_1 + 1), (false, RESHEL_1)] {
            let Some(mut rt) = on_reshel(flag) else {
                return;
            };
            rt.refresh_field_after_battle().unwrap();
            assert_eq!(rt.map_id(), MapId(want), "Dark Force 2 {flag}");
        }
    }

    /// A kept load whose redirected map is not in the pack fails and leaves
    /// `Field_Map_Index_2` and the map as they were. On `GumbiousEntrance`
    /// after Dark Force 2 the redirect names `Gumbious_B2` and writes
    /// `Field_Map_Index_2` = Dezolis, so a write before the load would show;
    /// the pack is the real one with `Gumbious_B2` moved from `maps` to
    /// `skipped` in a copy of its manifest (every other entry a link).
    #[cfg(unix)]
    #[test]
    fn a_failed_kept_load_writes_nothing() {
        let pack = Path::new(PACK);
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack absent; skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("psiv-kept-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(pack).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() != "manifest.json" {
                std::os::unix::fs::symlink(
                    entry.path().canonicalize().unwrap(),
                    dir.join(entry.file_name()),
                )
                .unwrap();
            }
        }
        let text = std::fs::read_to_string(pack.join("manifest.json")).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
        let b2 = 0x164_u16;
        let maps = manifest["maps"].as_array_mut().unwrap();
        maps.retain(|entry| entry["id"].as_u64() != Some(u64::from(b2)));
        manifest["skipped"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": b2, "id_hex": "0x164", "reason": "test", "symbol": "Gumbious_B2"
            }));
        std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        let data = GameData::load(&dir).expect("the pack without Gumbious_B2");
        let _ = std::fs::remove_dir_all(&dir);
        let mut rt = Runtime::new(
            data,
            0x160,
            Cell::new(31, 39),
            Direction::Up,
            StepFrames::default(),
        )
        .unwrap();
        rt.game.set(Flag::event(0x9E)).unwrap();
        rt.saved_map_index_2 = 0x1234;
        assert!(matches!(
            rt.refresh_field_after_battle(),
            Err(BridgeError::NotPacked(id)) if id == b2
        ));
        assert_eq!(rt.saved_map_index_2, 0x1234, "Field_Map_Index_2 untouched");
        assert_eq!(rt.map_id(), MapId(0x160));
        assert!(rt.reload_field_after_cutscene().is_err());
        assert_eq!(rt.saved_map_index_2, 0x1234);
    }

    /// The same on the cutscene return's load.
    #[test]
    fn the_cutscene_return_load_follows_the_redirects() {
        for (flag, want) in [(true, RESHEL_1 + 1), (false, RESHEL_1)] {
            let Some(mut rt) = on_reshel(flag) else {
                return;
            };
            rt.reload_field_after_cutscene().unwrap();
            assert_eq!(rt.map_id(), MapId(want), "Dark Force 2 {flag}");
        }
    }
}
