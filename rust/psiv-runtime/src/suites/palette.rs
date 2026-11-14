//! The palette scenes on the pack: what `Runtime::cram` holds, frame by
//! frame, while a transcribed scene runs its palette loops. Expected words
//! are worked by hand from the routine each op cites (a component's level
//! formula, not the code under test) from the map's own palette in the pack.

use std::path::Path;

use crate::{Runtime, RuntimeEvent};
use psiv_core::{
    CharId, Flag, GameState, Input, PresentationOp, RetailLocation, RetailSave, SceneOp, StepFrames,
};
use psiv_data::GameData;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack_present() -> bool {
    let present = Path::new(PACK).join("manifest.json").is_file();
    if !present {
        eprintln!("runtime pack not present; skipping");
    }
    present
}

fn runtime_on(map: u16, party: [Option<CharId>; 5], flags: &[Flag]) -> Runtime {
    runtime_at(map, (16, 16), party, flags)
}

fn runtime_at(map: u16, pixels: (u16, u16), party: [Option<CharId>; 5], flags: &[Flag]) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let initial = crate::Session::start(data.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    // A member a scene writes to (Demi's Phonon skill) needs a record.
    for &who in party.iter().flatten() {
        if game.roster().get(who).is_none() {
            game.roster_mut()
                .seat(who, super::save::stats(who.0))
                .expect("seat");
        }
    }
    game.set_party(party);
    for &flag in flags {
        let _ = game.set(flag);
    }
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: pixels.0,
                char_y: pixels.1,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime starts")
}

/// One tick of a scene run: the frame count, the events, the palette after.
struct Frame {
    count: u16,
    events: Vec<RuntimeEvent>,
    words: [u16; 64],
    beams: Vec<(bool, u64)>,
}

/// Runs `event` to its end, closing every window at once, and records every
/// tick. The palette before the first tick is the trace's base.
fn trace(runtime: &mut Runtime, event: u16) -> ([u16; 64], Vec<Frame>) {
    let base = runtime.cram().words;
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut frames = Vec::new();
    for _ in 0..50_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "{item:?}"
            );
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        frames.push(Frame {
            count: runtime.frames,
            events,
            words: runtime.cram().words,
            beams: runtime
                .bespoke_actors()
                .iter()
                .filter_map(|actor| actor.beam_draw())
                .collect(),
        });
        if !runtime.scene_active() {
            assert!(
                runtime.cram().missing.is_empty(),
                "{:x?}",
                runtime.cram().missing
            );
            return (base, frames);
        }
    }
    panic!("event {event:#x} did not finish");
}

/// The tick a presentation record matching `pick` was emitted on.
fn emitted(frames: &[Frame], pick: impl Fn(&PresentationOp) -> bool) -> usize {
    frames
        .iter()
        .position(|frame| {
            frame.events.iter().any(|event| {
                matches!(event, RuntimeEvent::ScenePresentation { op: SceneOp::Presentation { op }, .. } if pick(op))
            })
        })
        .expect("the record is emitted")
}

/// The first tick after `from` whose events satisfy `pick`.
fn next_event(frames: &[Frame], from: usize, pick: impl Fn(&RuntimeEvent) -> bool) -> usize {
    from + 1
        + frames[from + 1..]
            .iter()
            .position(|frame| frame.events.iter().any(&pick))
            .expect("the event follows")
}

/// A component `field` (`$E`, `$E0` or `$E00`) of `word`, as a level 0..7.
fn level(word: u16, shift: u16) -> u16 {
    (word >> shift) & 7
}

fn rgb(word: u16) -> [u16; 3] {
    [level(word, 1), level(word, 5), level(word, 9)]
}

