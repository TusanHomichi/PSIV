//! Build-dependent memory evidence policy. Numeric debug-era diagnostics live
//! in the existing replay adapter; this module alone owns counter capability,
//! normalization charges and the selection of checks that establish a pass.
#![forbid(unsafe_code)]

use serde_json::{Value, json};

type Result<T> = std::result::Result<T, String>;

fn mode(samples: &[Value]) -> Result<bool> {
    let mut available = None;
    for sample in samples {
        let Some(sample) = sample.as_object() else {
            continue;
        };
        let current = match sample.get("static_available") {
            None => true, // Historical receipts came from a debug engine.
            Some(Value::Bool(value)) => *value,
            _ => return Err("inconsistent static-memory availability".into()),
        };
        if available.is_some_and(|previous| previous != current)
            || (!current && sample.get("static_kb").and_then(Value::as_i64) != Some(0))
        {
            return Err("inconsistent static-memory availability".into());
        }
        available = Some(current);
    }
    Ok(available.unwrap_or(true))
}

fn charge(sample: &mut Value, available: bool) {
    let Some(sample) = sample.as_object_mut() else {
        return;
    };
    let bytes = sample
        .get("cache_bytes")
        .and_then(Value::as_f64)
        .or_else(|| {
            sample
                .get("caches")?
                .as_object()?
                .values()
                .try_fold(0.0, |sum, cache| Some(sum + cache.get("bytes")?.as_f64()?))
        })
        .unwrap_or(0.0);
    sample.insert(
        "static_cache_kb".into(),
        json!(if available { bytes / 1024.0 } else { 0.0 }),
    );
}

fn prepare(mut request: Value) -> Result<Value> {
    let mut modes = Vec::new();
    if let Some(samples) = request.get("samples").and_then(Value::as_array) {
        modes.push(mode(samples)?);
    }
    if let Some(probes) = request.get_mut("probes").and_then(Value::as_object_mut) {
        for name in ["start", "end"] {
            let Some(probe) = probes.get_mut(name).and_then(Value::as_object_mut) else {
                continue;
            };
            let Some(samples) = probe.get_mut("samples").and_then(Value::as_array_mut) else {
                continue;
            };
            let available = mode(samples)?;
            modes.push(available);
            let orphan_available = match probe.get("orphan_monitor_available") {
                None => true,
                Some(Value::Bool(value)) => *value,
                _ => return Err("inconsistent orphan-monitor availability".into()),
            };
            if orphan_available != available {
                return Err("inconsistent orphan-monitor availability".into());
            }
            if available && probe.get("orphan_monitor_alive") != Some(&Value::Bool(true)) {
                return Err(format!(
                    "{name} probe its orphan-node monitor did not count a test orphan"
                ));
            }
            if !available {
                let samples = probe["samples"].as_array().expect("checked above");
                if samples
                    .iter()
                    .any(|item| item.get("orphan_nodes").and_then(Value::as_i64) != Some(0))
                {
                    return Err("an unavailable orphan monitor reported nodes".into());
                }
            }
            for sample in probe["samples"].as_array_mut().expect("checked above") {
                charge(sample, available);
            }
        }
    }
    let available = modes.first().copied().unwrap_or(true);
    if modes.iter().any(|value| *value != available) {
        return Err("samples and probes disagree on static-memory availability".into());
    }
    if let Some(samples) = request.get_mut("samples").and_then(Value::as_array_mut) {
        for sample in samples {
            charge(sample, available);
        }
    }
    Ok(
        json!({"samples": request.get("samples"), "probes": request.get("probes"),
              "static_available": available}),
    )
}

fn unavailable_bracket(verdict: &mut Value) {
    if let Some(verdict) = verdict.as_object_mut() {
        for key in ["static_growth_mb", "total_static_growth_mb", "orphan_nodes"] {
            verdict.insert(key.into(), Value::Null);
        }
        for key in ["start_floor", "end_floor"] {
            if let Some(floor) = verdict.get_mut(key).and_then(Value::as_object_mut) {
                floor.insert("static_mb".into(), Value::Null);
            }
        }
    }
}

