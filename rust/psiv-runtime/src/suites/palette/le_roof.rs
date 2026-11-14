//! The Le Roof palette scenes on the pack: `Event_MeetingLeRoof`'s colour
//! cycle and ramp fade-in, its return of the room palette, and the repeat
//! visit's cycle and 60-frame wait.

use std::path::Path;

use super::{PACK, pack_present, runtime_at};
use crate::{Runtime, RuntimeEvent};
use psiv_core::{CharId, Flag, Input, PresentationOp, SceneOp};
use psiv_data::GameData;

/// One tick of a Le Roof run: the frame count, the events, the palette and
/// the map-update calls so far.
struct LeRoofTick {
    count: u16,
    events: Vec<RuntimeEvent>,
    words: [u16; 64],
    calls: u64,
}

/// Runs `event` on Le Roof Room (`$F0`) from beside Le Roof, closing every
/// window at once, until `stop` holds for a tick's events or the scene ends.
fn le_roof_run(
    event: u16,
    flags: &[Flag],
    stop: impl Fn(&RuntimeEvent) -> bool,
) -> Vec<LeRoofTick> {
    le_roof_run_on(&mut le_roof_room(flags), event, stop)
}

/// Le Roof Room (`$F0`) with Chaz beside Le Roof, its map just loaded.
fn le_roof_room(flags: &[Flag]) -> Runtime {
    let party = [Some(CharId(0)), None, None, None, None];
    runtime_at(0x0F0, (0x1F0, 0x1F0), party, flags)
}

/// [`le_roof_run`] on `runtime`, which is left where the run stopped.
fn le_roof_run_on(
    runtime: &mut Runtime,
    event: u16,
    stop: impl Fn(&RuntimeEvent) -> bool,
) -> Vec<LeRoofTick> {
    assert_eq!(
        runtime.map_updates().counters[1],
        0,
        "the map load clears $ECF3"
    );
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut ticks = Vec::new();
    for _ in 0..5_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "{item:?}"
            );
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        let done = events.iter().any(&stop) || !runtime.scene_active();
        ticks.push(LeRoofTick {
            count: runtime.frames,
            events,
            words: runtime.cram().words,
            calls: runtime.map_updates().calls,
        });
        if done {
            assert!(
                runtime.cram().missing.is_empty(),
                "{:x?}",
                runtime.cram().missing
            );
            return ticks;
        }
    }
    panic!("event {event:#x} did not reach its stop");
}

/// The tick a presentation record matching `pick` was emitted on, from
/// `from` on.
fn le_roof_emitted(
    ticks: &[LeRoofTick],
    from: usize,
    pick: impl Fn(&PresentationOp) -> bool,
) -> usize {
    from + ticks[from..]
        .iter()
        .position(|tick| {
            tick.events.iter().any(|event| {
                matches!(event, RuntimeEvent::ScenePresentation { op: SceneOp::Presentation { op }, .. } if pick(op))
            })
        })
        .expect("the record is emitted")
}

/// Checks the Le Roof loops from tick `start` for `frames` ticks against
/// `loc_70638` and `loc_705D6`, worked from their listings: on a tick whose
/// count is a multiple of 4 the index (`$FFFFECF3`, from `index`) restarts
/// at 0 once it reaches 11, words 11 and 12 take series entries `index` and
/// `index + 11`, and the index steps; other ticks leave both words. During
/// the fade-in (`grow` ticks from `grow_at`) pass `k` lights the last
/// `1 + k'` ramp words over words 2.. (`k'` the passes before `k` whose
/// count `70 - pass` is a multiple of 8), the rest of the nine black. No tick
/// updates the map. Returns the index left.
fn check_le_roof_loops(
    ticks: &[LeRoofTick],
    start: usize,
    frames: usize,
    mut index: usize,
    ramp: &[u16],
    cycle: &[u16],
    grow_at: Option<usize>,
) -> usize {
    for at in start..start + frames {
        let tick = &ticks[at];
        let before = &ticks[at - 1];
        assert_eq!(tick.calls, before.calls, "tick {at}: no map update");
        if tick.count.is_multiple_of(4) {
            if index >= 11 {
                index = 0;
            }
            assert_eq!(
                tick.words[11..13],
                [cycle[index], cycle[index + 11]],
                "tick {at}"
            );
            index += 1;
        } else {
            assert_eq!(tick.words[11..13], before.words[11..13], "tick {at}");
        }
        if let Some(grow_at) = grow_at
            && (grow_at..grow_at + 71).contains(&at)
        {
            let pass = at - grow_at;
            let lit = 1
                + (0..pass)
                    .filter(|done| (70 - done).is_multiple_of(8))
                    .count();
            let mut expected = [0u16; 9];
            expected[..lit].copy_from_slice(&ramp[9 - lit..]);
            assert_eq!(tick.words[2..11], expected, "tick {at}, pass {pass}");
        }
        assert_eq!(
            tick.words[13..],
            before.words[13..],
            "tick {at}: words 13 on"
        );
        assert_eq!(tick.words[..2], before.words[..2], "tick {at}: words 0-1");
    }
    index
}

