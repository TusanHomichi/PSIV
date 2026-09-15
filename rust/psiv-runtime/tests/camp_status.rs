//! All STATUS identities come from the extracted character records.
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

#[test]
fn every_character_has_original_profession_age_and_distinct_status_art() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").exists() {
        return;
    }
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    let mut rt = Runtime::new(
        data.clone(),
        0x47,
        Cell::new(30, 45),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    let expected = [
        ("HUNTER", Some(16)),
        ("HUNTER", None),
        ("SCHOLAR", Some(24)),
        ("WIZARD", None),
        ("MOTAVIAN", Some(19)),
        ("NUMAN", Some(1)),
        ("ANDROID", Some(324)),
        ("ANDROID", Some(998)),
        ("PRIEST", Some(85)),
        ("ESPER", Some(18)),
        ("SCHOLAR", Some(39)),
    ];
    for (id, (profession, age)) in expected.into_iter().enumerate() {
        let mut game = GameState::from_snapshot(&rt.game().snapshot());
        game.set_party([Some(CharId(id as u8)), None, None, None, None]);
        let save = RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 480,
                char_y: 720,
            },
        };
        let mut selected = Runtime::from_save(data.clone(), save, StepFrames::default()).unwrap();
        selected.enable_battles(&files).unwrap();
        let camp = selected.camp_state();
        assert_eq!(camp.party[0].profession, profession);
        assert_eq!(camp.party[0].age, age);
        let path = &files.characters.characters[id]
            .status_portrait
            .as_ref()
            .unwrap()
            .png;
        assert_eq!(path, &format!("battle/status_portraits/{id:02x}.png"));
        assert!(pack.join(path).is_file());
    }
}
