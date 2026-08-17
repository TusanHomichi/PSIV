use super::*;

const ACADEMY: CameraBounds = CameraBounds {
    width: 1024,
    height: 512,
    edges: CameraEdges::Clamped,
};

fn px(v: i32) -> i32 {
    v * ONE_PIXEL
}

#[test]
fn the_home_offset_is_the_two_numbers_the_rom_states_twice() {
    // FieldObj_Camera*Pos_FG thresholds, less the sprite origin.
    assert_eq!(HOME_X, 152);
    assert_eq!(HOME_Y, 88);
    // Event_MoveCamera subtracts exactly these to turn a subject position
    // into a camera position.
    assert_eq!(HOME_X, 0x98);
    assert_eq!(HOME_Y, 0x58);
}

#[test]
fn a_placed_camera_puts_the_driver_at_its_home_offset() {
    let driver = Driver {
        x: px(800),
        y: px(240),
    };
    let camera = Camera::placed_on(driver, ACADEMY);
    assert_eq!(camera.position(), (800 - HOME_X, 240 - HOME_Y));
    assert_eq!(
        camera.sprite_pos(driver.x, driver.y),
        (THRESHOLD_X, THRESHOLD_Y)
    );
}

#[test]
fn a_placed_camera_clamps_into_a_small_map_instead_of_showing_the_void() {
    let driver = Driver::at_cell(Cell::new(1, 1));
    let camera = Camera::placed_on(driver, ACADEMY);
    assert_eq!(camera.position(), (0, 0), "clamped at the top-left corner");
}

#[test]
fn following_holds_the_driver_at_the_threshold_rather_than_centring_it() {
    // y 300 keeps the camera off both clamps, so this is the latch alone.
    let mut driver = Driver {
        x: px(800),
        y: px(300),
    };
    let mut camera = Camera::placed_on(driver, ACADEMY);
    assert_eq!(camera.sprite_pos(driver.x, driver.y).1, THRESHOLD_Y);
    // Move first, then tick, so the driver the assertions read is the one
    // the last tick actually saw.
    for _ in 0..20 {
        driver.y -= px(2);
        camera.tick(driver);
    }
    assert_eq!(
        camera.sprite_pos(driver.x, driver.y).1,
        THRESHOLD_Y,
        "held at the threshold, not re-centred"
    );
    // It really did scroll rather than sit still.
    assert_eq!(camera.position().1, 300 - HOME_Y - 2 * 20);
}

#[test]
fn the_camera_tracks_the_driver_with_no_frame_of_lag() {
    // The oracle's camera columns hold `leader - camera` at exactly
    // (152, 88) on all 1563 field-control frames of tape 02 — one distinct
    // offset, no lag anywhere. That is only true if the commit lands after
    // the latch inside a frame, and it is the sharpest available check on
    // the ordering.
    let mut driver = Driver {
        x: px(800),
        y: px(300),
    };
    let mut camera = Camera::placed_on(driver, ACADEMY);
    for _ in 0..24 {
        driver.y -= px(2);
        camera.tick(driver);
        let (cx, cy) = camera.position();
        assert_eq!(
            ((driver.x >> 16) - cx, (driver.y >> 16) - cy),
            (HOME_X, HOME_Y),
            "the offset never varies while the camera is unclamped"
        );
    }
}

#[test]
fn a_driver_short_of_the_threshold_walks_across_a_still_camera() {
    // Placed at the top-left clamp, the driver is above its home offset, so
    // walking down does not move the camera until it reaches the threshold.
    let mut driver = Driver {
        x: px(160),
        y: px(16),
    };
    let mut camera = Camera::placed_on(driver, ACADEMY);
    let before = camera.position();
    for _ in 0..8 {
        driver.y += px(2);
        camera.tick(driver);
    }
    assert_eq!(camera.position(), before, "camera has not moved");
    assert!(
        camera.sprite_pos(driver.x, driver.y).1 > SPRITE_ORIGIN,
        "but the driver has walked down the screen"
    );
}