/// Acceptance for `Event_MeetingLeRoof` (#591): from the red fade's end,
/// 180 passes of the colour cycle, the 71-pass fade-in (`$0705D6`), 120
/// passes, dialogue `0`, 120 passes, every one `VInt_Prepare` only. The
/// cycle index runs on across the loops and the dialogue. Negative control:
/// the pack without the tables lists both as missing.
#[test]
fn meeting_le_roof_cycles_and_fades_in_its_palette() {
    if !pack_present() {
        return;
    }
    let ticks = le_roof_run(0x0048, &[], |_| false);
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let ramp = data
        .scene_palettes()
        .words(0x0007_0626)
        .expect("ramp")
        .to_vec();
    let cycle = data
        .scene_palettes()
        .words(0x0007_0672)
        .expect("cycle")
        .to_vec();
    assert_eq!((ramp.len(), cycle.len()), (9, 22));
    let first = le_roof_emitted(&ticks, 0, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 180, .. })
    });
    let grow = le_roof_emitted(&ticks, first, |op| {
        matches!(op, PresentationOp::PaletteGrowIn { .. })
    });
    assert_eq!(grow - first, 180);
    let second = le_roof_emitted(&ticks, grow, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 120, .. })
    });
    assert_eq!(second - grow, 71);
    let index = check_le_roof_loops(&ticks, first, 180 + 71 + 120, 0, &ramp, &cycle, Some(grow));
    let dialogue = second + 120;
    assert!(
        ticks[dialogue]
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneDialogue { .. })),
        "dialogue 0 right after the first 120"
    );
    let third = le_roof_emitted(&ticks, dialogue, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 120, .. })
    });
    check_le_roof_loops(&ticks, third, 120, index, &ramp, &cycle, None);
    // The ramp stays lit through the dialogue and the last loop.
    assert_eq!(ticks[third + 119].words[2..11], ramp[..]);
    // The music stop follows the last pass.
    assert!(ticks[third + 120].events.iter().any(|event| matches!(
        event,
        RuntimeEvent::ScenePresentation {
            op: SceneOp::PlaySound { id: 0xFB },
            ..
        }
    )));

    let mut cram = crate::cram::Cram::default();
    let _ = cram.apply(
        &SceneOp::Presentation {
            op: PresentationOp::PaletteGrowIn {
                source_rom_addr: 0x0007_0626,
                offset: 2,
                words: 9,
                passes: 71,
                cycle: psiv_core::CounterCycle {
                    source_rom_addr: 0x0007_0672,
                    offset: 11,
                    entries: 11,
                },
            },
        },
        0,
        &psiv_data::ScenePalettes::default(),
        None,
        &mut 0,
    );
    assert_eq!(cram.view().missing, [0x0007_0626]);
}