/// Acceptance for `$99`: forty frames of `Pal_IncreaseTone` with line 2
/// held at `Pal_Init_Line_3`, then forty of `loc_58666`. By hand, each
/// component of lines 0, 1 and 3 is `min(level + s, 7)` after `s` phase
/// frames of the tone, then `max(7 - s', start)` after `s'` of the restore
/// (seven tone steps whiten every word, ten happen); line 2 is the pack's
/// `$296300` words on every frame of both loops.
#[test]
fn enter_anger_tower_whitens_three_lines_and_holds_the_third() {
    if !pack_present() {
        return;
    }
    // Anger Tower ($FC), Elsydeon ($D9) held: the barrier breaks.
    let mut runtime = runtime_on(
        0x0FC,
        [Some(CharId(0)), None, None, None, None],
        &[Flag::event(0xD9)],
    );
    let held: Vec<u16> = runtime
        .data()
        .scene_palettes()
        .words(0x0029_6300)
        .expect("the pack carries Pal_Init_Line_3")[..16]
        .to_vec();
    let (base, frames) = trace(&mut runtime, 0x99);
    let tone = emitted(&frames, |op| {
        matches!(op, PresentationOp::PaletteIncreaseTone { .. })
    });
    let start = if tone == 0 {
        base
    } else {
        frames[tone - 1].words
    };
    assert_eq!(
        start[32..48],
        held[..],
        "the map load put the party line in line 2"
    );
    let failures = check_anger_tower(&start, &held, &frames[tone..tone + 80]);
    assert_eq!(failures, Vec::<String>::new());
    assert!(runtime.game().is_set(Flag::event(0xE0)));
}

/// The `$99` assertions as a list of failures, so a negative control can
/// show which one a broken model trips.
fn check_anger_tower(start: &[u16; 64], held: &[u16], frames: &[Frame]) -> Vec<String> {
    let mut failures = Vec::new();
    let mut tone_steps = 0;
    let mut restore_steps = 0;
    for (k, frame) in frames.iter().enumerate() {
        if frame.count & 3 == 0 {
            if k < 40 {
                tone_steps += 1;
            } else {
                restore_steps += 1;
            }
        }
        if frame.words[32..48] != *held {
            failures.push(format!(
                "frame {k}: line 2 is {:04X?}",
                &frame.words[32..48]
            ));
        }
        for slot in (0..32).chain(48..64) {
            let want: Vec<u16> = rgb(start[slot])
                .iter()
                .map(|&from| {
                    if k < 40 {
                        (from + tone_steps).min(7)
                    } else {
                        7u16.saturating_sub(restore_steps).max(from)
                    }
                })
                .collect();
            if rgb(frame.words[slot]).to_vec() != want {
                failures.push(format!(
                    "frame {k}: word {slot} {:04X} is not levels {want:?}",
                    frame.words[slot]
                ));
            }
        }
    }
    if tone_steps != 10 || restore_steps != 10 {
        failures.push(format!(
            "{tone_steps} tone and {restore_steps} restore phase frames"
        ));
    }
    failures
}

/// Negative control for the `$99` check: a model with the hold dropped
/// steps line 2 too, and the check names line 2.
#[test]
fn the_anger_tower_check_catches_a_dropped_hold() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_on(
        0x0FC,
        [Some(CharId(0)), None, None, None, None],
        &[Flag::event(0xD9)],
    );
    let held: Vec<u16> = runtime
        .data()
        .scene_palettes()
        .words(0x0029_6300)
        .unwrap()
        .to_vec();
    let start = runtime.cram().words;
    // The tone without its hold, through the same CRAM model.
    let mut frames = Vec::new();
    let op = SceneOp::Presentation {
        op: PresentationOp::PaletteIncreaseTone {
            frames: 40,
            hold: None,
        },
    };
    let palettes = runtime.data().scene_palettes().clone();
    let first = runtime.frames;
    let cram = &mut runtime.effects.cram;
    let _ = cram.apply(&op, first, &palettes, None);
    frames.push(Frame {
        count: first,
        events: Vec::new(),
        words: cram.view().words,
        beams: Vec::new(),
    });
    for k in 1..40u16 {
        cram.tick(first.wrapping_add(k));
        frames.push(Frame {
            count: first.wrapping_add(k),
            events: Vec::new(),
            words: cram.view().words,
            beams: Vec::new(),
        });
    }
    let failures = check_anger_tower(&start, &held, &frames);
    assert!(
        failures.iter().any(|failure| failure.contains("line 2")),
        "{failures:?}"
    );
}