#[test]
fn a_clamped_edge_stops_the_camera_dead_however_long_the_driver_walks() {
    let mut driver = Driver {
        x: px(800),
        y: px(120),
    };
    let mut camera = Camera::placed_on(driver, ACADEMY);
    for _ in 0..64 {
        camera.tick(driver);
        driver.y -= px(2);
    }
    assert_eq!(camera.position().1, 0, "pinned at the top of the map");
    // The clamp is re-applied every frame rather than latched once, so a
    // driver that keeps walking never drags the view off the map.
    for _ in 0..64 {
        camera.tick(driver);
        driver.y -= px(2);
        assert_eq!(camera.position().1, 0);
    }
}

#[test]
fn a_wrapping_camera_comes_back_around_the_overworld() {
    let bounds = CameraBounds {
        width: 2048,
        height: 2048,
        edges: CameraEdges::Wrapping,
    };
    let mut camera = Camera::new(bounds);
    camera.set_position(16, 16);
    let mut driver = Driver {
        x: px(16 + HOME_X),
        y: px(16 + HOME_Y),
    };
    // A camera parked rather than walked to needs its driver baseline set,
    // or the first tick reads the whole jump as one frame of velocity.
    camera.reseat(driver);
    for _ in 0..20 {
        driver.x -= px(2);
        camera.tick(driver);
    }
    // Walked left off x=0 and came back around the far side.
    assert_eq!(camera.position().0, 2024, "wrapped, never clamped");
    assert_eq!(
        camera.sprite_pos(driver.x, driver.y).0,
        THRESHOLD_X,
        "and the driver is still held at the threshold across the seam"
    );
}

#[test]
fn the_on_screen_box_is_the_view_plus_thirty_two_pixels() {
    // Dead centre.
    assert!(on_screen(0x140, 0x120));
    // The margin's four edges, inclusive.
    assert!(on_screen(0x60, 0x100));
    assert!(on_screen(0x1E0, 0x100));
    assert!(on_screen(0x100, 0x60));
    assert!(on_screen(0x100, 0x180));
    // One step outside each.
    assert!(!on_screen(0x5F, 0x100));
    assert!(!on_screen(0x1E1, 0x100));
    assert!(!on_screen(0x100, 0x5F));
    assert!(!on_screen(0x100, 0x181));
}

#[test]
fn only_the_object_types_whose_routine_calls_the_test_are_frozen() {
    // The generic townsfolk, including both wanderers.
    assert!(type_tests_visibility(0x8038), "NPCType1");
    assert!(type_tests_visibility(0x803C), "NPCType2, a wanderer");
    assert!(
        type_tests_visibility(0x8040),
        "NPCType3, the other wanderer"
    );
    // The party and the named story NPCs never run the test, so they are
    // updated wherever they are.
    assert!(!type_tests_visibility(0x8004), "Chaz");
    assert!(!type_tests_visibility(0x8008), "Alys");
    assert!(!type_tests_visibility(0x8068), "NPCAlysPiata");
    assert!(!type_tests_visibility(0x8070), "NPCRune");
}

#[test]
fn a_sprite_coordinate_of_exactly_zero_short_circuits_to_on_screen() {
    // `move.w sprite_x_pos(a4), d0; beq.w .onscreen` runs before any range
    // check, so a zero on either axis wins outright — even paired with a
    // coordinate that is otherwise far off screen.
    assert!(on_screen(0, 0x9999));
    assert!(on_screen(0x100, 0));
    assert!(on_screen(0, 0));
    // And it really is zero that is special, not "small".
    assert!(!on_screen(1, 0x100));
    assert!(!on_screen(0x100, 1));
}

