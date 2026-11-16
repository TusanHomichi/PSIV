use super::*;

fn captures() -> Value {
    json!([
        {"label":"before","tick":1.0,"word47":3822.0,"visible":true,"viewport":[1280,800]},
        {"label":"fall","tick":2.0,"word47":2184.0,"visible":true,"viewport":[1280,800]},
        {"label":"dark","tick":3.0,"word47":546.0,"visible":true,"viewport":[1280,800]},
        {"label":"rise","tick":4.0,"word47":2730.0,"visible":true,"viewport":[1280,800]},
        {"label":"after","tick":5.0,"word47":3822.0,"visible":true,"viewport":[1280,800]},
        {"label":"cleared","tick":6.0,"word47":3822.0,"visible":false,"viewport":[1280,800]}
    ])
}

fn comparisons() -> Value {
    json!(
        PAIRS
            .iter()
            .map(|p| json!({"label":p.label,"rmse":"0.000000",
        "comparison":"Rust comparison report"}))
            .collect::<Vec<_>>()
    )
}

fn negative() -> Value {
    json!({"rmse":"29.006627","comparison":"Rust negative-control report"})
}

#[test]
fn all_six_phases_require_ordered_ticks_palette_visibility_and_viewport() {
    assert!(validate_observations(&captures()).is_ok());
    for field in ["label", "tick", "word47", "visible", "viewport"] {
        let mut bad = captures();
        bad[2].as_object_mut().unwrap().remove(field);
        assert!(validate_observations(&bad).is_err(), "missing {field}");
    }
    for bad in [Value::Null, json!([]), json!([captures()[0]])] {
        assert!(validate_observations(&bad).is_err());
    }
    let mut bad = captures();
    bad.as_array_mut().unwrap().swap(0, 1);
    assert!(validate_observations(&bad).is_err());
    for (field, wrong) in [
        ("tick", json!(0)),
        ("tick", json!(1.5)),
        ("tick", json!(-1)),
        ("word47", json!(0)),
        ("visible", json!(false)),
        ("viewport", json!([320, 224])),
        ("viewport", json!([1280, 800, 1])),
    ] {
        let mut bad = captures();
        bad[2][field] = wrong;
        assert!(validate_observations(&bad).is_err(), "{field}: {bad}");
    }
    let mut bad = captures();
    bad[5]["visible"] = json!(true);
    assert!(
        validate_observations(&bad).is_err(),
        "clear must remove text"
    );
}

#[test]
fn missing_reordered_or_nonzero_comparisons_never_pass() {
    assert!(validate_comparisons(&comparisons(), &negative(), &json!([])).is_ok());
    for metric in [
        Value::Null,
        json!(0),
        json!("0"),
        json!("0.000001"),
        json!("NaN"),
    ] {
        let mut bad = comparisons();
        bad[1]["rmse"] = metric;
        assert!(validate_comparisons(&bad, &negative(), &json!([])).is_err());
    }
    let mut reordered = comparisons();
    reordered.as_array_mut().unwrap().swap(1, 2);
    assert!(validate_comparisons(&reordered, &negative(), &json!([])).is_err());
    assert!(validate_comparisons(&json!([]), &negative(), &json!([])).is_err());
    let mut missing_report = comparisons();
    missing_report[0]["comparison"] = json!("");
    assert!(validate_comparisons(&missing_report, &negative(), &json!([])).is_err());
}

#[test]
fn negative_control_must_fail_with_a_finite_positive_metric() {
    for metric in [
        Value::Null,
        json!(0),
        json!("0.000000"),
        json!("-1"),
        json!("NaN"),
        json!("inf"),
        json!("not a metric"),
    ] {
        let bad = json!({"rmse":metric,"comparison":"report"});
        assert!(validate_comparisons(&comparisons(), &bad, &json!([])).is_err());
    }
    assert!(validate_comparisons(&comparisons(), &negative(), &json!(["leak"])).is_err());
    assert!(validate_comparisons(&comparisons(), &negative(), &Value::Null).is_err());
}

#[test]
fn oracle_identity_rejects_changed_or_missing_bytes() {
    let directory = std::env::temp_dir().join(format!("staff-pin-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("frame.png");
    std::fs::write(&path, b"synthetic frame").unwrap();
    let digest = sha256(b"synthetic frame");
    assert!(pinned(&path, &digest).is_ok());
    std::fs::write(&path, b"changed frame").unwrap();
    assert!(pinned(&path, &digest).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(pinned(&path, &digest).is_err());
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn the_bridge_exposes_the_plan_and_rejects_invalid_observation_files() {
    let plan = crate::exchange(&json!({"operation":"ending-staff-plan"})).unwrap();
    assert_eq!(plan["pairs"].as_array().unwrap().len(), 6);
    assert_eq!(plan["region"], json!([24, 64, 256, 88]));
    assert_eq!(plan["tape"], "oracle/tapes/47_ending_staff.tape");
    let directory = std::env::temp_dir().join(format!("staff-rows-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("captures.json");
    std::fs::write(&file, captures().to_string()).unwrap();
    let request = json!({"operation":"ending-staff-observations","directory":directory});
    assert_eq!(crate::exchange(&request).unwrap(), captures());
    for bad in ["not JSON", "[]"] {
        std::fs::write(&file, bad).unwrap();
        assert!(crate::exchange(&request).is_err());
    }
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
