//! The spaceship's destination list and flight legs: `loc_63BC4` and the
//! routines it calls.
//!
//! `Cutscene_InsideSpaceship` (`ps4.asm:155427`) is a jump into `loc_63BC4`
//! (`:133499`), and `Cutscene_SpaceshipSabotage` carries a copy of the same
//! menu (`:155431-155470`). The menu builds its rows from a flag table and
//! `World_Index`, lets the player pick one, writes `World_Index` and flies; the
//! flight reads three tables by the current map or by the chosen world. This
//! module is those tables and the two small computations around them. The
//! window, the cursor and the buttons are the session's
//! (`psiv-runtime/src/session/destination.rs`); the legs run as ordinary scene
//! ops that ask the runtime for the map each table names.

use crate::scene::SceneOp;
use crate::state::Flag;

/// How many worlds the menu can name: Motavia, Dezolis, Rykros, Zelan, Kuran
/// and the Air Castle (the bit order of the mask, `ps4.asm:133528-133538`).
pub const WORLD_COUNT: u8 = 6;

/// `World_Index` of Motavia.
pub const WORLD_MOTAVIA: u8 = 0;
/// `World_Index` of Dezolis.
pub const WORLD_DEZOLIS: u8 = 1;
/// `World_Index` of Rykros.
pub const WORLD_RYKROS: u8 = 2;
/// `World_Index` of Zelan.
pub const WORLD_ZELAN: u8 = 3;
/// `World_Index` of Kuran.
pub const WORLD_KURAN: u8 = 4;
/// `World_Index` of the Air Castle.
pub const WORLD_AIR_CASTLE: u8 = 5;

/// The map ids the flight tables are keyed by (`ps4.constants.asm`, `MapID_*`).
const MOTA_SPACEPORT: u16 = 0x0BF;
const DEZO_SPACEPORT: u16 = 0x0D4;
const LE_ROOF_ROOM: u16 = 0x0F0;
const AIR_CASTLE: u16 = 0x171;
const ZELAN_SPACE: u16 = 0x18C;
const ZELAN: u16 = 0x18D;
const KURAN_SPACE: u16 = 0x18F;
const KURAN: u16 = 0x190;
const AIR_CASTLE_SPACE: u16 = 0x1A0;

/// Where a destination menu gets its row mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DestinationMask {
    /// The first set flag of `loc_63EF2` (`ps4.asm:133728-133735`), else
    /// `loc_63EFE`'s `$88`: `Cutscene_InsideSpaceship`'s menu.
    FlagTable,
    /// A literal mask byte: the sabotage scene's copy reads `loc_76586`
    /// (`ps4.asm:155741`), `$08`, and never looks at a flag.
    Fixed(u8),
}

/// `loc_63EF2`: flag, then the mask byte it selects (`ps4.asm:133728-133735`).
/// The first flag set wins; bits 7 to 2 are worlds 0 to 5.
pub const DESTINATION_FLAG_TABLE: [(Flag, u8); 5] = [
    (Flag::event(0xC6), 0xF8), // EventFlag_DarkForce3Defeated
    (Flag::event(0x9B), 0xD8), // EventFlag_Lashiec
    (Flag::event(0x99), 0xDC), // EventFlag_AirCastleFound
    (Flag::event(0x82), 0xD8), // EventFlag_DezoSpaceport
    (Flag::event(0x08), 0x90), // EventFlag_AlysFound
];

/// `loc_63EFE`: the mask when no table flag is set (`ps4.asm:133740`).
pub const DESTINATION_DEFAULT_MASK: u8 = 0x88;

/// The mask byte `loc_63BC4` stores at `$FFFFED42` (`ps4.asm:133499-133515`).
#[must_use]
pub fn destination_mask_byte(source: DestinationMask, flag_is_set: impl Fn(Flag) -> bool) -> u8 {
    match source {
        DestinationMask::Fixed(mask) => mask,
        DestinationMask::FlagTable => DESTINATION_FLAG_TABLE
            .iter()
            .find(|(flag, _)| flag_is_set(*flag))
            .map_or(DESTINATION_DEFAULT_MASK, |&(_, mask)| mask),
    }
}

