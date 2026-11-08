use psiv_memory_verdict::evaluate;
use serde_json::{Value, json};

fn sample(available: bool) -> Value {
    json!({"static_available": available, "static_kb": if available { 100 } else { 0 },
           "rss_kb": 200, "orphan_nodes": 0, "cache_bytes": 4096})
}

fn request(available: bool) -> Value {
    json!({"operation": "prepare", "samples": [sample(available)], "probes": {
        "start": {"samples": [sample(available)], "orphan_monitor_available": available,
                  "orphan_monitor_alive": available},
        "end": {"samples": [sample(available)], "orphan_monitor_available": available,
                "orphan_monitor_alive": available}}})
}

#[test]
fn release_preserves_raw_rss_and_static_but_charges_no_unavailable_static_bytes() {
    for available in [false, true] {
        let input = request(available);
        let output = evaluate(input.clone());
        assert_eq!(output["static_available"], available);
        assert_eq!(
            output["samples"][0]["static_kb"],
            input["samples"][0]["static_kb"]
        );
        assert_eq!(output["samples"][0]["rss_kb"], 200);
        assert_eq!(
            output["samples"][0]["static_cache_kb"],
            if available { 4.0 } else { 0.0 }
        );
    }
}

#[test]
fn legacy_debug_receipts_keep_their_capability() {
    let mut input = request(true);
    input["samples"][0]
        .as_object_mut()
        .unwrap()
        .remove("static_available");
    assert_eq!(evaluate(input)["static_available"], true);
}

#[test]
fn contradictory_counter_capabilities_fail_closed() {
    for path in [
        "sample_mode",
        "nonzero_static",
        "orphan_mode",
        "nonzero_orphan",
        "dead_monitor",
        "probe_mode",
    ] {
        let mut input = request(false);
        match path {
            "sample_mode" => input["samples"][0]["static_available"] = json!(true),
            "nonzero_static" => input["samples"][0]["static_kb"] = json!(1),
            "orphan_mode" => input["probes"]["end"]["orphan_monitor_available"] = json!(true),
            "nonzero_orphan" => input["probes"]["end"]["samples"][0]["orphan_nodes"] = json!(1),
            "dead_monitor" => {
                input = request(true);
                input["probes"]["end"]["orphan_monitor_alive"] = json!(false);
            }
            "probe_mode" => {
                input["probes"]["end"]["samples"][0]["static_available"] = json!(true);
                input["probes"]["end"]["orphan_monitor_available"] = json!(true);
                input["probes"]["end"]["orphan_monitor_alive"] = json!(true);
            }
            _ => unreachable!(),
        }
        assert!(evaluate(input).get("error").is_some(), "{path}");
    }
}

fn final_request(available: bool) -> Value {
    json!({"operation": "finalize", "static_available": available, "bracket_only": false,
           "verdict": {"rust_heap": {"pass": true}, "objects": {"pass": true},
                       "bracket": {"pass": true}, "godot_static": {"pass": false, "growth_mb": 100},
                       "maps": {"1": {"static_growth_mb": 100}}, "static_drift": {"drift_mb": 100}}})
}

#[test]
fn release_selects_counted_checks_and_labels_unavailable_ones() {
    let output = evaluate(final_request(false));
    assert_eq!(output["pass"], true);
    assert_eq!(output["godot_static"]["judged"], false);
    assert!(output["godot_static"]["pass"].is_null());
    assert!(output["godot_static"]["growth_mb"].is_null());
    assert!(output["maps"]["1"]["static_growth_mb"].is_null());
    assert_eq!(evaluate(final_request(true))["pass"], false);
    for part in ["rust_heap", "objects", "bracket"] {
        let mut input = final_request(false);
        input["verdict"][part]["pass"] = json!(false);
        assert_eq!(evaluate(input)["pass"], false, "{part}");
    }
}

fn bracket_request(available: bool) -> Value {
    json!({"operation": "finalize", "static_available": available, "bracket_only": true,
           "bound_mb": 8, "object_bound": 64, "static_growth_kb": 9 * 1024,
           "rust_growth_kb": 0, "verdict": {"static_growth_mb": 9,
           "rust_live_growth_mb": 0, "object_growth": 0, "orphan_nodes": 0,
           "caches": {"tiles": {"pass": true}}}})
}

#[test]
fn release_bracket_checks_rust_objects_and_cache_ceilings_without_static_counter() {
    assert_eq!(evaluate(bracket_request(false))["pass"], true);
    assert_eq!(evaluate(bracket_request(true))["pass"], false);
    for defect in ["rust", "object", "cache", "fraction"] {
        let mut input = bracket_request(false);
        match defect {
            "rust" => input["rust_growth_kb"] = json!(9 * 1024),
            "fraction" => input["rust_growth_kb"] = json!(8.01 * 1024.0),
            "object" => input["verdict"]["object_growth"] = json!(65),
            "cache" => input["verdict"]["caches"]["tiles"]["pass"] = json!(false),
            _ => unreachable!(),
        }
        assert_eq!(evaluate(input)["pass"], false, "{defect}");
    }
}

#[test]
fn malformed_protocol_is_an_explicit_error_and_missing_checks_never_pass() {
    for request in [
        Value::Null,
        json!({}),
        json!({"operation": "finalize"}),
        json!({"operation": "other"}),
    ] {
        assert!(evaluate(request).get("error").is_some());
    }
    let mut input = final_request(false);
    input["verdict"]["objects"] = Value::Null;
    assert_eq!(evaluate(input)["pass"], false);
}