/// Acceptance for `Event_Recovery`: the flashes step only words 0..31, and
/// each pair returns them to the words they started from; 32..63 never
/// change.
#[test]
fn the_recovery_flash_restores_two_lines_and_leaves_two() {
    if !pack_present() {
        return;
    }
    // AirCastle_F1_Part9 ($176), whose recovery tile fires $21.
    let mut runtime = runtime_on(0x176, [Some(CharId(0)), None, None, None, None], &[]);
    let (base, frames) = trace(&mut runtime, 0x21);
    let flash = emitted(&frames, |op| {
        matches!(op, PresentationOp::PaletteToneFlash { .. })
    });
    // Something changed: the first flash's frames brighten line 0 or 1.
    assert!(
        frames[flash..flash + 16]
            .iter()
            .any(|f| f.words[..32] != base[..32])
    );
    for (k, frame) in frames.iter().enumerate() {
        assert_eq!(frame.words[32..], base[32..], "frame {k}: lines 2 and 3");
    }
    assert_eq!(runtime.cram().words, base, "every word back");
    assert_eq!(runtime.effects.cram.saved()[..32], base[..32]);
}

/// Acceptance for Rykros: `loc_78296` spends each `loc_782D2` delay
/// (US `$0782D2`) plus one frame for the `dbf` loop: 46
/// frames from the cycle's first frame to the dialogue that follows it. The
/// last frame's nine words are the table's sixth row, at `$FFFFFB2E`.
#[test]
fn the_rykros_cycle_blocks_its_frames() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_on(0x002, [Some(CharId(0)), None, None, None, None], &[]);
    let table: Vec<u16> = runtime
        .data()
        .scene_palettes()
        .words(0x0007_82DA)
        .expect("Rykros table")
        .to_vec();
    // Check the scene's actual emitted ids, including panels outside the
    // dialogue-action census. An empty pack is the negative control.
    let missing_panels = |palettes: &psiv_data::ScenePalettes| {
        psiv_core::scene_for(psiv_core::EventIndex(0x801B))
            .unwrap()
            .ops
            .iter()
            .filter_map(|op| match op {
                SceneOp::PanelCreate { id } if palettes.panel(*id).is_none() => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        missing_panels(&psiv_data::ScenePalettes::default()),
        [0x18B, 0x18F, 0x190]
    );
    assert!(missing_panels(runtime.data().scene_palettes()).is_empty());
    let (_, frames) = trace(&mut runtime, 0x801B);
    let cycle = emitted(&frames, |op| {
        matches!(op, PresentationOp::RykrosPaletteCycle { .. })
    });
    let resume = next_event(&frames, cycle, |event| {
        matches!(
            event,
            RuntimeEvent::SceneDialogueResume | RuntimeEvent::SceneDialogue { .. }
        )
    });
    assert_eq!(resume - cycle, 46);
    assert_eq!(frames[cycle].words[23..32], table[..9]);
    assert_eq!(frames[resume - 1].words[23..32], table[45..54]);
}

/// `Pal_VariableFadeOut` with `$ED52` = 5 in `Event_Phonon` blocks for its
/// passes: the largest nibble of the palette it fades, six frames each.
#[test]
fn a_variable_fade_blocks_for_the_palette_it_fades() {
    if !pack_present() {
        return;
    }
    let party = [
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(6)),
        None,
        None,
    ];
    let mut runtime = runtime_on(0x0BD, party, &[]);
    let (_, frames) = trace(&mut runtime, 0x6C);
    let fade = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade { fade_in: false, .. }
        )
    });
    let before = frames[fade - 1].words;
    let largest = before
        .iter()
        .flat_map(|word| [word & 0xF, (word >> 4) & 0xF, (word >> 8) & 0xF, word >> 12])
        .max()
        .unwrap()
        .max(1);
    // `$ED52` is 5 here: a pass, then six `VInt_Prepare`s (`loc_5B1A4`).
    let every = 6;
    let spent = usize::from(largest) * every;
    // Each pass writes the buffer before its wait (`ps4.asm:122042-122066`),
    // so the last pass turns it black at the start of its six frames, and
    // the pass before leaves a nibble lit.
    assert_eq!(frames[fade + spent - every].words, [0; 64]);
    assert_ne!(frames[fade + spent - every - 1].words, [0; 64]);
    // The scene's next ops are `wait(40)` (no event) and a sound: the sound
    // lands the fade's frames and forty more after it.
    let next = next_event(&frames, fade, |event| {
        matches!(event, RuntimeEvent::ScenePresentation { .. })
    });
    assert_eq!(next - fade, spent + 40);
}