/// Acceptance for `Event_MeetingLeRoof`'s return (`$07058A..$0705C3`): the
/// fade-out, then `Pal_SilenceTm`'s words 0-31 into `Palette_Table_Buffer_2`
/// and the fade up to them, give back the room's own palette: after
/// `SceneEnded` and on every field tick after it, all 64 words are the ones
/// the map load left, except the words the room's own map program animates
/// (learnt from the same room left alone), which take only values that
/// program writes, through the `Event_Recovery` the cell beside Le Roof
/// starts. Negative control: the last tick of the last colour loop still
/// shows the cinematic words (the ramp, the cycle words and the black second
/// line), so the check would see them left up.
#[test]
fn meeting_le_roof_gives_back_the_room_palette() {
    if !pack_present() {
        return;
    }
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let silence = data
        .scene_palettes()
        .words(0x0018_711A)
        .expect("Pal_SilenceTm")
        .to_vec();
    assert_eq!(silence.len(), 32);

    // The room left alone (`$D1` set, so its trigger stays off, and Chaz off
    // the recovery tile `RunEvent_Recovery` watches): the words its map
    // program writes, and the values.
    let party = [Some(CharId(0)), None, None, None, None];
    let mut alone = runtime_at(0x0F0, (16, 16), party, &[Flag::event(0xD1)]);
    let field = alone.cram().words;
    let mut animated: Vec<std::collections::BTreeSet<u16>> = vec![Default::default(); 64];
    for _ in 0..600 {
        let events = alone.tick(Input::Neutral);
        assert!(
            !alone.scene_active(),
            "no event starts once `$D1` is set: {events:?}"
        );
        for (slot, &word) in alone.cram().words.iter().enumerate() {
            if word != field[slot] {
                animated[slot].insert(word);
            }
        }
    }
    let still = |slot: usize| animated[slot].is_empty();
    // The room's lines 1-2 are `Pal_SilenceTm`'s, the fade's target.
    for slot in (0..32).filter(|&slot| still(slot)) {
        assert_eq!(field[slot], silence[slot], "word {slot} of the map load");
    }

    let mut runtime = le_roof_room(&[]);
    assert_eq!(runtime.cram().words, field, "the same map load");
    let ticks = le_roof_run_on(&mut runtime, 0x0048, |event| {
        matches!(event, RuntimeEvent::SceneEnded)
    });
    let ended = ticks.last().expect("ticks");
    assert!(
        ended
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneEnded)),
        "the scene ends"
    );
    assert!(runtime.game().is_set(Flag::event(0xD1)));

    let stop = ticks
        .iter()
        .position(|tick| {
            tick.events.iter().any(|event| {
                matches!(
                    event,
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::PlaySound { id: 0xFB },
                        ..
                    }
                )
            })
        })
        .expect("the music stop after the last colour loop");
    let cinematic = &ticks[stop - 1].words;
    let differs = |range: std::ops::Range<usize>| {
        range
            .filter(|&slot| still(slot) && cinematic[slot] != field[slot])
            .count()
    };
    assert!(differs(2..11) > 0, "the ramp is up before the fade-out");
    assert!(differs(16..32) > 0, "the second line is black before it");

    let check = |words: &[u16; 64], when: &str| {
        for (slot, &word) in words.iter().enumerate() {
            if still(slot) {
                assert_eq!(word, field[slot], "{when}: word {slot}");
            } else {
                assert!(
                    word == field[slot] || animated[slot].contains(&word),
                    "{when}: word {slot} is {word:#06x}, not one its map program writes"
                );
            }
        }
    };
    check(&ended.words, "SceneEnded");
    // Chaz stands on the recovery cell beside Le Roof, so `RunEvent_Recovery`
    // (`$12`) starts `Event_Recovery` on the first field tick. Its flashes
    // step the live words and restore the ones it copied first, so the
    // room's palette holds on every tick no scene runs, through it and after.
    let mut field_ticks = 0;
    let mut recovered = false;
    for k in 0..600 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            recovered |= matches!(item, RuntimeEvent::SceneStarted { trigger: 0x12 });
            assert!(
                !matches!(item, RuntimeEvent::SceneDialogue { .. }),
                "tick {k}: {item:?}"
            );
        }
        if runtime.scene_active() {
            continue;
        }
        check(&runtime.cram().words, &format!("tick {k} after the scene"));
        field_ticks += 1;
        if field_ticks == 120 {
            break;
        }
    }
    assert_eq!(field_ticks, 120, "120 field ticks after the scene");
    assert!(recovered, "the recovery cell's event ran in between");
}

/// Acceptance for `Cutscene_LeRoofAgain` (#591): the same loops (180,
/// `jsr loc_705D6` at `$078406`, 120) before dialogue `$0B`, and after the
/// panel's fade-out the 60-frame `DoMapUpdateLoop` at `$07846C`, which runs
/// a map update on each of its frames.
#[test]
fn le_roof_again_cycles_then_waits_with_map_updates() {
    if !pack_present() {
        return;
    }
    let flags = [Flag::event(0xD5), Flag::event(0xD3)];
    let ticks = le_roof_run(0x801C, &flags, |event| {
        matches!(
            event,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PanelCreate { id: 0x125 },
                ..
            }
        )
    });
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let ramp = data.scene_palettes().words(0x0007_0626).unwrap().to_vec();
    let cycle = data.scene_palettes().words(0x0007_0672).unwrap().to_vec();
    let first = le_roof_emitted(&ticks, 0, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 180, .. })
    });
    let grow = le_roof_emitted(&ticks, first, |op| {
        matches!(op, PresentationOp::PaletteGrowIn { .. })
    });
    let second = le_roof_emitted(&ticks, grow, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 120, .. })
    });
    assert_eq!((grow - first, second - grow), (180, 71));
    check_le_roof_loops(&ticks, first, 180 + 71 + 120, 0, &ramp, &cycle, Some(grow));
    assert!(
        ticks[second + 120]
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneDialogue { .. })),
        "dialogue $0B right after the 120"
    );
    let is = |tick: &LeRoofTick, want: &dyn Fn(&SceneOp) -> bool| {
        tick.events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::ScenePresentation { op, .. } if want(op)))
    };
    let fade_out = (second..ticks.len())
        .find(|&at| is(&ticks[at], &|op| matches!(op, SceneOp::FadeOut)))
        .expect("the fade-out");
    let reinit = (fade_out..ticks.len())
        .find(|&at| is(&ticks[at], &|op| matches!(op, SceneOp::InitVramAndCram)))
        .expect("the second InitVRAMAndCRAM");
    assert_eq!(reinit - fade_out, 60, "the 60-frame wait between");
    assert_eq!(
        ticks[reinit - 1].calls - ticks[fade_out - 1].calls,
        60,
        "a map update on each of its frames"
    );
}
