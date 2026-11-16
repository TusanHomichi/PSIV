use super::*;
use crate::dialogue::retirement::emission;
use serde_json::{Value, json};

#[test]
fn an_empty_census_keeps_unused_ids_characters_and_zero_totals() {
    let c = Value::from(&Census::default().to_json(
        &BTreeSet::from([1, 2]),
        &BTreeSet::new(),
        &crate::dialogue::trees::control_codes(),
    ));
    assert_eq!(c["entries"], 0);
    assert_eq!(c["portrait_ids_unused"], json!([1, 2]));
    assert_eq!(
        c["unused_charset_bytes"].as_array().unwrap().len(),
        dialogue_charset().size()
    );
}

#[test]
fn census_reports_retail_controls_portrait_references_pages_and_action_totals() {
    let Some(e) = emission() else { return };
    let c = &e.fragment["census"];
    assert_eq!(c["entries"], 2736);
    assert_eq!(c["empty_entries"], 1098);
    assert_eq!(
        c["control_codes"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        [
            "0xF2", "0xF3", "0xF4", "0xF5", "0xF6", "0xF7", "0xF9", "0xFA", "0xFC", "0xFD"
        ]
    );
    assert_eq!(
        c["unused_control_codes"],
        json!(["0xF0", "0xF1", "0xF8", "0xFB", "0xFE", "0xFF"])
    );
    assert_eq!(c["longest_line"], 32);
    assert_eq!(c["lines_past_the_window"], json!([]));
    assert_eq!(c["preamble"]["outside_preamble"], json!([]));
    assert_eq!(c["preamble"]["events"], 69);
    assert_eq!(c["preamble"]["keep_npc_facing"], 33);
    assert_eq!(c["portrait_ids_outside_table"], json!([]));
    assert_eq!(c["portrait_ids_unused"], json!([18, 22, 24, 30]));
    assert_eq!(
        c["portrait_positions"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
    assert!(c["portrait_ids"].get("0").is_some());
    assert_eq!(
        c["page_endings"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["choice", "close", "end", "full", "wait"]
    );
    assert_eq!(
        c["pages"].as_u64().unwrap(),
        c["page_endings"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>()
    );
    assert_eq!(
        c["unused_charset_bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["char"].clone())
            .collect::<Vec<_>>(),
        json!(["X", "*", "<", ">", "%", "6", "7", "9", "[", "(", ")", "="])
            .as_array()
            .unwrap()
            .to_vec()
    );
    let blank = c["blank_glyphs_in_charset"].as_array().unwrap();
    assert_eq!(
        blank.iter().map(|v| v["char"].clone()).collect::<Vec<_>>(),
        json!(["[", "(", ")", "="]).as_array().unwrap().to_vec()
    );
    assert!(blank.iter().all(|v| v["used"] == false));
    let expected = json!({"load_panel":165,"destroy_last_panel":35,"destroy_all_panels":7,"load_sound":35,"load_sound_2":14,"update_palette":4,"zio_eyes_red":1,"pause_music":1,"resume_music":1,"sabotage_alarm_red_palette":1,"set_event_flag":1,"elsydeon_broken":1});
    assert_eq!(c["text_actions"], expected);
    // Preserve the research census's 266 occurrences, 163 panel payloads,
    // and exact tree/entry/segment locations without storing cartridge text.
    let trees = e.document("trees");
    let mut actions = BTreeMap::<String, usize>::new();
    let mut panels = BTreeSet::new();
    let mut locations = BTreeMap::<(usize, usize, usize), String>::new();
    let mut portraits = BTreeSet::new();
    for tree in trees["trees"].as_array().unwrap() {
        for entry in tree["entries"].as_array().unwrap() {
            for (i, s) in entry["segments"].as_array().unwrap().iter().enumerate() {
                if s["ctrl"] == "portrait" {
                    portraits.insert(s["id"].as_u64().unwrap());
                }
                if s["ctrl"] != "action" {
                    continue;
                }
                let name = s["action"].as_str().unwrap();
                *actions.entry(name.into()).or_default() += 1;
                locations.insert(
                    (
                        tree["tree"].as_u64().unwrap() as usize,
                        entry["id"].as_u64().unwrap() as usize,
                        i,
                    ),
                    name.into(),
                );
                if name == "load_panel" {
                    panels.insert(s["panel"].as_u64().unwrap());
                }
            }
        }
    }
    assert_eq!(json!(actions), expected);
    assert_eq!(locations.len(), 266);
    assert_eq!(panels.len(), 163);
    let known = e.document("portraits")["portraits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(portraits.iter().all(|p| *p == 0 || known.contains(p)));
    assert_eq!(
        portraits,
        c["portrait_ids"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.parse().unwrap())
            .collect()
    );
}