fn finalize(request: Value) -> Result<Value> {
    let available = request
        .get("static_available")
        .and_then(Value::as_bool)
        .ok_or("finalize needs a boolean static_available")?;
    let bracket_only = request
        .get("bracket_only")
        .and_then(Value::as_bool)
        .ok_or("finalize needs a boolean bracket_only")?;
    let mut verdict = request
        .get("verdict")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or("finalize needs a verdict object")?;
    if bracket_only {
        // Failure verdicts carry a reason rather than numeric bracket terms.
        if verdict.get("reason").is_some() {
            return Ok(verdict);
        }
        let static_growth = request
            .get("static_growth_kb")
            .and_then(Value::as_f64)
            .ok_or("bracket lacks static growth")?;
        let rust_growth = request
            .get("rust_growth_kb")
            .and_then(Value::as_f64)
            .ok_or("bracket lacks Rust growth")?;
        let bound = request
            .get("bound_mb")
            .and_then(Value::as_f64)
            .ok_or("bracket lacks bound")?;
        let object_bound = request
            .get("object_bound")
            .and_then(Value::as_i64)
            .ok_or("bracket lacks object bound")?;
        let objects = verdict
            .get("object_growth")
            .and_then(Value::as_i64)
            .ok_or("bracket lacks object growth")?;
        let orphans = verdict
            .get("orphan_nodes")
            .and_then(Value::as_i64)
            .ok_or("bracket lacks orphan count")?;
        let caches = verdict
            .get("caches")
            .and_then(Value::as_object)
            .ok_or("bracket lacks caches")?;
        let pass = (!available || static_growth / 1024.0 <= bound)
            && rust_growth / 1024.0 <= bound
            && objects <= object_bound
            && (!available || orphans == 0)
            && caches
                .values()
                .all(|cache| cache.get("pass") == Some(&Value::Bool(true)));
        verdict["pass"] = json!(pass);
        verdict["static_available"] = json!(available);
        verdict["orphan_monitor_available"] = json!(available);
        if !available {
            unavailable_bracket(&mut verdict);
        }
        return Ok(verdict);
    }
    let mut parts = vec!["rust_heap", "bracket", "objects"];
    if available {
        parts.push("godot_static");
    }
    // A missing or malformed check never counts as a pass.
    verdict["pass"] = json!(
        parts
            .iter()
            .all(|part| verdict.get(part).and_then(|value| value.get("pass"))
                == Some(&Value::Bool(true)))
    );
    verdict["static_available"] = json!(available);
    if let Some(term) = verdict
        .get_mut("godot_static")
        .and_then(Value::as_object_mut)
    {
        term.insert("judged".into(), json!(available));
        if !available {
            term.insert("pass".into(), Value::Null);
            term.insert("growth_mb".into(), Value::Null);
        }
    }
    if !available {
        if let Some(bracket) = verdict.get_mut("bracket") {
            unavailable_bracket(bracket);
        }
        if let Some(drift) = verdict
            .get_mut("static_drift")
            .and_then(Value::as_object_mut)
        {
            for key in ["drift_mb", "within_bound"] {
                drift.insert(key.into(), Value::Null);
            }
        }
        if let Some(maps) = verdict.get_mut("maps").and_then(Value::as_object_mut) {
            for map in maps.values_mut().filter_map(Value::as_object_mut) {
                for key in ["first_floor_static_mb", "static_growth_mb"] {
                    map.insert(key.into(), Value::Null);
                }
            }
        }
    }
    Ok(verdict)
}

/// A request-level error is explicit JSON and can never be read as a pass.
pub fn evaluate(request: Value) -> Value {
    let result = match request.get("operation").and_then(Value::as_str) {
        Some("prepare") => prepare(request),
        Some("finalize") => finalize(request),
        _ => Err("unknown memory policy operation".into()),
    };
    result.unwrap_or_else(|error| json!({"error": error}))
}