#[test]
fn bg_has_its_own_driver_latch_and_ec25_can_disable_fg() {
    let driver = Driver {
        x: px(800),
        y: px(300),
    };
    let bg = CameraBounds {
        width: 2048,
        height: 1024,
        edges: CameraEdges::Clamped,
    };
    let mut camera = Camera::placed_on_planes(
        driver,
        ACADEMY,
        bg,
        CameraGates {
            ec24: 0,
            ec25: 0,
            ec26: 1,
        },
    );
    let fg_before = camera.position_on(CameraPlane::Foreground);
    let bg_before = camera.position_on(CameraPlane::Background);
    let driver = Driver {
        x: driver.x + px(2),
        y: driver.y,
    };
    camera.tick(driver);
    assert_eq!(camera.position_on(CameraPlane::Foreground), fg_before);
    assert_eq!(
        camera.position_on(CameraPlane::Background),
        (bg_before.0 + 2, bg_before.1)
    );
    assert_eq!(camera.raw_step_on(CameraPlane::Foreground), (0, 0));
    assert_eq!(camera.raw_step_on(CameraPlane::Background), (px(2), 0));
    camera.set_gates(CameraGates {
        ec24: 1,
        ec25: 0,
        ec26: 1,
    });
    assert_eq!(camera.active_plane(), CameraPlane::Background);
    assert_eq!(
        camera.position(),
        camera.position_on(CameraPlane::Background)
    );
}

#[test]
fn refresh_map_rewrites_gate_bytes_and_only_zero_gates_consume_counters() {
    let bounds = CameraBounds::from_cells(40, 40, CameraEdges::Clamped);
    let mut camera = Camera::new_planes(bounds, bounds, CameraGates::field_default());

    // `loc_51AB2` writes EC24/25/26, then conditionally consumes the two
    // longwords behind each zero gate. This is the post-entry RefreshMap
    // contract, not a fresh camera placement.
    camera.apply_gate_write(
        CameraGates {
            ec24: 0,
            ec25: 0,
            ec26: 1,
        },
        (px(-2), px(3)),
        (px(5), px(-6)),
    );
    assert_eq!(
        camera.gates(),
        CameraGates {
            ec24: 0,
            ec25: 0,
            ec26: 1,
        }
    );
    assert_eq!(camera.raw_step_on(CameraPlane::Foreground), (px(-2), px(3)));
    assert_eq!(camera.raw_step_on(CameraPlane::Background), (0, 0));

    camera.apply_gate_write(
        CameraGates {
            ec24: 1,
            ec25: 1,
            ec26: 0,
        },
        (px(9), px(10)),
        (px(7), px(-8)),
    );
    assert_eq!(camera.raw_step_on(CameraPlane::Foreground), (px(-2), px(3)));
    assert_eq!(camera.raw_step_on(CameraPlane::Background), (px(7), px(-8)));
}

#[test]
fn the_existing_field_tape_replays_both_camera_planes() {
    let tape =
        crate::replay::Tape::parse(include_str!("../../../oracle/tapes/02_walk_timing.tape"))
            .unwrap();
    assert!(tape.frame_count() > 1_000, "the oracle tape is the fixture");
    let map = crate::FieldMap::new(
        crate::MapId(0),
        crate::CollisionGrid::filled(40, 40, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = crate::FieldState::new(
        &map,
        Cell::new(10, 10),
        crate::Direction::Down,
        crate::StepFrames::default(),
    )
    .unwrap();
    let mut camera = Camera::placed_on(
        Driver::at_cell(Cell::new(10, 10)),
        CameraBounds::from_cells(40, 40, CameraEdges::Clamped),
    );
    for frame in tape.frames().into_iter().take(256) {
        state.tick(&map, frame.buttons.to_input());
        let at = crate::trigger::PixelPos::from_cell(state.cell());
        let (ox, oy) = state.render_offset_16ths();
        camera.tick(Driver {
            x: (at.x + ox) * ONE_PIXEL,
            y: (at.y + oy) * ONE_PIXEL,
        });
        assert_eq!(
            camera.position_on(CameraPlane::Foreground),
            camera.position_on(CameraPlane::Background),
            "FG/BG remain aligned on the recorded field path"
        );
        assert_eq!(
            camera.raw_step_on(CameraPlane::Foreground),
            camera.raw_step_on(CameraPlane::Background)
        );
    }
    assert_eq!(camera.gates(), CameraGates::field_default());
}

#[test]
fn the_field_default_gates_match_the_oracle_field_entry() {
    assert_eq!(
        CameraGates::field_default(),
        CameraGates {
            ec24: 1,
            ec25: 1,
            ec26: 1,
        }
    );
}
