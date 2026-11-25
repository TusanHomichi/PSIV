//! Actual CRAM op dispatch in the arc, folding Init's internal fade-out.
use crate::RuntimeEvent;
use psiv_core::SceneOp;

pub(super) fn assert_fades(events: &[RuntimeEvent], expected: &[SceneOp]) {
    let mut seen = Vec::new();
    for event in events {
        if let RuntimeEvent::ScenePresentation { op, .. } = event {
            match op {
                SceneOp::InitVramAndCram => {
                    assert_eq!(seen.pop(), Some(SceneOp::FadeOut));
                    seen.push(op.clone());
                }
                SceneOp::FadeIn | SceneOp::FadeOut => seen.push(op.clone()),
                _ => {}
            }
        }
    }
    assert_eq!(seen, expected);
}
