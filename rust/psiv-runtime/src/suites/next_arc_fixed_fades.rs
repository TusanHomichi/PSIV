//! Actual CRAM op dispatch in the arc, folding Init's internal fade-out.
use super::*;

pub(super) fn assert_fades(events: &[RuntimeEvent], expected: &[SceneOp]) {
    let mut seen = Vec::new();
    for event in events {
        if let RuntimeEvent::ScenePresentation { op, .. } = event {
            match op {
                SceneOp::InitVramAndCram => {
                    assert_eq!(seen.pop(), Some(SceneOp::FadeOut));
                    seen.push(*op);
                }
                SceneOp::FadeIn | SceneOp::FadeOut => seen.push(*op),
                _ => {}
            }
        }
    }
    assert_eq!(seen, expected);
}