/// The worlds the menu lists, in row order (`ps4.asm:133517-133535`): each set
/// mask bit from bit 7 down to bit 2 names a world, and the world the party is
/// in (`World_Index`) is left out.
#[must_use]
pub fn destination_worlds(mask: u8, current_world: u8) -> Vec<u8> {
    (0..WORLD_COUNT)
        .filter(|world| mask & (0x80 >> world) != 0 && *world != current_world)
        .collect()
}

/// Which table a flight leg reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlightLeg {
    /// `loc_64B02`, keyed by the map the party is on: the takeoff map.
    Takeoff,
    /// `loc_64B34`, keyed by the chosen world: the map the ship crosses.
    Transit,
    /// `loc_64B5A`, keyed by the chosen world: the landing.
    Landing,
    /// `loc_64B5A`, keyed by the current map: where Cancel puts the party back
    /// (`ps4.asm:133692-133709`), with `Field_Map_Index_2` set to `$FFFF`.
    Return,
}

/// One row of a flight table: the map to load and `Map_Start_X/Y_Pos` in
/// eight-pixel units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightTarget {
    /// The map to load.
    pub map: u16,
    /// `Map_Start_X_Pos`.
    pub start_x: u16,
    /// `Map_Start_Y_Pos`.
    pub start_y: u16,
    /// What `Field_Map_Index_2` becomes: the map the party leaves, or `$FFFF`
    /// for a return to the same map.
    pub previous_is_current: bool,
}

/// `loc_64B02` (`ps4.asm:134540-134562`): current map, takeoff map, start.
const TAKEOFF: [(u16, u16, u16, u16); 6] = [
    (MOTA_SPACEPORT, 0x000, 0x68, 0xB4),
    (DEZO_SPACEPORT, 0x001, 0x18, 0x90),
    (LE_ROOF_ROOM, 0x002, 0x80, 0x80),
    (ZELAN, ZELAN_SPACE, 0x43, 0x3C),
    (KURAN, KURAN_SPACE, 0x43, 0x3C),
    (AIR_CASTLE, AIR_CASTLE_SPACE, 0x43, 0x3C),
];

/// `loc_64B34` (`ps4.asm:134570-134588`): by world, the map crossed and start.
const TRANSIT: [(u16, u16, u16); 6] = [
    (0x000, 0x68, 0x94),
    (0x001, 0x18, 0x70),
    (0x002, 0x80, 0x60),
    (ZELAN_SPACE, 0x43, 0x1C),
    (KURAN_SPACE, 0x43, 0x1C),
    (AIR_CASTLE_SPACE, 0x43, 0x1C),
];

/// `loc_64B5A` (`ps4.asm:134593-134610`): landing map and start, one row per
/// world and, read by current map, per place the menu can be opened.
const LANDING: [(u16, u16, u16); 6] = [
    (MOTA_SPACEPORT, 0x3C, 0x28),
    (DEZO_SPACEPORT, 0x3C, 0x28),
    (LE_ROOF_ROOM, 0x3E, 0x28),
    (ZELAN, 0x3E, 0x5A),
    (KURAN, 0x3E, 0x5A),
    (AIR_CASTLE, 0x7E, 0x6A),
];

/// The maps whose takeoff is the long one (`loc_64568`, `ps4.asm:133670-133674`
/// picks it for exactly these two; every other map takes `loc_6488A`).
pub const SPACEPORT_MAPS: [u16; 2] = [MOTA_SPACEPORT, DEZO_SPACEPORT];

/// The row `leg` reads, or `None` when the table has no row for the key (the
/// cartridge's search would run off the table's end there).
#[must_use]
pub fn flight_target(leg: FlightLeg, current_map: u16, world: u8) -> Option<FlightTarget> {
    let by_world = |table: &[(u16, u16, u16); 6]| {
        table
            .get(usize::from(world))
            .map(|&(map, start_x, start_y)| FlightTarget {
                map,
                start_x,
                start_y,
                previous_is_current: true,
            })
    };
    match leg {
        FlightLeg::Takeoff => {
            TAKEOFF
                .iter()
                .find(|row| row.0 == current_map)
                .map(|&(_, map, start_x, start_y)| FlightTarget {
                    map,
                    start_x,
                    start_y,
                    previous_is_current: true,
                })
        }
        FlightLeg::Transit => by_world(&TRANSIT),
        FlightLeg::Landing => by_world(&LANDING),
        FlightLeg::Return => {
            LANDING
                .iter()
                .find(|row| row.0 == current_map)
                .map(|&(map, start_x, start_y)| FlightTarget {
                    map,
                    start_x,
                    start_y,
                    previous_is_current: false,
                })
        }
    }
}

