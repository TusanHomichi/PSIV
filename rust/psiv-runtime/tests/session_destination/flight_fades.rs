//! Tape 35's five unique Pal_FadeIn sites, six executions across two routes.
//! Expected words come from the verified helper loads, with nibble progression
//! worked independently of the CRAM implementation. These are state checks.

use super::*;

fn risen(target: u16, pass: u16) -> u16 {
    [0, 4, 8, 12].into_iter().fold(0, |word, shift| {
        word | (((target >> shift) & 15).min(pass) << shift)
    })
}

#[test]
fn all_flight_fades_step_the_helper_palettes_and_spend_their_terminal_frame() {
    for (origin, world_word, approach, times, maps, planet) in [
        (
            MOTA_GANGWAY,
            0,
            Button::Up,
            [218, 826, 1236],
            [0, 0x18C],
            WORLD_ZELAN,
        ),
        (
            (ZELAN, 31, 46),
            0x0300,
            Button::Down,
            [200, 556, 990],
            [0x18C, 0],
            WORLD_MOTAVIA,
        ),
    ] {
        let mut story = post_zio();
        story.world_word = world_word;
        let Some(mut player) = Player::at(origin, &story) else {
            return;
        };
        player.walk_into_menu(approach);
        let palettes = player.runtime().data().scene_palettes();
        let line = |address| &palettes.words(address).expect("rebuilt flight palettes")[..16];
        let ship = line(0x1D_E0F8).to_vec();
        let mut planet_target: [u16; 64] = palettes.words(0x09_F2BC).unwrap().try_into().unwrap();
        let planet_address = palettes
            .longword(0x06_4550, usize::from(planet))
            .expect("packed world-palette pointer");
        planet_target[..16].copy_from_slice(line(planet_address));
        planet_target[16..32].copy_from_slice(line(0x1D_E118));
        planet_target[48..64].copy_from_slice(line(0x1D_E138));
        planet_target[47] = 0;
        let targets: Vec<[u16; 64]> = [maps[0], maps[1]]
            .into_iter()
            .map(|map| {
                let mut words: [u16; 64] = player
                    .runtime()
                    .data()
                    .map(psiv_data::MapId(map))
                    .unwrap()
                    .map_update_palette
                    .as_deref()
                    .unwrap()
                    .try_into()
                    .unwrap();
                words[32..48].copy_from_slice(&ship);
                words
            })
            .collect();
        let expected = [targets[0], planet_target, targets[1]];
        let mut fades = Vec::new();
        for offset in 0..=times[2] + 16 {
            let frame = player.tick(if offset == 0 {
                Pad::new(Button::Speak)
            } else {
                Pad::NEUTRAL
            });
            for event in &frame.events {
                if matches!(
                    event,
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::FadeIn,
                        ..
                    }
                ) {
                    fades.push(offset);
                }
            }
            for (index, &start) in times.iter().enumerate() {
                if (start..start + 16).contains(&offset) {
                    let pass = (offset - start) as u16;
                    let cram = player.runtime().cram();
                    assert_eq!(
                        cram.words,
                        expected[index].map(|word| risen(word, pass)),
                        "{origin:?}, fade {index}, pass {pass}"
                    );
                    assert_eq!(cram.display, pass != 0, "display-enable wait, fade {index}");
                    assert!(cram.missing.is_empty(), "{:?}", cram.missing);
                    // Negative controls: an immediate restore or a two-frame
                    // cover cannot produce the first, odd-nibble palette pass.
                    if pass == 1 {
                        assert_ne!(cram.words, expected[index]);
                        assert_ne!(cram.words, [0; 64]);
                        assert_ne!(cram.words, expected[index].map(|word| risen(word, 2)));
                    }
                }
            }
        }
        assert_eq!(fades, times, "no omitted, duplicated or misplaced fade");
    }
}
