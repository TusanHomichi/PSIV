//! Who the field accepts as the caster of a TECH or SKILL
//! (`Runtime::camp_caster_refusal`, #483): `loc_5FF98` refuses a fallen or a
//! paralyzed member and nothing else (field TECH ignores seal). The camp, the
//! travel menu and the campaign runner's recovery planner all ask this one
//! rule. The cases need the runtime pack and skip with a message without one.

use std::path::Path;

use psiv_core::battle::status;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Session;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
/// The Academy corridor the camp fixtures use: no scene, no counter.
const FIELD: (u16, u16, u16) = (0x13, 48, 19);

/// Chaz and Rika in the corridor, Chaz with status `bits`.
fn session_with(bits: u8) -> Option<Session> {
    let path = Path::new(PACK);
    if !path.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let data = GameData::load(path).expect("pack loads");
    let files = BattleFiles::load(path).expect("battle files load");
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .new_game()
        .expect("START");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), Some(CharId(5)), None, None, None]);
    for id in [CharId(0), CharId(5)] {
        let member = game.roster_mut().get_mut(id).expect("a party member");
        member.curr_hp = member.max_hp;
        member.status = 0;
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).expect("Chaz");
    chaz.status = bits;
    if bits & status::DEAD != 0 {
        chaz.curr_hp = 0;
    }
    Some(
        Session::start(data)
            .with_battles(files)
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 1,
                    map_index_2: 0,
                    map_index: FIELD.0,
                    char_x: FIELD.1 * 16,
                    char_y: FIELD.2 * 16,
                },
            })
            .expect("the corridor loads"),
    )
}

#[test]
fn the_field_refuses_a_fallen_or_paralyzed_caster_only() {
    for (bits, refusal) in [
        (0, None),
        (status::DEAD, Some("CASTER IS DOWN")),
        (status::PARALYZED, Some("CASTER IS PARALYZED")),
        // Death is tested first, as `loc_5FF98` does.
        (status::DEAD | status::PARALYZED, Some("CASTER IS DOWN")),
        (status::TECH_SEALED, None),
        (status::POISONED, None),
        (status::ASLEEP, None),
    ] {
        let Some(session) = session_with(bits) else {
            return;
        };
        let runtime = session.runtime();
        assert_eq!(
            runtime.camp_caster_refusal(0),
            refusal,
            "Chaz with status {bits:#04x}"
        );
        assert_eq!(runtime.camp_caster_refusal(1), None, "Rika stands free");
        assert_eq!(
            runtime.camp_caster_refusal(2),
            Some("NO PARTY MEMBER"),
            "an empty slot casts nothing"
        );
    }
}