/// `MusicID_TakeOffLandeel` (`ps4.constants.asm`), written by both takeoffs.
const MUSIC_TAKE_OFF_LANDEEL: u8 = 0x9B;
/// `SpcSFXID_SpaceshipRadar` (`ps4.constants.asm:1051`).
const SFX_SPACESHIP_RADAR: u8 = 0xF8;
/// `SFXID_SpaceshipPropelled`, played as the long takeoff's counter reaches
/// `$E1` (`ps4.asm:134223`).
const SFX_SPACESHIP_PROPELLED: u8 = 0xE3;

/// How many ops a Cancel skips: from the op after the menu to the cancel leg.
const CANCEL_SKIP: u16 = 32;

/// `Pal_FadeIn` and `PalFadeOut_ClrSpriteTbl` each take 14 frames (measured on
/// the oracle, tape 35: six fades account for 84 of the flight's frames).
const FADE_FRAMES: u16 = 14;

/// The shared body of `$800D`, `Cutscene_InsideSpaceship` (`ps4.asm:155427`):
/// init, fade, `jmp loc_63BC4`. The two scenes that end in the same `jmp`
/// (`Cutscene_LeRoofAgain`, `Cutscene_FindingAirCastle`) inline it, and the
/// skips are relative so an inlined copy behaves the same at any offset.
///
/// ```text
/// menu              loc_63BC4               :133499-133666
/// takeoff           loc_64568 / loc_6488A   :134167 / :134376
///                   (the long takeoff is for the two spaceports only, :133667-133674)
/// planet screen     loc_64C1E               :134644-134712
/// transit, descent  loc_6472C..loc_64800    :134256-134322
/// landing           loc_6483A               :134322-134346
/// cancel            loc_63E5E               :133692-133726
/// ```
///
/// Frame counts: the loops and waits are the cartridge's `dbf` counts, and each
/// fade is the 14 frames the oracle measures. What the scene model has no op
/// for stays unmodelled and is listed in `docs/scenes/41_InsideSpaceship.md`
/// with the oracle's totals: the Speak button that cuts a flight short
/// (`:134228`, `:134315`), the camera pan `loc_5ABDC` runs before the descent,
/// `RefreshMap`'s own frames and the planet name's typing, and `loc_64C1E`'s
/// Rykros cutscene branch (`:134644-134655`).
pub(crate) const INSIDE_SPACESHIP_ROUTE: [SceneOp; 38] = [
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    // `SpcSFXID_SpaceshipRadar`, once the list is built (`ps4.asm:133537`).
    SceneOp::PlaySound {
        id: SFX_SPACESHIP_RADAR,
    },
    SceneOp::DestinationMenu {
        mask: DestinationMask::FlagTable,
        cancel_skip: CANCEL_SKIP,
    },
    // Both takeoffs write the track only when the saved byte differs.
    SceneOp::PlayMusicIfSavedDifferent {
        id: MUSIC_TAKE_OFF_LANDEEL,
    },
    SceneOp::SkipUnlessMap {
        maps: &SPACEPORT_MAPS,
        skip: 8,
    },
    // loc_64568: Pal_FadeIn, 419 iterations with the propulsion SFX near the
    // 224th, then loc_646DA's 60 frames of `Pal_IncreaseTone`.
    SceneOp::LoadFlightMap {
        leg: FlightLeg::Takeoff,
    },
    SceneOp::FadeIn,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::Wait { ticks: 224 },
    SceneOp::PlaySound {
        id: SFX_SPACESHIP_PROPELLED,
    },
    SceneOp::Wait { ticks: 195 },
    SceneOp::WaitFrames { frames: 60 },
    SceneOp::SkipOps { count: 4 },
    // loc_6488A: Pal_FadeIn, 228 iterations.
    SceneOp::LoadFlightMap {
        leg: FlightLeg::Takeoff,
    },
    SceneOp::FadeIn,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::Wait { ticks: 228 },
    // loc_6472C: fade out, 60 map updates, the planet screen's fade in, 300
    // frames (`d0 = $12B`, `:134670-134686`) and fade out, 60 more.
    SceneOp::FadeOut,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::Wait { ticks: 60 },
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::WaitFrames { frames: 300 },
    SceneOp::FadeOut,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::Wait { ticks: 60 },
    // Transit: Pal_FadeIn, then loc_64800's `$20000` down by `$200`,
    // including the terminal pass, then the fade out into the landing.
    SceneOp::LoadFlightMap {
        leg: FlightLeg::Transit,
    },
    SceneOp::FadeIn,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::Wait { ticks: 257 },
    SceneOp::FadeOut,
    SceneOp::WaitFrames {
        frames: FADE_FRAMES,
    },
    SceneOp::LoadFlightMap {
        leg: FlightLeg::Landing,
    },
    SceneOp::Return { value: 0 },
    // Cancel: back to the start cell of the map it was opened on.
    SceneOp::LoadFlightMap {
        leg: FlightLeg::Return,
    },
    SceneOp::Return { value: 0 },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_table_takes_the_first_set_flag_and_defaults_to_88() {
        let only = |wanted: u16| move |flag: Flag| flag == Flag::event(wanted);
        assert_eq!(
            destination_mask_byte(DestinationMask::FlagTable, only(8)),
            0x90
        );
        assert_eq!(
            destination_mask_byte(DestinationMask::FlagTable, only(0x99)),
            0xDC
        );
        assert_eq!(
            destination_mask_byte(DestinationMask::FlagTable, only(0xC6)),
            0xF8
        );
        assert_eq!(
            destination_mask_byte(DestinationMask::FlagTable, |_| false),
            0x88
        );
        // Order is the table's: Dark Force 3 beats AlysFound.
        assert_eq!(
            destination_mask_byte(DestinationMask::FlagTable, |flag| flag == Flag::event(8)
                || flag == Flag::event(0xC6)),
            0xF8
        );
        assert_eq!(
            destination_mask_byte(DestinationMask::Fixed(0x08), |_| true),
            0x08
        );
    }

    #[test]
    fn rows_are_the_mask_bits_minus_the_current_world() {
        // AlysFound at the Mota Spaceport: Motavia is current, so Zelan only.
        assert_eq!(destination_worlds(0x90, WORLD_MOTAVIA), vec![WORLD_ZELAN]);
        // At Zelan the same mask offers Motavia.
        assert_eq!(destination_worlds(0x90, WORLD_ZELAN), vec![WORLD_MOTAVIA]);
        // The sabotage's copy: Kuran.
        assert_eq!(destination_worlds(0x08, WORLD_ZELAN), vec![WORLD_KURAN]);
        // Everything unlocked, standing on Dezolis.
        assert_eq!(destination_worlds(0xFC, WORLD_DEZOLIS), vec![0, 2, 3, 4, 5]);
        // Bits 1 and 0 name no world.
        assert!(destination_worlds(0x03, WORLD_MOTAVIA).is_empty());
    }

    #[test]
    fn the_tables_resolve_the_cartridge_rows() {
        let takeoff = flight_target(FlightLeg::Takeoff, MOTA_SPACEPORT, 3).unwrap();
        assert_eq!(
            (takeoff.map, takeoff.start_x, takeoff.start_y),
            (0, 0x68, 0xB4)
        );
        let transit = flight_target(FlightLeg::Transit, 0, WORLD_ZELAN).unwrap();
        assert_eq!(
            (transit.map, transit.start_x, transit.start_y),
            (ZELAN_SPACE, 0x43, 0x1C)
        );
        let landing = flight_target(FlightLeg::Landing, ZELAN_SPACE, WORLD_ZELAN).unwrap();
        assert_eq!(
            (landing.map, landing.start_x, landing.start_y),
            (ZELAN, 0x3E, 0x5A)
        );
        let back = flight_target(FlightLeg::Return, MOTA_SPACEPORT, 0).unwrap();
        assert_eq!(
            (back.map, back.start_x, back.start_y),
            (MOTA_SPACEPORT, 0x3C, 0x28)
        );
        assert!(!back.previous_is_current);
        assert!(flight_target(FlightLeg::Takeoff, 0x123, 0).is_none());
        assert!(flight_target(FlightLeg::Transit, 0, 9).is_none());
    }

    use crate::collision::CollisionGrid;
    use crate::geom::{Cell, Direction};
    use crate::map::{FieldMap, MapId};
    use crate::scene::{ActorRef, SceneEffect, SceneInput, ScriptedActor};
    use crate::scene_runner::SceneRunner;
    use crate::state::{CharId, GameState};

    fn map_on(id: u16) -> FieldMap {
        FieldMap::new(
            MapId(id),
            CollisionGrid::filled(8, 8, 0).unwrap(),
            vec![],
            vec![],
        )
        .unwrap()
    }

    fn runner(ops: &'static [SceneOp]) -> (SceneRunner, GameState) {
        let mut state = GameState::new();
        state.set_party([Some(CharId(0)), None, None, None, None]);
        let cast = vec![ScriptedActor::new(
            ActorRef::PartyMember(0),
            Cell::new(1, 1),
            Direction::Down,
        )];
        (
            SceneRunner::new(ops, cast, crate::StepFrames::default()),
            state,
        )
    }

    /// Ticks until an effect satisfies `wanted`, feeding `input` once. Returns
    /// the effects seen.
    fn until(
        runner: &mut SceneRunner,
        map: &FieldMap,
        state: &mut GameState,
        input: SceneInput,
        wanted: impl Fn(&SceneEffect) -> bool,
    ) -> Vec<SceneEffect> {
        let mut seen = Vec::new();
        let mut input = input;
        for _ in 0..2000 {
            let effects = runner.tick(map, state, input);
            input = SceneInput::None;
            let done = effects.iter().any(&wanted);
            seen.extend(effects);
            if done {
                return seen;
            }
        }
        panic!("the wanted effect never came: {seen:?}");
    }

    fn is_menu(effect: &SceneEffect) -> bool {
        matches!(
            effect,
            SceneEffect::Presentation {
                op: SceneOp::DestinationMenu { .. }
            }
        )
    }

    fn is_leg(leg: FlightLeg) -> impl Fn(&SceneEffect) -> bool {
        move |effect| match effect {
            SceneEffect::MapRequested {
                op: SceneOp::LoadFlightMap { leg: got },
            } => *got == leg,
            _ => false,
        }
    }

    fn scene(name: &str) -> &'static [SceneOp] {
        crate::SCENES
            .iter()
            .find(|scene| scene.name == name)
            .unwrap()
            .ops
    }

    #[test]
    fn four_scenes_carry_the_menu_and_cancel_lands_on_the_return_leg() {
        let carriers: Vec<_> = crate::SCENES
            .iter()
            .filter(|scene| {
                scene
                    .ops
                    .iter()
                    .any(|op| matches!(op, SceneOp::DestinationMenu { .. }))
            })
            .collect();
        let mut names: Vec<_> = carriers.iter().map(|scene| scene.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "Cutscene_FindingAirCastle",
                "Cutscene_InsideSpaceship",
                "Cutscene_LeRoofAgain",
                "Cutscene_SpaceshipSabotage",
            ]
        );
        for scene in carriers {
            let at = scene
                .ops
                .iter()
                .position(|op| matches!(op, SceneOp::DestinationMenu { .. }))
                .unwrap();
            let SceneOp::DestinationMenu { cancel_skip, .. } = scene.ops[at] else {
                unreachable!()
            };
            assert_eq!(
                scene.ops[at + 1 + usize::from(cancel_skip)],
                SceneOp::LoadFlightMap {
                    leg: FlightLeg::Return
                },
                "{}: Cancel must land on the return leg",
                scene.name
            );
            assert_eq!(
                scene.ops[at + 2 + usize::from(cancel_skip)],
                SceneOp::Return { value: 0 },
                "{}",
                scene.name
            );
        }
    }

    #[test]
    fn the_menu_blocks_until_answered_and_cancel_skips_the_flight() {
        let map = map_on(MOTA_SPACEPORT);
        let (mut run, mut state) = runner(scene("Cutscene_InsideSpaceship"));
        until(&mut run, &map, &mut state, SceneInput::None, is_menu);
        // Nothing moves the scene while the menu is up.
        for _ in 0..30 {
            let effects = run.tick(&map, &mut state, SceneInput::None);
            assert!(effects.is_empty(), "{effects:?}");
        }
        let seen = until(
            &mut run,
            &map,
            &mut state,
            SceneInput::DestinationCancelled,
            is_leg(FlightLeg::Return),
        );
        assert!(
            !seen.iter().any(is_leg(FlightLeg::Takeoff)),
            "a Cancel flies nowhere: {seen:?}"
        );
    }

    #[test]
    fn a_confirm_flies_the_long_takeoff_from_a_spaceport_and_the_short_one_elsewhere() {
        // Ticks from the takeoff's map load to the transit leg, summed from the
        // cartridge's loops and the 14-frame fades: 14 + 419 + 60 (tone) + 14 + 60
        // + 14 + 300 + 14 + 60 from a spaceport, 14 + 228 + 14 + 60 + 14 + 300 +
        // 14 + 60 from anywhere else.
        for (map_id, expected_at_least, expected_below) in
            [(MOTA_SPACEPORT, 955, 995), (ZELAN, 704, 744)]
        {
            let map = map_on(map_id);
            let (mut run, mut state) = runner(scene("Cutscene_InsideSpaceship"));
            until(&mut run, &map, &mut state, SceneInput::None, is_menu);
            let seen = until(
                &mut run,
                &map,
                &mut state,
                SceneInput::DestinationChosen,
                is_leg(FlightLeg::Takeoff),
            );
            assert!(!seen.iter().any(is_leg(FlightLeg::Return)));
            run.tick(&map, &mut state, SceneInput::MapLoaded);
            let mut ticks = 0u16;
            loop {
                let effects = run.tick(&map, &mut state, SceneInput::None);
                ticks += 1;
                if effects.iter().any(is_leg(FlightLeg::Transit)) {
                    break;
                }
                assert!(ticks < 3000);
            }
            assert!(
                (expected_at_least..expected_below).contains(&ticks),
                "{map_id:#x}: {ticks} ticks"
            );
        }
    }

    #[test]
    fn the_sabotage_menu_cancels_to_the_return_leg_and_confirms_into_its_own_flight() {
        let map = map_on(ZELAN);
        let (mut run, mut state) = runner(scene("Cutscene_SpaceshipSabotage"));
        until(&mut run, &map, &mut state, SceneInput::None, is_menu);
        until(
            &mut run,
            &map,
            &mut state,
            SceneInput::DestinationCancelled,
            is_leg(FlightLeg::Return),
        );
        let (mut run, mut state) = runner(scene("Cutscene_SpaceshipSabotage"));
        until(&mut run, &map, &mut state, SceneInput::None, is_menu);
        until(
            &mut run,
            &map,
            &mut state,
            SceneInput::DestinationChosen,
            |effect| {
                matches!(
                    effect,
                    SceneEffect::MapRequested {
                        op: SceneOp::LoadMap { map: 0x18C, .. }
                    }
                )
            },
        );
    }

    #[test]
    fn skips_are_relative_and_a_map_test_skips_when_not_listed() {
        static OPS: [SceneOp; 6] = [
            SceneOp::SkipUnlessMap {
                maps: &[7],
                skip: 2,
            },
            SceneOp::SetWorldIndex { world: 1 },
            SceneOp::SetWorldIndex { world: 2 },
            SceneOp::SkipOps { count: 1 },
            SceneOp::SetWorldIndex { world: 3 },
            SceneOp::SetWorldIndex { world: 4 },
        ];
        let worlds = |map_id: u16| {
            let map = map_on(map_id);
            let (mut run, mut state) = runner(&OPS);
            run.tick(&map, &mut state, SceneInput::None)
                .into_iter()
                .filter_map(|effect| match effect {
                    SceneEffect::WorldIndexSet { world } => Some(world),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        // Listed map: no skip, then SkipOps jumps over world 3.
        assert_eq!(worlds(7), vec![1, 2, 4]);
        // Unlisted map: skips worlds 1 and 2, then SkipOps over world 3.
        assert_eq!(worlds(9), vec![4]);
    }
}
