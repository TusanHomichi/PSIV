use super::*;

#[test]
fn bit_order_is_most_significant_first() {
    // The cartridge's `7 - (id & 7)`: flag 0 is bit 7 of byte 0.
    let mut state = GameState::new();
    state.set(Flag::event(0)).unwrap();
    assert_eq!(state.snapshot().event_flags[0], 0b1000_0000);

    let mut state = GameState::new();
    state.set(Flag::event(7)).unwrap();
    assert_eq!(state.snapshot().event_flags[0], 0b0000_0001);

    let mut state = GameState::new();
    state.set(Flag::event(8)).unwrap();
    assert_eq!(state.snapshot().event_flags[1], 0b1000_0000);
}

#[test]
fn the_three_banks_are_independent() {
    let mut state = GameState::new();
    state.set(Flag::chest(8)).unwrap();
    assert!(state.is_set(Flag::chest(8)));
    assert!(state.is_clear(Flag::event(8)));
    assert!(state.is_clear(Flag::town(8)));
}

#[test]
fn a_chest_flag_is_the_same_bit_as_the_extended_event_flag_above_it() {
    // `$F120` is one bank with two name-spaces on it. The chest system
    // reaches it through the same door as the extended event flags — three
    // call sites, all `0x05762E`/`0x057670`, in the module docs.
    assert_eq!(Flag::chest(0x18), Flag::event(0x118));
    assert_eq!(Flag::chest(0).bank, FlagBank::Event);

    let mut state = GameState::new();
    state.set(Flag::chest(0x18)).unwrap();
    assert!(
        state.is_set(Flag::event(0x118)),
        "opening a chest is visible as an extended event flag"
    );

    // And back, including the clear — story code reads these bits.
    let mut state = GameState::new();
    state.set(Flag::event(0x118)).unwrap();
    assert!(state.is_set(Flag::chest(0x18)));
    state.clear(Flag::chest(0x18)).unwrap();
    assert!(state.is_clear(Flag::event(0x118)));
}

#[test]
fn a_temp_flag_is_independent_of_the_chest_flag_of_that_id() {
    // The correction that cost the most: `$F140` is the temp bank, not the
    // chest bank. An earlier revision aliased these and was wrong in both
    // directions. Id 8 is `TempEveFlag_BioPlantAlarm` and
    // `ChestFlag_Alshline`; they never touch.
    let mut state = GameState::new();
    state.set(Flag::temp(8)).unwrap();
    assert!(state.is_clear(Flag::chest(8)), "different array");
    assert!(
        state.is_clear(Flag::event(8)),
        "and not an event flag either"
    );

    state.set(Flag::chest(8)).unwrap();
    assert!(state.is_set(Flag::temp(8)), "still set, untouched");

    state.clear(Flag::chest(8)).unwrap();
    assert!(state.is_set(Flag::temp(8)), "clearing one leaves the other");

    assert_ne!(Flag::temp(0x13), Flag::chest(0x13));
    assert_eq!(Flag::temp(8).bank, FlagBank::Temp);
}

#[test]
fn the_eleven_preloaded_chests_read_as_open_at_a_new_game() {
    // The initialiser pre-sets eleven `$F120` bits, and every one is a real
    // chest's flag. Nothing in the cartridge clears a `$F120` bit — the
    // clear door's only caller is dead code — so those eleven chests are
    // unlootable from the first frame and stay that way.
    const PRELOADED: [u16; 11] = [
        0x27, 0x28, 0x2A, 0x34, 0x38, 0x50, 0x5B, 0x5D, 0x6B, 0x78, 0xA7,
    ];
    let mut state = GameState::new();
    for id in PRELOADED {
        state.set(Flag::chest(id)).unwrap();
    }
    for id in PRELOADED {
        assert!(state.is_set(Flag::chest(id)), "chest ${id:02X}");
        assert!(state.is_set(Flag::event(0x100 + id)));
        assert!(state.is_clear(Flag::temp(id)), "temp ${id:02X} untouched");
    }
}

#[test]
fn the_banks_hold_what_the_ram_map_gives_them() {
    // Four banks, not five. `$F140` runs to `$F160` where town starts, so
    // it is 32 bytes rather than the clone's 22 + 10 split.
    assert_eq!(FlagBank::Event.capacity(), 512, "$F100 + $F120");
    assert_eq!(FlagBank::Temp.capacity(), 256, "$F140..$F160");
    assert_eq!(FlagBank::Town.capacity(), 128, "$F160..$F170");

    let mut state = GameState::new();
    assert!(state.set(Flag::chest(255)).is_ok());
    assert!(matches!(
        state.set(Flag::chest(256)),
        Err(MapError::FlagOutOfRange { .. })
    ));
    assert!(state.set(Flag::temp(255)).is_ok());
    assert!(matches!(
        state.set(Flag::temp(256)),
        Err(MapError::FlagOutOfRange { .. })
    ));
    assert!(state.set(Flag::event(511)).is_ok());
    assert!(state.set(Flag::town(127)).is_ok());
    assert!(matches!(
        state.set(Flag::town(128)),
        Err(MapError::FlagOutOfRange { .. })
    ));
}