#[test]
fn zio_ramp_reaches_the_live_scene_palette() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_on(
        0x82,
        [
            Some(CharId(0)),
            Some(CharId(5)),
            Some(CharId(3)),
            Some(CharId(4)),
            Some(CharId(6)),
        ],
        &[Flag::event(0x63)],
    );
    let (_, frames) = trace(&mut runtime, 0x30);
    let ramp = emitted(&frames, |op| {
        matches!(op, PresentationOp::PaletteRampFromTable { .. })
    });
    assert_eq!(frames[ramp].words[49..52], [0xCCC, 0xCA, 0x88]);
    assert_eq!(frames[ramp + 30].words[49..52], [0xAAA, 0xA8, 0x66]);
    let main = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::BarrierBeamUpdates {
                frames: 40,
                reveal: false
            }
        )
    });
    let reveal = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::BarrierBeamUpdates {
                frames: 1,
                reveal: true
            }
        )
    });
    assert_eq!(
        frames[ramp].beams.len(),
        4,
        "all four routines must be represented"
    );
    assert_eq!(reveal - main, 40);
    assert_eq!(ramp - reveal, 1);
    for (before, after) in frames[main].beams.iter().zip(&frames[reveal].beams) {
        assert_eq!(
            after.1 - before.1,
            40,
            "40 further calls after the first update"
        );
    }
    assert_eq!(frames[ramp].beams, frames[reveal].beams);
    for frame in &frames[ramp..ramp + 210] {
        assert_eq!(
            frame.beams, frames[ramp].beams,
            "map-only ramp must freeze the SAT pose"
        );
    }
}

