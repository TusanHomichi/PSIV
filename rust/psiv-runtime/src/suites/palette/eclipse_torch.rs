//! `Event_EclipseTorchUsed` (`$0047`) on the pack, on Dezolis (`$001`) below
//! the Air Castle's trigger (#593): `Runtime::cram` and the map-update calls
//! through both white flashes and the main-update loop. Dezolis's
//! `MapUpdate_DezoSnowstormPal` makes every `RunMapUpdates` call a count in
//! [`Runtime::map_updates`], and rewrites words 19-22 on a frame whose
//! `Main_Frame_Count` is a multiple of 8, so the checks leave those four out.
//!
//! Each flash, by hand from the US bytes: the 64 words saved
//! (`move.w #$3F, d7` / `trap #1`), `$0EEE` over all 64 (`loc_7025A`,
//! `loc_7029C`), two frames of map updates (`DoMapUpdateLoop(1)` at
//! `$070266`, `$0702A8`), then forty of `loc_58666` and `VInt_Prepare` with
//! no map update (`$070270`, `$0702B2`). `loc_58666` steps a component one
//! level toward the saved word on a frame whose `Main_Frame_Count` is a
//! multiple of 4, so from white a component is `max(7 - s, saved)` after `s`
//! such frames; forty frames hold ten, enough for seven levels.

use super::{pack_present, runtime_at};
use crate::{Runtime, RuntimeEvent};
use psiv_core::{CharId, Flag, Input, PaletteSource, PresentationOp, SceneOp, palette};

/// The words `MapUpdate_DezoSnowstormPal` writes (`$26..$2C(a1)`).
const SNOW: std::ops::Range<usize> = 19..23;

struct Tick {
    count: u16,
    events: Vec<RuntimeEvent>,
    words: [u16; 64],
    calls: u64,
}

impl Tick {
    fn emits(&self, pick: impl Fn(&SceneOp) -> bool) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::ScenePresentation { op, .. } if pick(op)))
    }
}

