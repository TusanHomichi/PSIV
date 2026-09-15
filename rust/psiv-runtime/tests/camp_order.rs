//! ORDER changes targeting slots and field art without touching the roster.
use psiv_core::battle::{Side, SliceRolls, choose_target, targetable_party};
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;

#[test]
fn camp_order_is_atomic_preserves_positions_and_stats_and_survives_save() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    let mut game = GameState::new();
    game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(4)),
        None,
    ]);
    let mut rt = Runtime::from_save(
        data.clone(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0xA7,
                map_index_2: 0xA6,
                char_x: 68 * 16,
                char_y: 66 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    let before = rt.game().snapshot();
    let positions = rt.members();
    for invalid in [
        &[][..],
        &[4, 1, 0][..],
        &[4, 1, 0, 0][..],
        &[4, 1, 0, 5][..],
        &[4, 1, 0, 2, 5][..],
    ] {
        assert!(rt.order_camp_party(invalid).is_err());
        assert_eq!(rt.game().snapshot(), before);
    }
    assert_eq!(
        rt.order_camp_party(&[4, 1, 0, 2]).unwrap(),
        vec![RuntimeEvent::PartyChanged]
    );
    let mut expected = before.clone();
    expected.party = [4, 1, 0, 2, 0xFF];
    assert_eq!(rt.game().snapshot(), expected);
    assert_eq!(rt.members(), positions);
    assert!(rt.order_camp_party(&[4, 1, 0, 2]).unwrap().is_empty());
    let dir = std::env::temp_dir().join(format!("psiv-order-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let mut restored = Runtime::load_slot(data, &dir, 0, StepFrames::default()).unwrap();
    restored.enable_battles(&files).unwrap();
    assert_eq!(restored.game().snapshot(), expected);
    restored
        .start_battle(0xC1, restored.battle_party())
        .unwrap();
    let roster = restored.battle_roster().unwrap();
    assert_eq!(
        roster
            .side(Side::Party)
            .map(|f| f.character.unwrap())
            .collect::<Vec<_>>(),
        vec![4, 1, 0, 2]
    );
    let candidates = targetable_party(roster);
    let mut counts = [0; 4];
    for byte in 0..=255 {
        let target = choose_target(&candidates, &mut SliceRolls::new(&[byte])).unwrap();
        counts[target.slot()] += 1;
    }
    assert_eq!(counts, [103, 76, 51, 26]);
    assert!(restored.order_camp_party(&[1, 0, 2, 4]).is_err());
    assert_eq!(restored.game().snapshot(), expected);
    std::fs::remove_dir_all(dir).unwrap();
}
