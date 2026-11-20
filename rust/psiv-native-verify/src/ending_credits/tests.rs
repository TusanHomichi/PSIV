use super::*;

fn captures() -> Value {
    json!(
        PAIRS
            .iter()
            .enumerate()
            .map(|(index, pair)| json!({
                "label": pair.label,
                "tick": (index as f64) + 1.0,
                "stage": pair.stage as f64,
                "text_counter": pair.text_counter as f64,
                "fg_y": pair.fg_y as f64,
                "bg_y": pair.bg_y as f64,
                "column_scroll": pair.column_scroll,
                "word0": pair.word0 as f64,
                "word18": pair.word18 as f64,
                "word30": pair.word30 as f64,
                "word47": pair.word47 as f64,
                "word50": pair.word50 as f64,
                "viewport": [1280, 800]
            }))
            .collect::<Vec<_>>()
    )
}

fn comparisons() -> Value {
    json!(
        PAIRS
            .iter()
            .map(|p| json!({
                "label": p.label,
                "rmse": "0.000000",
                "comparison": "Rust comparison report"
            }))
            .collect::<Vec<_>>()
    )
}

fn negative() -> Value {
    json!({"rmse": "99.123456", "comparison": "Rust negative-control report"})
}

#[test]
fn all_eight_phases_require_ordered_ticks_state_cram_and_viewport() {
    assert!(validate_observations(&captures()).is_ok());
    for field in [
        "label",
        "tick",
        "stage",
        "text_counter",
        "fg_y",
        "bg_y",
        "column_scroll",
        "word0",
        "word18",
        "word30",
        "word47",
        "word50",
        "viewport",
    ] {
        let mut bad = captures();
        bad[2].as_object_mut().unwrap().remove(field);
        assert!(validate_observations(&bad).is_err(), "missing {field}");
    }
    for bad in [Value::Null, json!([]), json!([captures()[0]])] {
        assert!(validate_observations(&bad).is_err());
    }
    let mut reordered = captures();
    reordered.as_array_mut().unwrap().swap(2, 3);
    assert!(validate_observations(&reordered).is_err());
    for (field, wrong) in [
        ("tick", json!(0)),
        ("stage", json!(1)),
        ("column_scroll", json!(false)),
        ("fg_y", json!(0)),
        ("word50", json!(0)),
        ("viewport", json!([320, 224])),
    ] {
        let mut bad = captures();
        bad[2][field] = wrong;
        assert!(validate_observations(&bad).is_err(), "{field}: {bad}");
    }
}

#[test]
fn missing_reordered_or_nonzero_comparisons_and_leaks_never_pass() {
    assert!(validate_comparisons(&comparisons(), &negative(), &json!([])).is_ok());
    for metric in [Value::Null, json!(0), json!("0.000001"), json!("NaN")] {
        let mut bad = comparisons();
        bad[4]["rmse"] = metric;
        assert!(validate_comparisons(&bad, &negative(), &json!([])).is_err());
    }
    let mut bad_control = negative();
    bad_control["rmse"] = json!("0.000000");
    assert!(validate_comparisons(&comparisons(), &bad_control, &json!([])).is_err());
    assert!(validate_comparisons(&comparisons(), &negative(), &json!(["leak"])).is_err());
}

#[test]
fn the_bridge_exposes_the_credits_plan_and_rejects_bad_observations() {
    let plan = crate::exchange(&json!({"operation":"ending-credits-plan"})).unwrap();
    assert_eq!(plan["pairs"].as_array().unwrap().len(), 8);
    assert_eq!(plan["region"], json!([0, 0, 320, 224]));
    assert_eq!(plan["tape"], "oracle/tapes/48_ending_credits.tape");
    let directory = std::env::temp_dir().join(format!("psiv-credits-rows-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("captures.json");
    std::fs::write(&file, captures().to_string()).unwrap();
    let request = json!({"operation":"ending-credits-observations","directory":directory});
    assert_eq!(crate::exchange(&request).unwrap(), captures());
    std::fs::write(&file, "[]").unwrap();
    assert!(crate::exchange(&request).is_err());
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
