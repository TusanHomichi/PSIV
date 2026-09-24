//! Real-pack POISON (`$11`) dispatch: a live Caterpillr formation, the
//! wind-up cue and the status the field carries afterwards.
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders, status};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn caterpillar_poison_replaces_its_attack_in_the_real_formation() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x2B,
        Cell::new(17, 52),
        Direction::Up,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        None,
        None,
    ]);
    // Constructed durability fixture, unrelated to the connected native save:
    // three Caterpillrs hit hard, and the run needs several rounds for the
    // weighted ability roll to reach `$11`.
    for character in [1, 0, 2] {
        let stats = game.roster_mut().get_mut(CharId(character)).unwrap();
        stats.max_hp = 400;
        stats.curr_hp = 400;
        assert_eq!(stats.status, 0, "the fixture starts with no ailment");
    }
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x2B,
                map_index_2: 0,
                char_x: 17 * 16,
                char_y: 52 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.set_rng_seed(0x0BAD_F00D);
    // Pack formation 0x39 is enemy id 32 three times over: the Caterpillr trio
    // whose record 17 is `$11` in four of its eight ability slots.
    rt.start_battle(0x39, rt.battle_party()).unwrap();
    let mut casts = 0u32;
    let mut poisoned: Option<CharId> = None;
    for _ in 0..8 {
        let timeline = rt
            // DEFEND halves the physical hits without touching the efess factor
            // the poison roll is scaled by.
            .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        assert!(
            !timeline
                .events
                .iter()
                .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. })),
            "{:?}",
            timeline.events
        );
        for (index, event) in timeline.events.iter().enumerate() {
            let BattleEvent::EnemySkillUsed {
                actor,
                skill: 17,
                name,
            } = event
            else {
                continue;
            };
            assert_eq!(name, "POISON");
            casts += 1;
            assert!(
                timeline
                    .sounds
                    .iter()
                    .any(|s| s.event_index == index && s.id == 0xD8),
                "BattleObj_Poison's wind-up writes EnemyAttack4: {:?}",
                timeline.sounds
            );
            assert!(
                !timeline.events.iter().any(
                    |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if attacker == actor)
                ),
                "POISON must not also attack: {:?}",
                timeline.events
            );
            if let Some(BattleEvent::StatusInflicted {
                target,
                status: inflicted,
                ..
            }) = timeline.events.get(index + 1)
            {
                assert_eq!(*inflicted, status::POISONED);
                poisoned = rt
                    .battle_roster()
                    .and_then(|roster| roster.get(*target))
                    .and_then(|fighter| fighter.character)
                    .map(CharId);
            }
        }
        if casts > 0 && poisoned.is_some() {
            break;
        }
    }
    assert!(casts > 0, "the fixed seed must reach the real record 17");
    let poisoned = poisoned.expect("POISON must apply at least once");
    let mut escaped = false;
    for _ in 0..30 {
        if rt
            .battle_round(&RoundOrders::Run)
            .unwrap()
            .contains(&BattleEvent::Ended {
                outcome: Outcome::Escaped,
            })
        {
            escaped = true;
            break;
        }
    }
    assert!(escaped);
    let purse = rt.game().money();
    rt.finish_battle_for_outcome(Outcome::Escaped, 0);
    rt.return_to_field();
    assert_eq!(rt.game().money(), purse, "an escape pays no reward");
    let carried = rt.game().roster().get(poisoned).unwrap();
    assert_ne!(
        carried.status & status::POISONED,
        0,
        "the field record must carry the poison the battle applied"
    );
    assert_eq!(
        carried.status & (status::DEAD | status::ANDROID_DEAD),
        0,
        "the poisoned member survived, so the bit is the ability's and not a death clear"
    );
    let before = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-poison-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let loaded = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &dir,
        0,
        StepFrames::default(),
    )
    .unwrap();
    assert_eq!(loaded.game().snapshot(), before);
    std::fs::remove_dir_all(dir).unwrap();
}
