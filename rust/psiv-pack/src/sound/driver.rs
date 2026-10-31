//! The driver file's research blocks and the DAC banks.

use super::*;

pub(super) fn command_vocabulary() -> Value {
    let primary: serde_json::Map<String, Value> = COMMAND_SPECS
        .iter()
        .map(|s| {
            let mut entry = json!({
                "opcode": s.opcode,
                "type": s.kind,
                "subtype": s.subtype,
                "length": s.length,
            });
            if let Some(note) = s.note {
                entry["note"] = json!(note);
            }
            if let Some(at) = s.relative_word_at {
                entry["relative_word_at"] = json!(at);
            }
            if s.variable {
                entry["variable"] = json!(true);
            }
            (hx(usize::from(s.opcode), 2), entry)
        })
        .collect();
    json!({
        "primary": primary,
        "meta": {
            hx(usize::from(META_PAN_ANIM), 2): {
                "opcode": META_PAN_ANIM,
                "type": META_PAN_ANIM_TYPE,
                "length": META_PAN_ANIM_LENGTH,
                "payload": META_PAN_ANIM_PAYLOAD,
            },
        },
        "primary_count": COMMAND_SPECS.len(),
        "meta_count": 1,
        "total_documented_entries": COMMAND_SPECS.len() + 1,
    })
}

fn fixed_constants(rom: &[u8]) -> Result<Value, PackError> {
    let bytes = |s: usize, e: usize| rom[s..e].to_vec();
    let frequencies = (0..12)
        .map(|i| be16(rom, 0xD0DE2 + i * 2))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "fm_init_bytes": bytes(0xD07D2, 0xD07D9),
        "psg_init_bytes": bytes(0xD07DA, 0xD07DD),
        "spc_fm3_registers": bytes(0xD0490, 0xD0498),
        "fm_algorithm_operator_masks": bytes(0xD1290, 0xD1298),
        "fm_operator_registers": bytes(0xD1308, 0xD131C),
        "fm_volume_registers": bytes(0xD131C, 0xD1320),
        "fm3_frequency_values": bytes(0xD1510, 0xD1518),
        "fm_frequencies_big_endian_words": frequencies,
        "psg_frequency_table_raw_hex": hex(&rom[0xD0FA2..0xD102E]),
        "pan_animation_sequences": {
            "0": hex(&rom[0xD053C..0xD053E]),
            "1": hex(&rom[0xD053E..0xD0541]),
            "2": hex(&rom[0xD0541..0xD0545]),
        },
    }))
}

