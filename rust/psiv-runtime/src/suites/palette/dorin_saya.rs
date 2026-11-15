//! #655: the first Saya visit fades before the common party/map tail;
//! a repeat visit jumps over that fade. These are state/timing fixtures.
use super::{Frame, next_event, pack_present, runtime_on, trace};
use crate::RuntimeEvent;
use psiv_core::{CharId, Flag, SceneOp};

fn is_op(event: &RuntimeEvent, want: SceneOp) -> bool {
    matches!(event, RuntimeEvent::ScenePresentation { op, .. } if *op == want)
}

fn first(frames: &[Frame], pick: impl Fn(&RuntimeEvent) -> bool) -> usize {
    frames
        .iter()
        .position(|f| f.events.iter().any(&pick))
        .expect("event emitted")
}

#[test]
fn saya_fades_to_black_before_the_party_changes_and_skips_it_on_repeat() {
    if !pack_present() {
        return;
    }
    for visited in [false, true] {
        let party = [0, 1, 2, 4, 5].map(|id| Some(CharId(id)));
        let flags = if visited {
            vec![Flag::event(0x12)]
        } else {
            vec![]
        };
        let mut rt = runtime_on(0x8B, party, &flags);
        let (_, frames) = trace(&mut rt, 0x8009);
        let common = first(&frames, |e| is_op(e, SceneOp::PlaySound { id: 0xA5 }));
        let saya = frames.iter().position(|f| {
            f.events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneDialogue { entry: 0x2C }))
        });
        assert_eq!(saya.is_some(), !visited);
        if let Some(saya) = saya {
            let out = next_event(&frames, saya, |e| is_op(e, SceneOp::FadeOut));
            let before = frames[out - 1].words;
            // PalFadeOut_ClrSpriteTbl subtracts one from each nibble per
            // VBlank. Its longest nibble is the number of frames it spends.
            let spent = usize::from(
                before
                    .iter()
                    .flat_map(|w| [w & 15, (w >> 4) & 15, (w >> 8) & 15, w >> 12])
                    .max()
                    .unwrap()
                    .max(1),
            );
            assert!(spent > 1, "a lit palette makes the missing fade observable");
            assert_eq!(common - out, spent, "party/music tail waits for every pass");
            assert_ne!(frames[out + spent - 2].words, [0; 64]);
            assert_eq!(frames[out + spent - 1].words, [0; 64]);
        }
        // InitVRAMAndCRAM emits its own FadeOut twice on both paths. Only
        // the first visit has the third, explicit fade at $074EBC.
        assert_eq!(
            frames[..common]
                .iter()
                .flat_map(|f| &f.events)
                .filter(|e| is_op(e, SceneOp::FadeOut))
                .count(),
            if visited { 2 } else { 3 }
        );
        let map = first(&frames[common..], |e| {
            matches!(e, RuntimeEvent::MapChanged { .. })
        }) + common;
        assert_eq!(frames[map].words, [0; 64], "common inn reload stays dark");
        let fade_in = next_event(&frames, map, |e| is_op(e, SceneOp::FadeIn));
        let text = next_event(&frames, fade_in, |e| {
            matches!(e, RuntimeEvent::SceneDialogue { entry: 0x2D })
        });
        assert_ne!(frames[text].words, [0; 64], "common dialogue is lit");
        assert_eq!(
            rt.game().party_members(),
            vec![CharId(0), CharId(4), CharId(5), CharId(6)]
        );
        assert_eq!(rt.map_id().0, 0x3F);
        assert!(rt.game().is_set(Flag::event(0x12)));
        assert!(rt.game().is_set(Flag::event(0x47)));
    }
}