/// Runs the torch on foot from below the trigger, closing every window at
/// once, to its end.
fn torch_run() -> (Vec<Tick>, Runtime) {
    let party = [Some(CharId(0)), Some(CharId(1)), None, None, None];
    let mut runtime = runtime_at(0x001, (0xBA0, 0xD0), party, &[Flag::event(0x9B)]);
    assert!(runtime.start_event(0x0047), "the torch starts");
    let mut ticks = Vec::new();
    for _ in 0..2_000 {
        let events = runtime.tick(Input::Neutral);
        for event in &events {
            assert!(
                !matches!(event, RuntimeEvent::SceneFaulted { .. }),
                "{event:?}"
            );
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        ticks.push(Tick {
            count: runtime.frames,
            events,
            words: runtime.cram().words,
            calls: runtime.map_updates().calls,
        });
        if !runtime.scene_active() {
            assert!(
                runtime.cram().missing.is_empty(),
                "{:x?}",
                runtime.cram().missing
            );
            return (ticks, runtime);
        }
    }
    panic!("the torch did not finish");
}

fn level(word: u16, shift: u16) -> u16 {
    (word >> shift) & 7
}

fn is_white(op: &SceneOp) -> bool {
    matches!(
        op,
        SceneOp::Presentation {
            op: PresentationOp::PaletteCopy {
                source: PaletteSource::Fill(0x0EEE),
                start: 0,
                words: 64,
                ..
            }
        }
    )
}

/// The checks of one flash whose white fill is on tick `fill`, as failures.
/// `saved` is the palette the tick before it left, which the fill's copy
/// keeps and the restore returns to.
fn flash_failures(ticks: &[Tick], fill: usize) -> Vec<String> {
    let mut failures = Vec::new();
    let saved = ticks[fill - 1].words;
    // Two frames of map updates, all white but the snow words, which a
    // map update may have rewritten.
    for k in fill..fill + 2 {
        if ticks[k].calls != ticks[k - 1].calls + 1 {
            failures.push(format!("white frame {}: no map update", k - fill));
        }
        for (slot, &word) in ticks[k].words.iter().enumerate() {
            if word != 0x0EEE && !SNOW.contains(&slot) {
                failures.push(format!(
                    "white frame {}: word {slot} is {word:04X}",
                    k - fill
                ));
            }
        }
    }
    let restore = fill + 2;
    if !ticks[restore].emits(|op| {
        matches!(
            op,
            SceneOp::Presentation {
                op: PresentationOp::PaletteRestoreTone { frames: 40 }
            }
        )
    }) {
        failures.push("no forty-frame restore after the two frames".to_owned());
    }
    let start = ticks[restore - 1].words;
    let mut steps = 0;
    for k in restore..restore + 40 {
        let tick = &ticks[k];
        if tick.calls != ticks[k - 1].calls {
            failures.push(format!("restore frame {}: a map update", k - restore));
        }
        if palette::tone_phase(tick.count) {
            steps += 1;
        }
        for slot in (0..64).filter(|slot| !SNOW.contains(slot)) {
            for shift in [1, 5, 9] {
                let want = level(start[slot], shift)
                    .saturating_sub(steps)
                    .max(level(saved[slot], shift));
                if level(tick.words[slot], shift) != want {
                    failures.push(format!(
                        "restore frame {}: word {slot} is {:04X}",
                        k - restore,
                        tick.words[slot]
                    ));
                    break;
                }
            }
        }
    }
    if steps != 10 {
        failures.push(format!("{steps} restore steps"));
    }
    let end = &ticks[restore + 39].words;
    for slot in (0..64).filter(|slot| !SNOW.contains(slot)) {
        if end[slot] != saved[slot] & 0x0EEE {
            failures.push(format!("word {slot} not restored: {:04X}", end[slot]));
        }
    }
    failures
}

/// Both flashes whiten the palette, run two map-update frames, and restore
/// the saved words over forty frames with no map update; the panel's 120
/// map-update frames come before them, and eighty after the creations.
#[test]
fn the_eclipse_torch_flashes_twice_and_restores_the_cram() {
    if !pack_present() {
        return;
    }
    let (ticks, runtime) = torch_run();
    let fills: Vec<usize> = (0..ticks.len())
        .filter(|&k| ticks[k].emits(is_white))
        .collect();
    assert_eq!(fills.len(), 2, "two white fills");
    assert_eq!(fills[1] - fills[0], 42, "two frames and forty between them");
    for &fill in &fills {
        assert!(
            ticks[fill].emits(|op| matches!(op, SceneOp::PlaySound { id: 0xDC })),
            "Deban with the fill"
        );
        assert_eq!(flash_failures(&ticks, fill), Vec::<String>::new());
    }
    // The panel's DMA (`$070230`) spends one bare frame, then
    // `DoMapUpdateLoop($77)` spends 120 map-update frames before the fill.
    let panel = ticks
        .iter()
        .position(|tick| tick.emits(|op| matches!(op, SceneOp::PanelCreate { id: 0x10D })))
        .expect("the panel");
    assert!(ticks[panel].emits(|op| matches!(op, SceneOp::DmaPlanes)));
    assert_eq!(ticks[panel].calls, ticks[panel - 1].calls);
    assert_eq!(fills[0] - panel, 1 + 120);
    assert_eq!(ticks[fills[0] - 1].calls - ticks[panel - 1].calls, 120);
    // After the second 2 + 40 flash frames, destroying the panel runs
    // `DMAPlanes_VInt` (`$0702C8`): one bare frame before the creations.
    let dma = fills[1] + 2 + 40;
    assert!(ticks[dma].emits(|op| matches!(op, SceneOp::DmaPlanes)));
    assert_eq!(ticks[dma].calls, ticks[dma - 1].calls);
    let objects = dma + 1;
    let created = ticks[objects]
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::CreateEclipseTorch(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(created, 9, "nine creations on one tick");
    // `DMAPlane_B_VInt`'s frame, then the sound and eighty frames.
    assert_eq!(ticks[objects].calls, ticks[objects - 1].calls);
    assert!(ticks[objects + 1].emits(|op| matches!(op, SceneOp::PlaySound { id: 0xB9 })));
    assert_eq!(ticks[objects + 80].calls - ticks[objects].calls, 80);
    assert!(
        ticks[objects + 81]
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneDialogue { entry: 0x2A })),
        "dialogue $2A after the eightieth frame"
    );
    assert!(runtime.game().is_set(Flag::event(0x9C)));
}

/// The checks reject a flash that never whitens, one whose restore runs map
/// updates, and one cut short.
#[test]
fn control_the_flash_checks_reject_a_wrong_palette_or_update() {
    if !pack_present() {
        return;
    }
    let (ticks, _) = torch_run();
    let fill = (0..ticks.len())
        .find(|&k| ticks[k].emits(is_white))
        .expect("a fill");
    let mut unwhite: Vec<Tick> = ticks.iter().map(copy).collect();
    unwhite[fill].words = unwhite[fill - 1].words;
    assert!(flash_failures(&unwhite, fill)[0].starts_with("white frame 0: word 0 is"));
    let mut updating: Vec<Tick> = ticks.iter().map(copy).collect();
    for tick in &mut updating[fill + 5..] {
        tick.calls += 1;
    }
    assert_eq!(
        flash_failures(&updating, fill),
        ["restore frame 3: a map update"]
    );
    let mut short: Vec<Tick> = ticks.iter().map(copy).collect();
    for k in fill + 22..fill + 42 {
        short[k].words = short[fill + 21].words;
    }
    assert!(
        flash_failures(&short, fill)
            .iter()
            .any(|failure| failure.starts_with("word ") && failure.contains("not restored"))
    );
}

fn copy(tick: &Tick) -> Tick {
    Tick {
        count: tick.count,
        events: tick.events.clone(),
        words: tick.words,
        calls: tick.calls,
    }
}

mod objects;
