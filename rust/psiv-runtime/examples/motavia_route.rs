//! Scout the next connected route from the native Academy save. No relocations
//! or edited stats: field recovery and combat are ordinary runtime commands.
mod support;
use psiv_core::{Cell, Direction, Input, StepFrames};
use psiv_data::{BattleFiles, DialogueSet, GameData};
use psiv_runtime::{CampAbilityKind, CampUseResult, Runtime};
use std::path::Path;
use support::Walk;

fn heal(route: &mut Walk) {
    for target in 0..route.rt.camp_state().party.len() {
        for _ in 0..20 {
            let party = route.rt.camp_state().party;
            let member = &party[target];
            if member.current_hp * 4 >= member.max_hp * 3 {
                break;
            }
            let caster = party
                .iter()
                .filter(|c| c.status & 6 == 0)
                .find(|c| {
                    route
                        .rt
                        .camp_abilities(c.party_slot, CampAbilityKind::Technique)
                        .iter()
                        .any(|a| a.id == 24 && a.remaining >= u16::from(a.cost))
                })
                .expect("route needs a healer or an inn");
            let result = route.rt.use_camp_ability(
                CampAbilityKind::Technique,
                caster.party_slot,
                24,
                member.party_slot,
            );
            println!("CAMP {result:?}");
            assert!(matches!(result, CampUseResult::Used { .. }));
        }
    }
}

fn main() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let save_dir = std::env::var("PSIV_ROUTE_CONTINUE_DIR")
        .expect("PSIV_ROUTE_CONTINUE_DIR must name the native save directory");
    let mut rt = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        Path::new(&save_dir),
        0,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&BattleFiles::load(pack).unwrap())
        .unwrap();
    let mut route = Walk {
        rt,
        dialogue: DialogueSet::load(pack).unwrap(),
        ticks: 0,
        battles: 0,
        heal_in_battle: true,
    };
    route.checkpoint("continued native Academy save");
    heal(&mut route);
    // The Edge's late-story warp is absent at this point in the campaign.
    route.walk_to(Cell::new(61, 100));
    assert_eq!(route.rt.map_id().0, 0x1D);
    route.checkpoint("Mile");
    route.walk_to(Cell::new(20, 50));
    assert_eq!(route.rt.map_id().0, 0);
    route.walk_to(Cell::new(99, 82));
    assert_eq!(route.rt.map_id().0, 0x24);
    route.checkpoint("Zema");
    heal(&mut route);
    route.walk_to(Cell::new(31, 11));
    assert_eq!(route.rt.map_id().0, 0x2B);
    route.checkpoint("Birth Valley");
    route.walk_to(Cell::new(33, 15));
    assert_eq!(route.rt.map_id().0, 0x2C);
    route.checkpoint("Birth Valley B1");
    route.walk_to(Cell::new(23, 17));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    route.checkpoint("Professor Holt");
    if let Some(dir) = std::env::var_os("PSIV_ROUTE_SAVE_DIR") {
        route.rt.save_slot(Path::new(&dir), 0).unwrap();
    }
}
