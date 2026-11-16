//! Remaining tests/test_shop_flow.py decoder contracts (#560).
use super::*;
use crate::test_support::retail_rom;
use serde_json::{Value, json};

#[test]
fn retail_flow_keeps_position_prices_recovery_and_presentation_rules() {
    let Some(rom) = retail_rom("shop flow rules") else {
        return;
    };
    let flow = read_flow(&rom).unwrap();
    let rules = Value::from(&flow.json);
    let scan = &rules["interaction"];
    assert_eq!(scan["table"], hex6(tables::LOCATIONS_START));
    assert_eq!(scan["entry_bytes"], tables::LOCATION_ENTRY_BYTES);
    assert_eq!(scan["terminator"], "0xFFFF");
    assert_eq!(scan["selector_ram"], "0xFFFFECD0");
    assert_eq!(scan["matches_on"], "position");
    let fields: Vec<_> = scan["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["offset"].clone(), f["compared_against"].clone()))
        .collect();
    assert_eq!(
        fields,
        vec![
            (json!("0x00"), json!("d2")),
            (json!("0x02"), json!("d0")),
            (json!("0x04"), json!("d1")),
            (json!("0x06"), Value::Null)
        ]
    );
    let prices = &rules["prices"];
    assert_eq!(prices["price_offset"], "0x14");
    assert_eq!(prices["price_bytes"], 2);
    assert_eq!(prices["encoding"], "binary");
    assert_eq!(prices["sell"]["divisor"], 2);
    assert_eq!(prices["sell"]["arithmetic"], "lsr.w #1");
    assert_eq!(flow.sell_divisor, 2);
    assert_eq!(prices["stock"]["unlimited"], true);
    for (buy, sell) in [(11, 5), (10, 5), (0, 0)] {
        assert_eq!(sell_price(buy, flow.sell_divisor), sell);
    }
    let inn = &rules["inn"];
    assert_eq!(inn["cost"]["formula"], "rate * party_slots");
    assert_eq!(inn["cost"]["max_party"], 5);
    assert_eq!(inn["entry_bytes"], 2);
    assert_eq!(inn["rate_offset"], "0x01");
    assert_eq!(inn["scripted_rest"]["selector"], "0x0006");
    let writes = inn["restores"]["per_character"].as_array().unwrap();
    assert_eq!(writes.len(), 11);
    for (write, (field, source, bytes)) in writes.iter().zip([
        ("curr_hp", Some("max_hp"), 2),
        ("curr_tp", Some("max_tp"), 2),
        ("status", None, 1),
    ]) {
        assert_eq!(write["field"], field);
        assert_eq!(write["from"], json!(source));
        assert_eq!(write["bytes"], bytes);
    }
    assert_eq!(writes[2]["value"], 0);
    for (slot, write) in writes[3..].iter().enumerate() {
        assert_eq!(write["field"], format!("curr_skill_uses[{slot}]"));
        assert_eq!(write["from"], format!("max_skill_uses[{slot}]"));
        assert_eq!(write["bytes"], 1);
    }
    assert_eq!(flow.portrait_groups.each_ref().map(Vec::len), [18, 49]);
    assert_eq!(
        flow.portrait_groups
            .iter()
            .flatten()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
    assert_eq!(
        rules["presentation"]["portraits"]["alternate_group_uses"],
        1
    );
    assert_eq!(rules["presentation"]["greetings"]["entry_bytes"], 2);
    assert_eq!(rules["presentation"]["greetings"]["table"], "0x068136");
    assert_eq!(flow.greetings_table, 0x68136);
    // Read the table locally rather than committing the extracted rate vector.
    assert_eq!(flow.inn_table, 0x68112);
    assert_eq!(flow.greetings_table - flow.inn_table, 18 * 2);
    for (group, base, end) in [(0, 0x68006, 0x6804e), (1, 0x6804e, flow.inn_table)] {
        let pointers: Vec<_> = (base..end)
            .step_by(4)
            .map(|at| l(rom.bytes(), at).unwrap())
            .collect();
        assert_eq!(flow.portrait_groups[group], pointers);
    }
}

#[test]
fn retail_signatures_have_their_declared_counts_and_damage_fails_closed() {
    let Some(rom) = retail_rom("shop flow refusals") else {
        return;
    };
    let data = rom.bytes();
    for (pin, label) in [
        (COUNTER_SCAN, "CounterScan"),
        (COUNTER_SCAN_CALL, "CounterScanCall"),
        (BUY_PRICE, "BuyPrice"),
        (SELL_PRICE, "SellPrice"),
        (INN_RATES, "InnRates"),
        (INN_PARTY_MULTIPLY, "InnPartyMultiply"),
        (GREETING_SELECTOR, "GreetingSelector"),
        (PORTRAIT_GROUPS, "PortraitGroups"),
        (RECOVER_HP, "RecoverHp"),
        (REST_EVENT, "RestEvent"),
    ] {
        let found = sites(data, pin, label).unwrap();
        assert_eq!(found.len(), pin.1, "{label}");
        let mut broken = data.to_vec();
        broken[found[0]] ^= 0xff;
        let error = match read_flow(&Rom::for_tests(broken)) {
            Err(error) => error,
            Ok(_) => panic!("lost {label} signature was accepted"),
        };
        assert!(error.0.contains(label), "{label}: {error}");
    }
    let scan = site(data, COUNTER_SCAN, "CounterScan").unwrap();
    let sell = site(data, SELL_PRICE, "SellPrice").unwrap();
    let inn = site(data, INN_RATES, "InnRates").unwrap();
    // A lost scan, a non-shifting sale and a lost rate-index doubling are
    // the three Python refusal checks. Additional scan/body mutations keep
    // operand decoding covered independently of the signature census.
    for (at, message) in [
        (scan, "CounterScan"),
        (sell + 4, "SellPrice"),
        (inn + 10, "add.w d0, d0"),
        (scan + 10, "cmpi.w #terminator"),
        (scan + 16, "unexpected opcode"),
    ] {
        let mut broken = data.to_vec();
        broken[at..at + 2].copy_from_slice(&0x4e71u16.to_be_bytes());
        let error = match read_flow(&Rom::for_tests(broken)) {
            Err(error) => error,
            Ok(_) => panic!("damaged flow instruction was accepted"),
        };
        assert!(error.0.contains(message), "{message}: {error}");
    }
}