pub(super) fn driver(rom: &[u8], raw_files: &mut RawFiles) -> Result<Driver, PackError> {
    let mut regions = BTreeMap::new();
    for (name, label, start, end, _) in KNOWN_REGIONS {
        let path = format!("raw/driver/{name}.bin");
        regions.insert(
            (*name).to_owned(),
            region(rom, raw_files, label, *start, *end, path)?,
        );
    }

    let general_start = 0xD1A60;
    let general = (0..8)
        .map(|i| be32(rom, general_start + i * 4).map(rom_offset))
        .collect::<Result<Vec<_>, _>>()?;
    let modulation = envelopes(rom, raw_files, "modulation", 0xD1A80, 8)?;
    let volume = envelopes(rom, raw_files, "volume", 0xD1B44, 10)?;

    let dac_raw = range(rom, 0xD1A3E, 0xD1A60, "zBankTbl")?;
    let bank_table_entries = dac_raw
        .as_chunks::<2>()
        .0
        .iter()
        .zip(0x81u8..)
        .map(|(pair, sound_id)| DacTableEntry {
            sound_id,
            bank: pair[0],
            sample: pair[1],
        })
        .collect();

    let control = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    Ok(Driver {
        provenance: json!({
            "rom_sha256": sha(rom),
            "rom_size_bytes": rom.len(),
            "source_tree": "reference/ps4disasm/sound",
            "source_files": [
                "DefCFlag.txt", "DefDrv.txt", "Notes.txt", "Pointers.txt",
                "ps4.sound_driver.asm", "ps4.dac_driver.asm",
            ],
            "clone_drift_guard": "ROM hash, structural anchors, and fixed region hashes are verified before extraction",
        }),
        timing: json!({
            "update_sound_rom_offset": rom_offset(0xD0008),
            "global_tempo": {
                "music_header_byte": 5,
                "reload_register": "$2(a6)",
                "counter_register": "$1(a6)",
                "semantics": "Each UpdateSound call decrements the counter; zero reloads it from the header byte and increments $E on each active channel.",
                "zero_behavior": "A zero reload disables the global tempo tick path.",
            },
            "channel_tick_multiplier": {
                "music_header_byte": 4,
                "sfx_header_byte": 2,
                "register": "$2(a5)",
                "semantics": "TickMultiplier multiplies a note/delay byte by this value before loading channel $E/$F.",
            },
            "note_tokens": {
                "delay": "bytes 0x00-0x7F consume one byte and load that delay",
                "rest_or_note": "0x80 is rest; 0x81-0xDF are notes; a following byte below 0x80 is consumed as delay, while a high-bit byte is the next token",
            },
        }),
        command_vocabulary: command_vocabulary(),
        loop_commands: json!({
            "F6": "signed big-endian relative word at opcode+1; target is opcode+2+offset",
            "F7": "index byte, count byte, signed big-endian relative word at opcode+3; taken target is opcode+4+offset and exhausted fallthrough is opcode+5",
            "F8": "signed big-endian relative word at opcode+1; target is opcode+2+offset and return address is opcode+3",
            "F9": "pop the most recent F8 return address; an empty stack is undecodable",
        }),
        fixed_constants: fixed_constants(rom)?,
        envelopes: Envelopes {
            modulation_control_bytes: control(&[
                ("0x80", "RESET"),
                ("0x81", "HOLD"),
                ("0x82", "LOOP"),
                ("0x83", "STOP"),
                ("0x84", "CHG_MULT then one multiplier byte"),
            ]),
            volume_control_bytes: control(&[
                ("0x80", "RESET"),
                ("0x81", "HOLD"),
                ("0x82", "JUMP2IDX then one index byte"),
                ("0x83", "OFF"),
            ]),
            modulation,
            volume,
        },
        pointer_tables: json!({
            "driver_block": {
                "rom_offset": rom_offset(general_start),
                "pointers": general,
                "meaning": [
                    "SndPriorities", "SpcSFXPtrs", "MusicPtrs", "SFXPtrs",
                    "ModEnvPtrs", "VolEnvPtrs", "regular SFX id base", "UpdateSound",
                ],
            },
            "music": {"rom_offset": rom_offset(MUSIC_PTRS), "id_first": 0x81, "count": MUSIC_COUNT},
            "sfx": {"rom_offset": rom_offset(SFX_PTRS), "id_first": 0xB5, "count": SFX_COUNT},
            "special_sfx": {"rom_offset": rom_offset(SPECIAL_PTRS), "id_first": 0xF8, "count": SPECIAL_COUNT},
        }),
        dac: Dac {
            z80_driver: regions["z80_dac_driver"].clone(),
            bank_table: regions["dac_bank_table"].clone(),
            bank_table_entries,
            bank_rom_bases: DAC_BANKS.iter().map(|b| rom_offset(b.1)).collect(),
            table_entry_size_bytes: 8,
            table_word_encoding: "ROM stores each Z80 little-endian word as the two bytes read by ld e,(hl); ld d,(hl+1); swap bytes when presenting the address/length.".to_owned(),
            banks: Vec::new(),
        },
        regions,
    })
}

pub(super) fn dac_banks(
    rom: &[u8],
    raw_files: &mut RawFiles,
    dac: &mut Dac,
) -> Result<(), PackError> {
    let mut first_entry = 0;
    for (bank, base, count) in DAC_BANKS {
        let table_path = format!("raw/dac/bank_{bank:02X}_table.bin");
        raw_files.add(
            table_path.clone(),
            range(rom, base, base + count * 8, "DAC bank table")?,
        )?;
        let mut samples = Vec::new();
        for local in 0..count {
            let sound_id = dac.bank_table_entries[first_entry + local].sound_id;
            let entry_offset = base + local * 8;
            let z80_start = usize::from(be16(rom, entry_offset)?.swap_bytes());
            let length = usize::from(be16(rom, entry_offset + 2)?.swap_bytes());
            if z80_start < 0x8000 || length == 0 {
                return Err(error(format!(
                    "DAC sample {} has invalid Z80 range",
                    hx(usize::from(sound_id), 2)
                )));
            }
            let sample_offset = base + (z80_start - 0x8000);
            let sample_end = sample_offset + length;
            if sample_end > base + 0x8000 {
                return Err(error(format!(
                    "DAC sample {} leaves bank {}",
                    hx(usize::from(sound_id), 2),
                    hx(usize::from(bank), 2)
                )));
            }
            let raw_path = format!("raw/dac/bank_{bank:02X}_sample_{local:02X}.bin");
            let raw = range(rom, sample_offset, sample_end, "DAC sample")?;
            raw_files.add(raw_path.clone(), raw)?;
            samples.push(DacSample {
                sound_id,
                bank,
                sample: local as u8,
                table_rom_offset: rom_offset(entry_offset),
                z80_start: hx(z80_start, 4),
                length_bytes: length as u32,
                rom_offset: rom_offset(sample_offset),
                raw_file: raw_path,
                raw_sha256: sha(raw),
            });
        }
        dac.banks.push(DacBank {
            bank,
            rom_base: rom_offset(base),
            entry_count: count as u32,
            table_raw_file: table_path,
            samples,
        });
        first_entry += count;
    }
    Ok(())
}