fn item_chest(flag: u8, item: u8) -> Chest {
    Chest {
        cell: crate::geom::Cell::new(4, 4),
        flag,
        contents: ChestContents::Item(item),
        white: false,
        index: 0,
    }
}

#[test]
fn opening_a_chest_grants_the_item_and_sets_its_flag() {
    let mut state = GameState::new();
    let chest = item_chest(24, 0x7D);
    assert!(!state.chest_is_open(&chest));

    assert_eq!(
        state.open_chest(&chest),
        ChestOutcome::Took {
            item: 0x7D,
            slot: 0
        }
    );
    assert_eq!(state.inventory().get(0), Some(0x7D));
    assert!(state.chest_is_open(&chest), "the flag records it");
}

#[test]
fn a_chest_stays_open_and_cannot_be_looted_twice() {
    // Re-entering the map rebuilds the chest from the same flag, so this is
    // also what makes it draw open.
    let mut state = GameState::new();
    let chest = item_chest(24, 0x7D);
    state.open_chest(&chest);

    assert_eq!(state.open_chest(&chest), ChestOutcome::AlreadyOpen);
    assert_eq!(state.inventory().occupied(), 1, "no second copy");
    assert!(state.chest_is_open(&chest));
}

#[test]
fn a_meseta_chest_pays_in_hundreds() {
    let mut state = GameState::new();
    let chest = Chest {
        contents: ChestContents::Meseta(400),
        ..item_chest(25, 0)
    };
    assert_eq!(
        state.open_chest(&chest),
        ChestOutcome::Meseta { amount: 400 }
    );
    assert_eq!(state.money(), 400);
    assert!(state.chest_is_open(&chest));
    assert_eq!(state.inventory().occupied(), 0, "meseta takes no slot");
}

#[test]
fn a_full_inventory_leaves_the_chest_shut_until_the_swap() {
    // The grant happens before the flag is set, so a chest that could not
    // give up its contents is still closed and can be opened again later.
    let mut state = GameState::new();
    for id in 1..=40 {
        state.inventory_mut().add(id).unwrap();
    }
    let chest = item_chest(24, 0x7D);

    assert_eq!(state.open_chest(&chest), ChestOutcome::Full { item: 0x7D });
    assert!(!state.chest_is_open(&chest), "still shut");
    assert!(!state.inventory().contains(0x7D), "and nothing was granted");

    assert_eq!(
        state.complete_chest_swap(&chest, 7).unwrap(),
        ChestOutcome::Took {
            item: 0x7D,
            slot: 7
        }
    );
    assert_eq!(state.inventory().get(7), Some(0x7D));
    assert!(state.chest_is_open(&chest));
}

#[test]
fn every_story_gated_id_is_a_real_chests_flag() {
    // These are the only ids in the ROM that reach the `$F120` test door with
    // a literal immediate. Every one is a treasure chest's flag.
    const STORY_GATED: [(u16, &str); 11] = [
        (0x08, "TonoeBasement_B3 EclpsTorch"),
        (0x09, "LadeaTower_F5 FradeMantl"),
        (0x0A, "MachineCenter_B1 Canceller"),
        (0x0B, "Zelan_F1 PalmaRing"),
        (0x0C, "AirCastleInner_B1_Part3 AeroPrism"),
        (0x0D, "SoldiersTemple RepairKit"),
        (0xA1, "StrengthTower_F4 MotaRing"),
        (0xA2, "StrengthTower_F4 DezoRing"),
        (0xA3, "StrengthTower_F4 RykrRing"),
        (0xA4, "CourageTower_F4 AlgoRing"),
        (0xA5, "CourageTower_F4 MahlayRing"),
    ];

    let mut state = GameState::new();
    for (id, chest) in STORY_GATED {
        state.set(Flag::chest(id)).unwrap();
        assert!(state.is_set(Flag::event(0x100 + id)), "{chest}");
        assert!(state.is_clear(Flag::temp(id)), "{chest}: not a temp flag");
    }
}

