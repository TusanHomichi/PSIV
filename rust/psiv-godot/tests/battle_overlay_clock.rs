//! The shell must use the runtime's battle-loop clock, never the display clock.
//! State tests pin the signal for each entry route; this boundary check catches
//! dropping that signal or leaving a second unconditional clock in the shell.

fn uses_runtime_clock(source: &str) -> bool {
    let code = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");
    let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
    code.matches(".advance_animation()").count() == 1
        && code.contains(
            "if battle.advance_enemy_overlays && let Some(screen) = self.battle_screen.as_mut() { screen.bind_mut().advance_animation(); }",
        )
}

#[test]
fn battle_enemy_overlays_have_one_runtime_gated_clock() {
    let source = include_str!("../src/battle/mod.rs");
    assert!(uses_runtime_clock(source));
    // Both ways of restoring the bug must fail: discard the hold signal, or
    // retain the old display-frame clock alongside the new presentation path.
    assert!(!uses_runtime_clock(
        &source.replace("if battle.advance_enemy_overlays", "if true")
    ));
    assert!(!uses_runtime_clock(&format!(
        "{source}\n screen.bind_mut().advance_animation();"
    )));
}