/// All three real boarding events: the art load precedes the counted palette
/// copy, which precedes exposing the vehicle body, even on the camera-pan path.
#[test]
fn vehicle_boarding_loads_the_pack_palette_before_exposing_the_body() {
    use psiv_core::{PaletteBuffer, PaletteSource};
    if !pack_present() {
        return;
    }
    // Interior placements from session_menu_scenes' boarding fixtures. Each
    // requires the real camera pan; the map corner's target cannot be reached.
    for (index, address, cell) in [
        (1, 0x0029_754E, (115, 177)),
        (2, 0x0029_756E, (114, 178)),
        (3, 0x0029_758E, (20, 20)),
    ] {
        let mut runtime = runtime_at(
            0,
            (cell.0 * 16, cell.1 * 16),
            [Some(CharId(0)), None, None, None, None],
            &[Flag::event(0x0C), Flag::event(0x43)],
        );
        let source = runtime
            .data()
            .scene_palettes()
            .words(address)
            .expect("boarding palette")
            .to_vec();
        assert_eq!(source.len(), 17);
        let saved = *runtime.effects.cram.saved();
        let (base, frames) = trace(&mut runtime, 8 + index);
        let at = emitted(&frames, |op| {
            matches!(op, PresentationOp::PaletteCopy {
            source: PaletteSource::Rom(a), destination: PaletteBuffer::Live,
            start: 48, words: 17,
        } if *a == address)
        });
        let art_at = emitted(&frames, |op| {
            matches!(
                op,
                PresentationOp::LoadSceneAsset {
                    destination_ram: 0xFFFF_0000,
                    ..
                }
            )
        });
        assert!(art_at <= at);
        let ops: Vec<_> = frames[at]
            .events
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::ScenePresentation { op, .. } => Some(op),
                _ => None,
            })
            .collect();
        let art = ops
            .iter()
            .position(|op| {
                matches!(
                    op,
                    SceneOp::Presentation {
                        op: PresentationOp::LoadSceneAsset { .. }
                    }
                )
            })
            .unwrap();
        let copy = ops
            .iter()
            .position(|op| {
                matches!(
                    op,
                    SceneOp::Presentation {
                        op: PresentationOp::PaletteCopy { .. }
                    }
                )
            })
            .unwrap();
        assert!(art < copy, "art, then palette in the emitted tick");
        let scene = psiv_core::scene_for(psiv_core::EventIndex(8 + index)).unwrap();
        let copy = scene
            .ops
            .iter()
            .position(|op| {
                matches!(
                    op,
                    SceneOp::Presentation {
                        op: PresentationOp::PaletteCopy { .. }
                    }
                )
            })
            .unwrap();
        let body = scene
            .ops
            .iter()
            .position(|op| matches!(op, SceneOp::AlignVehicleBoarding { .. }))
            .unwrap();
        assert!(copy < body, "palette before exposing the vehicle body");
        let mut expected = base;
        expected[48..].copy_from_slice(&source[..16]);
        assert_eq!(
            frames[at].words, expected,
            "index {index}: all 64 words at the load"
        );
        // The normal Motavia water cycle continues during the camera pan;
        // boarding preserves its program and keeps the vehicle line loaded.
        for frame in frames.iter().skip(at) {
            assert_eq!(
                frame.words[48..],
                expected[48..],
                "index {index}, frame {}: vehicle line through the pan",
                frame.count,
            );
        }
        let mut expected_saved = saved;
        expected_saved[0] = source[16];
        assert_eq!(runtime.effects.cram.saved(), &expected_saved);
        assert_eq!(runtime.game().vehicle_index(), index);
    }
}

/// `Cutscene_Alshline` (`$8005`) copies `PiataBasementB2_Palettes`'
/// fourteen words to line 3 before it builds Igglanova (`$188`), whose sheet
/// draws from line 3. On the tick the object is built, line 3's first
/// fourteen words are the pack's, and every other word is what it was on the
/// tick before the copy.
#[test]
fn alshline_loads_igglanova_s_line_before_building_it() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_at(
        0x24,
        (31 * 16, 49 * 16),
        [Some(CharId(0)), None, None, None, None],
        &[Flag::event(0x32)],
    );
    let source = runtime
        .data()
        .scene_palettes()
        .words(0x0005_2848)
        .expect("PiataBasementB2_Palettes is packed")
        .to_vec();
    assert_eq!(source.len(), 14);
    assert!(runtime.start_event(0x8005), "Alshline starts");
    let mut before = runtime.cram().words;
    for _ in 0..10_000 {
        let events = runtime.tick(Input::Neutral);
        if events.iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            )
        }) {
            runtime.dialogue_closed();
        }
        let ops: Vec<&SceneOp> = events
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::ScenePresentation { op, .. } => Some(op),
                _ => None,
            })
            .collect();
        let load = ops.iter().position(|op| {
            matches!(
                op,
                SceneOp::LoadPalette {
                    rom_addr: 0x0005_2848,
                    ..
                }
            )
        });
        let build = ops.iter().position(|op| {
            matches!(
                op,
                SceneOp::CreateFieldObject {
                    object_id: 0x188,
                    ..
                }
            )
        });
        if let Some(build) = build {
            let load = load.expect("the copy is on the object's tick");
            assert!(load < build, "palette before the object");
            let words = runtime.cram().words;
            assert_eq!(words[48..62], source[..], "line 3's fourteen words");
            assert_eq!(words[..48], before[..48], "lines 0-2 keep their words");
            assert_eq!(words[62..], before[62..], "words 62-63 stay Zema's");
            assert_ne!(
                before[48..62],
                source[..],
                "the map palette differs, so the copy is observable"
            );
            assert!(runtime.cram().missing.is_empty());
            return;
        }
        before = runtime.cram().words;
    }
    panic!("Igglanova was never built");
}