#[test]
fn the_two_id_conventions_land_on_the_same_bit() {
    // The off-by-$100 killer. The pack emits the preloaded set as combined
    // ids (`0x127`), while a chest record names the same bit as `0x27`.
    for n in [0u16, 1, 7, 8, 0x27, 0x78, 0xA7, 0xFF] {
        let from_chest = Flag::chest(n);
        let from_combined = Flag::event(0x100 + n);
        assert_eq!(from_chest, from_combined, "id ${n:02X}");

        let mut a = GameState::new();
        a.set(from_chest).unwrap();
        let mut b = GameState::new();
        b.set(from_combined).unwrap();
        assert_eq!(
            a.snapshot().event_flags,
            b.snapshot().event_flags,
            "id ${n:02X}: same byte, same bit"
        );

        let byte = 32 + (n as usize >> 3);
        let bit = 7 - (n & 7);
        assert_eq!(
            a.snapshot().event_flags[byte],
            1u8 << bit,
            "id ${n:02X} at $F1{:02X} bit {bit}",
            0x20 + (n >> 3)
        );
    }

    assert_ne!(Flag::chest(0x27), Flag::event(0x27));
}

#[test]
fn the_snapshot_carries_four_banks() {
    let snapshot = GameState::new().snapshot();
    assert_eq!(snapshot.event_flags.len(), 64);
    assert_eq!(snapshot.temp_flags.len(), 32);
    assert_eq!(snapshot.town_flags.len(), 16);
    assert_eq!(snapshot.inventory.len(), 40, "$F410 to $F438");

    let mut state = GameState::new();
    state.set(Flag::chest(0)).unwrap();
    assert_eq!(state.snapshot().event_flags[32], 0b1000_0000);

    let mut state = GameState::new();
    state.set(Flag::temp(0)).unwrap();
    assert_eq!(state.snapshot().temp_flags[0], 0b1000_0000);

    let mut state = GameState::new();
    state.set(Flag::temp(22 * 8)).unwrap();
    assert_eq!(state.snapshot().temp_flags[22], 0b1000_0000);
}

#[test]
fn the_event_bank_spans_the_extended_range() {
    let mut state = GameState::new();
    state.set(Flag::event(0x100)).unwrap();
    assert!(state.is_set(Flag::event(0x100)));
    assert!(state.is_clear(Flag::event(0)));
    assert_eq!(state.snapshot().event_flags[32], 0b1000_0000);
}

#[test]
fn ids_past_a_banks_capacity_are_rejected_and_read_clear() {
    let mut state = GameState::new();
    assert!(matches!(
        state.set(Flag::town(128)),
        Err(MapError::FlagOutOfRange { .. })
    ));
    assert!(state.is_clear(Flag::town(128)));
    assert!(state.set(Flag::town(127)).is_ok());
}

#[test]
fn setting_and_clearing_round_trips() {
    let mut state = GameState::new();
    for id in 0..64 {
        state.set(Flag::event(id)).unwrap();
    }
    for id in 0..64 {
        assert!(state.is_set(Flag::event(id)), "flag {id}");
    }
    for id in (0..64).step_by(2) {
        state.clear(Flag::event(id)).unwrap();
    }
    for id in 0..64 {
        assert_eq!(state.is_set(Flag::event(id)), id % 2 == 1, "flag {id}");
    }
}

#[test]
fn party_counts_to_the_first_empty_slot() {
    let mut state = GameState::new();
    assert_eq!(state.party_len(), 0);

    state.set_party([
        Some(CharId(1)),
        Some(CharId(2)),
        Some(CharId(3)),
        None,
        None,
    ]);
    assert_eq!(state.party_len(), 3);

    state.set_party([Some(CharId(1)), None, Some(CharId(3)), None, None]);
    assert_eq!(state.party_len(), 1);
    assert_eq!(state.party_slot(2), Some(CharId(3)));
}

#[test]
fn the_alys_found_write_puts_alys_in_front() {
    // Event_AlysFound writes (CharID_Alys << 8) | CharID_Chaz as a word to
    // Current_Party_Slots: big-endian, so the high byte lands in slot 1.
    const CHAZ: u8 = 0;
    const ALYS: u8 = 1;
    let word: u16 = (u16::from(ALYS) << 8) | u16::from(CHAZ);

    let mut state = GameState::new();
    state
        .set_party_slot(0, Some(CharId((word >> 8) as u8)))
        .unwrap();
    state.set_party_slot(1, Some(CharId(word as u8))).unwrap();

    assert_eq!(state.party_slot(0), Some(CharId(ALYS)), "Alys leads");
    assert_eq!(state.party_slot(1), Some(CharId(CHAZ)));
    assert_eq!(state.party_len(), 2);
}

#[test]
fn snapshots_round_trip() {
    let mut state = GameState::new();
    state.set(Flag::event(0x42)).unwrap();
    state.set(Flag::chest(0x0D)).unwrap();
    state.set(Flag::temp(0x1B)).unwrap();
    state.set(Flag::town(3)).unwrap();
    state.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
    state.add_money(400);

    let restored = GameState::from_snapshot(&state.snapshot());
    assert_eq!(restored.money(), 400);
    assert_eq!(restored, state);
}
