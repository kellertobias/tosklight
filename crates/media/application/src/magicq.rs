//! Native MagicQ HED writer. DMX slots and ranges come exclusively from the decoder table.
//!
//! HED v99 stores CSV records with hexadecimal integers, then XORs non-newline bytes with
//! the repeating sequence ff..81. Encoder addresses are I/P/C/B banks of 64, eight slots
//! per page in A,B,C,D,E,F,Y,X order. There is deliberately no layer-number input.
use media_domain::personality::channels::{
    ChannelSpec, LAYER_CHANNELS, MASTER_CHANNELS, Resolution,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Control {
    label: &'static str,
    attribute: u32,
    encoder: u32,
}

fn control(name: &str, master: bool) -> Control {
    let (label, attribute, encoder) = if !master {
        match name {
            // IPC B Split activates each Gobo/Rotate pair at its current output. Keep
            // the two libraries in separate pairs, with independent transport controls.
            "Folder" => ("Media Folder", 10, 0xc6),
            "File" => ("Media File", 8, 0xc7),
            "Play mode" => ("Play Mode", 58, 0xc5),
            "Speed multiplier" => ("Speed Multiplier", 59, 0xc4),
            "In point" => ("In Point", 12, 0xc0),
            "Out point" => ("Out Point", 14, 0xc1),
            "Blend mode" => ("Blend Mode", 15, 0xc2),
            "3D model" => ("3D Model", 7, 0xc3),
            "Scale Y" => ("Scale Y", 48, 0x42),
            "Scale X" => ("Scale X", 49, 0x43),
            "Rotation" => ("Rotation", 50, 0x44),
            "Scaling mode" => ("Scale Mode", 51, 0x45),
            "Position Y" => ("Position Y", 5, 0x46),
            "Position X" => ("Position X", 4, 0x47),
            "Model pan" => ("Model Pan", 2, 0x40),
            "Model tilt" => ("Model Tilt", 3, 0x41),
            "Dimmer" => ("Dimmer", 0, 7),
            "Volume" => ("Volume", 1, 6),
            "Cyan" => ("Cyan", 16, 0x84),
            "Magenta" => ("Magenta", 17, 0x85),
            "Yellow" => ("Yellow", 18, 0x86),
            // Generic Col1 stays available with CMY mixing. White (19) is an emitter
            // and MagicQ hides it when the fixture declares only three colour emitters.
            "Grayscale" => ("Greyscale", 6, 0x87),
            "Mask folder" => ("Mask Folder", 11, 0xe6),
            "Mask file" => ("Mask File", 9, 0xe7),
            "Mask position X" => ("Mask Position X", 52, 0xe0),
            "Mask position Y" => ("Mask Position Y", 53, 0xe1),
            "Mask scale X" => ("Mask Scale X", 54, 0xe2),
            "Mask scale Y" => ("Mask Scale Y", 55, 0xe3),
            "Mask invert" => ("Mask Invert", 56, 0xe4),
            "Mask opacity" => ("Mask Opacity", 57, 0xe5),
            "Effect 1 Select" => ("FX1 Select", 28, 0xcf),
            "Effect 1 Strength" => ("FX1 Mix", 29, 0xce),
            "Effect 2 Select" => ("FX2 Select", 36, 0xd7),
            "Effect 2 Strength" => ("FX2 Mix", 37, 0xd6),
            // Each bank's four parameters fill A–D of its own FX page, beside Select and Mix.
            "Effect 1 Parameter 1" => ("FX1 Param 1", 30, 0xc8),
            "Effect 1 Parameter 2" => ("FX1 Param 2", 31, 0xc9),
            "Effect 1 Parameter 3" => ("FX1 Param 3", 32, 0xca),
            "Effect 1 Parameter 4" => ("FX1 Param 4", 33, 0xcb),
            "Effect 2 Parameter 1" => ("FX2 Param 1", 38, 0xd0),
            "Effect 2 Parameter 2" => ("FX2 Param 2", 39, 0xd1),
            "Effect 2 Parameter 3" => ("FX2 Param 3", 40, 0xd2),
            "Effect 2 Parameter 4" => ("FX2 Param 4", 41, 0xd3),
            "Visualizer Parameter 1" => ("Vis Param 1", 20, 0xd8),
            "Visualizer Parameter 2" => ("Vis Param 2", 21, 0xd9),
            "Visualizer Parameter 3" => ("Vis Param 3", 22, 0xda),
            "Visualizer Parameter 4" => ("Vis Param 4", 23, 0xdb),
            _ => panic!("unmapped canonical layer control {name}"),
        }
    } else {
        match name {
            "Master dimmer" => ("Master Dimmer", 0, 7),
            "Master volume" => ("Master Volume", 1, 6),
            "Master cyan" => ("Master Cyan", 16, 0x84),
            "Master magenta" => ("Master Magenta", 17, 0x85),
            "Master yellow" => ("Master Yellow", 18, 0x86),
            "Master scale Y" => ("Scale Y", 48, 0x42),
            "Master scale X" => ("Scale X", 49, 0x43),
            "Master rotation" => ("Rotation", 50, 0x44),
            "Master scaling mode" => ("Scale Mode", 47, 0x45),
            "Master position Y" => ("Position Y", 5, 0x46),
            "Master position X" => ("Position X", 4, 0x47),
            "Master mask" => ("Master Mask", 59, 0xe7),
            "Master mask position X" => ("Mask Position X", 52, 0xe0),
            "Master mask position Y" => ("Mask Position Y", 53, 0xe1),
            "Shaper left" => ("Shaper Left", 40, 0xe8),
            "Shaper right" => ("Shaper Right", 41, 0xe9),
            "Shaper top" => ("Shaper Top", 42, 0xea),
            "Shaper bottom" => ("Shaper Bottom", 43, 0xeb),
            "Shaper left rotation" => ("Left Rotation", 44, 0xec),
            "Shaper right rotation" => ("Right Rotation", 45, 0xed),
            "Shaper top rotation" => ("Top Rotation", 46, 0xee),
            "Shaper bottom rotation" => ("Bottom Rotation", 54, 0xef),
            "Shaper rotation" => ("Shaper Rotation", 55, 0xf0),
            "Layer Opacity Cycle" => ("Layer Opacity Cycle", 28, 0xcf),
            _ => panic!("unmapped canonical master control {name}"),
        }
    };
    Control {
        label,
        attribute,
        encoder,
    }
}

fn source(table: &[ChannelSpec], spec: &ChannelSpec) -> ChannelSpec {
    if spec.resolution == Resolution::Fine {
        table[usize::from(spec.offset - 1)]
    } else {
        *spec
    }
}
fn raw_default(table: &[ChannelSpec], spec: &ChannelSpec) -> u16 {
    match spec.resolution {
        Resolution::Byte => spec.default_value,
        Resolution::Coarse => spec.default_value >> 8,
        Resolution::Fine => source(table, spec).default_value & 255,
    }
}
fn indexed(name: &str) -> bool {
    matches!(name, "Folder" | "File" | "Mask folder" | "Mask file")
}
fn ranges(table: &[ChannelSpec]) -> Vec<(u16, String, u16, u16, bool)> {
    let mut result = vec![];
    for spec in table
        .iter()
        .filter(|s| s.resolution != Resolution::Fine && s.implementation.is_implemented())
    {
        // Native ArKaos personalities mark library-managed selections with one Dynamic
        // range. Keep the full raw byte span and slow encoders, without static names
        // that compete with the CITP library.
        if indexed(spec.name) {
            result.push((spec.offset, "Dynamic".to_owned(), 0, 255, false));
            continue;
        }
        for set in spec.values.sets() {
            if set.step == 1 {
                // Expose each labelled speed interval as native Index (0x6000), as confirmed
                // by a head saved in MagicQ. This does not alter the canonical default.
                // HTTP speedMultiplierDmx is an enum representative, not a raw DMX echo.
                result.push((
                    spec.offset,
                    set.name,
                    set.from,
                    set.to,
                    spec.name == "Speed multiplier",
                ));
            } else {
                for value in (set.from..=set.to).step_by(usize::from(set.step)) {
                    result.push((spec.offset, set.name.clone(), value, value, false));
                }
            }
        }
    }
    result.sort_by_key(|range| (range.0, range.2));
    result
}
fn ascii(text: &str) -> String {
    text.replace('—', "-")
        .replace('×', "x")
        .replace('"', "'")
        .chars()
        .filter(char::is_ascii)
        .collect()
}

pub fn layer() -> Vec<u8> {
    layer_with_count(8)
}
/// Encode the media-server association for the active output. MagicQ creates its Media
/// entry while patching this head only when both the CITP type and layer count are present.
pub fn layer_with_count(layer_count: u16) -> Vec<u8> {
    assert!(matches!(layer_count, 2 | 8));
    encode(&description(LAYER_CHANNELS, false, layer_count))
}
pub fn master() -> Vec<u8> {
    encode(&description(MASTER_CHANNELS, true, 0))
}

fn description(table: &[ChannelSpec], master: bool, layer_count: u16) -> String {
    let name = if master { "Master" } else { "Layer" };
    let ranges = ranges(table);
    let highest_start = 513 - table.len();
    // Match the installed native MediaMaster layer's Pan/Tilt visualiser convention.
    // These metadata spans do not change the canonical raw 16-bit DMX controls, and
    // do not resolve MagicQ's remaining physical-parameter validation warning.
    let position_span = 65;
    let mut text = format!(
        "\\ Generated from the ToskLight Pixel canonical DMX decoder\nV,0099,\"MagicQ 1\";\nP,0005,\"ToskLight Pixel {name}\",\"ToskLight\",\"{name}\",\"Pixel {name}\",\n{:04x},{:04x},0000,0000,{position_span:04x},{position_span:04x},0001,0001,{highest_start:04x},00000000,\n",
        table.len(),
        ranges.len()
    );
    for spec in table {
        let src = source(table, spec);
        let c = control(src.name, master);
        let flags = if !spec.implementation.is_implemented() {
            0
        } else {
            let category = match c.encoder / 64 {
                0 => 0,
                2 => 0x20,
                _ => 0x10,
            };
            let width = match spec.resolution {
                Resolution::Byte => 0,
                Resolution::Coarse => 4,
                Resolution::Fine => 8,
            };
            let slow = if indexed(src.name) { 0x28000 } else { 0 };
            (if c.attribute == 0 { 1 } else { 2 }) | category | width | slow
        };
        let label = if spec.resolution == Resolution::Fine {
            format!("{} Fine", c.label)
        } else {
            c.label.to_owned()
        };
        text.push_str(&format!(
            "\"{}\",{flags:08x},{:08x},\n",
            ascii(&label),
            c.attribute
        ));
    }
    text.push_str(&format!("{:04x},", table.len()));
    for spec in table {
        let locate = if spec.name == "Dimmer" {
            255
        } else {
            raw_default(table, spec)
        };
        text.push_str(&format!("{:04x},{locate:04x},", spec.offset));
    }
    text.push('\n');
    for (slot, label, from, to, index) in ranges {
        let kind = if index { 0x6000 } else { 0 };
        text.push_str(&format!(
            "{slot:04x},\"{}\",{from:04x},{to:04x},{kind:04x},00000000,\n",
            ascii(&label)
        ));
    }
    text.push_str("00000000,\n");
    text.push_str(&"00000000,".repeat(table.len()));
    text.push_str("\n\"\",00000000,0000,0000,0000,0000,0000,\n");
    for spec in table {
        let c = control(source(table, spec).name, master);
        let default = 0x100 | raw_default(table, spec);
        let highlight = if c.attribute == 0 { 0x1ff } else { 0 };
        text.push_str(&format!(
            "{:08x},{default:04x},0000,{highlight:04x},\n",
            c.encoder
        ));
    }
    let native_metadata_marker = if master { 1 } else { 7 };
    let fixture_id = if master {
        crate::gdtf::master_fixture().id
    } else {
        crate::gdtf::layer_fixture().id
    };
    // The native Head Editor writes CITP MSEX as type 0005 and Num of layers into
    // this footer. Head number remains zero so MagicQ uses the first patched head.
    let media_type = if master { 0 } else { 5 };
    text.push_str(&format!("\"ToskLight Pixel {name}\",\n0000,\"\",\"\",\"\",0000,{media_type:04x},0000,0000,{layer_count:04x},00000000,\n{native_metadata_marker:08x},\n\"\",\"\",\"\",\n0.000000,0.000000,0.000000,0.000000,0.000000,0.000000,\n\"\",0000,\n0,0,0,0,0,0,0,0,0.000000,\n0,0,0,0,0,0,0,0,0,0,0,\n0,0,0,\n0,\n"));
    text.push_str("00000000,00000000,");
    text.push_str(&"0000,".repeat(table.len() + 1));
    text.push_str(&format!("\n0000,0000,0000,00000007,00000000,0,\n\"Pixel {name}\",\n0,\n\"{{{fixture_id}}}\",\n1.000000,0000,0000,\n0000,\n0.800000,0.800000,0.000000,0.000000,\n0.000000,0.000000,0.000000,0.000000,0,0,\n0,\n;\n\n"));
    text
}
fn encode(text: &str) -> Vec<u8> {
    let mut key = 255;
    text.bytes()
        .map(|byte| {
            if byte == b'\n' {
                return byte;
            }
            let encoded = byte ^ key;
            key = if key == 129 { 255 } else { key - 1 };
            encoded
        })
        .collect()
}

pub fn channels_csv(master: bool) -> String {
    let table = if master {
        MASTER_CHANNELS
    } else {
        LAYER_CHANNELS
    };
    table
        .iter()
        .map(|spec| {
            let c = control(source(table, spec).name, master);
            let bank = ["I", "P", "C", "B"][((c.encoder / 64).min(3)) as usize];
            let encoder = if c.encoder == 255 {
                String::new()
            } else {
                format!(
                    "{bank}{}{}",
                    (c.encoder % 64) / 8 + 1,
                    ["A", "B", "C", "D", "E", "F", "Y", "X"][(c.encoder % 8) as usize]
                )
            };
            let size = match spec.resolution {
                Resolution::Byte => "8 bit",
                Resolution::Coarse => "16 bit hi",
                Resolution::Fine => "16 bit lo",
            };
            let raw = raw_default(table, spec);
            let kind = if c.attribute == 0 { "HTP" } else { "LTP" };
            format!(
                "{},{},{kind},{},{encoder},{size},no,yes,no,{raw},{raw},0,,0,no,yes,1\n",
                spec.offset + 1,
                c.label,
                c.attribute
            )
        })
        .collect()
}
pub fn ranges_csv(master: bool) -> String {
    ranges(if master {
        MASTER_CHANNELS
    } else {
        LAYER_CHANNELS
    })
    .iter()
    .map(|(slot, label, from, to, index)| {
        let kind = if *index { 0x6000 } else { 0 };
        format!("{},{},{from},{to},{kind},0,,\n", slot + 1, ascii(label))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Decode independently from the writer: the header and records must remain native HED,
    // not an opaque pinned binary or CSV renamed to .hed.
    fn decode(bytes: &[u8]) -> String {
        let mut index = 0;
        String::from_utf8(
            bytes
                .iter()
                .map(|byte| {
                    if *byte == b'\n' {
                        *byte
                    } else {
                        let value = byte ^ (255 - (index % 127) as u8);
                        index += 1;
                        value
                    }
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn native_records_preserve_every_slot_resolution_and_default() {
        for (table, bytes) in [(LAYER_CHANNELS, layer()), (MASTER_CHANNELS, master())] {
            let text = decode(&bytes);
            let lines: Vec<_> = text.lines().collect();
            assert_eq!(lines[1], "V,0099,\"MagicQ 1\";");
            let header: Vec<_> = lines[3].split(',').collect();
            assert_eq!(usize::from_str_radix(header[0], 16).unwrap(), table.len());
            assert_eq!(
                usize::from_str_radix(header[8], 16).unwrap(),
                513 - table.len(),
                "native patch must fit the final DMX slot"
            );
            assert!(u16::from_str_radix(header[4], 16).unwrap() > 0);
            assert!(u16::from_str_radix(header[5], 16).unwrap() > 0);
            assert_eq!(
                usize::from_str_radix(header[1], 16).unwrap(),
                ranges(table).len()
            );
            let encoder_start = 4 + table.len() + 1 + ranges(table).len() + 3;
            for spec in table {
                let channel: Vec<_> = lines[4 + usize::from(spec.offset)].split(',').collect();
                let flags = u32::from_str_radix(channel[1], 16).unwrap();
                assert_eq!(
                    flags & 12,
                    match spec.resolution {
                        Resolution::Byte => 0,
                        Resolution::Coarse => 4,
                        Resolution::Fine => 8,
                    },
                    "{} width",
                    spec.name
                );
                let encoder: Vec<_> = lines[encoder_start + usize::from(spec.offset)]
                    .split(',')
                    .collect();
                assert_eq!(
                    u16::from_str_radix(encoder[1], 16).unwrap(),
                    256 | raw_default(table, spec),
                    "{} default",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn native_footer_arrays_follow_the_actual_footprint() {
        for (table, bytes, name) in [
            (LAYER_CHANNELS, layer(), "Layer"),
            (MASTER_CHANNELS, master(), "Master"),
        ] {
            let text = decode(&bytes);
            let lines: Vec<_> = text.lines().collect();
            let footer = lines
                .iter()
                .position(|line| *line == format!("\"ToskLight Pixel {name}\","))
                .unwrap();
            // Native v99 contains two fixed flags, then one slot word per channel and a
            // terminator word. A 39-slot constant here shifted the 41-slot Master's name
            // into the UUID field on import, despite a valid-looking channel section.
            assert_eq!(
                lines[footer + 10]
                    .split(',')
                    .filter(|part| !part.is_empty())
                    .count(),
                table.len() + 3
            );
            assert_eq!(lines[footer + 12], format!("\"Pixel {name}\","));
            assert!(lines[footer + 14].starts_with("\"{746f736b-6c69-6768-745f-"));
        }
    }

    #[test]
    fn layer_footer_registers_citp_media_server_with_the_patched_layer_count() {
        for count in [2, 8] {
            let text = decode(&layer_with_count(count));
            let lines: Vec<_> = text.lines().collect();
            let footer = lines
                .iter()
                .position(|line| *line == "\"ToskLight Pixel Layer\",")
                .unwrap();
            assert_eq!(
                lines[footer + 1],
                format!("0000,\"\",\"\",\"\",0000,0005,0000,0000,{count:04x},00000000,")
            );
        }
        let master = decode(&master());
        assert!(master.contains("0000,\"\",\"\",\"\",0000,0000,0000,0000,0000,00000000,"));
    }

    #[test]
    fn every_layer_uses_one_profile_with_exact_encoder_pages_and_unassigned_slots() {
        let expected = [
            (0xc0, "In Point"),
            (0xc1, "Out Point"),
            (0xc2, "Blend Mode"),
            (0xc3, "3D Model"),
            (0xc4, "Speed Multiplier"),
            (0xc5, "Play Mode"),
            (0xc6, "Media Folder"),
            (0xc7, "Media File"),
            (0x84, "Cyan"),
            (0x85, "Magenta"),
            (0x86, "Yellow"),
            (0x87, "Greyscale"),
            (0x42, "Scale Y"),
            (0x43, "Scale X"),
            (0x44, "Rotation"),
            (0x45, "Scale Mode"),
            (0x46, "Position Y"),
            (0x47, "Position X"),
            (0x40, "Model Pan"),
            (0x41, "Model Tilt"),
            (6, "Volume"),
            (7, "Dimmer"),
            (0xcf, "FX1 Select"),
            (0xce, "FX1 Mix"),
            (0xc8, "FX1 Param 1"),
            (0xc9, "FX1 Param 2"),
            (0xca, "FX1 Param 3"),
            (0xcb, "FX1 Param 4"),
            (0xd7, "FX2 Select"),
            (0xd6, "FX2 Mix"),
            (0xd0, "FX2 Param 1"),
            (0xd1, "FX2 Param 2"),
            (0xd2, "FX2 Param 3"),
            (0xd3, "FX2 Param 4"),
            (0xd8, "Vis Param 1"),
            (0xd9, "Vis Param 2"),
            (0xda, "Vis Param 3"),
            (0xdb, "Vis Param 4"),
            (0xe0, "Mask Position X"),
            (0xe1, "Mask Position Y"),
            (0xe2, "Mask Scale X"),
            (0xe3, "Mask Scale Y"),
            (0xe4, "Mask Invert"),
            (0xe5, "Mask Opacity"),
            (0xe6, "Mask Folder"),
            (0xe7, "Mask File"),
        ];
        for _layer in 1..=8 {
            let assigned: Vec<_> = LAYER_CHANNELS
                .iter()
                .filter(|s| s.resolution != Resolution::Fine && s.implementation.is_implemented())
                .map(|s| control(s.name, false))
                .collect();
            assert_eq!(assigned.len(), expected.len());
            for (encoder, label) in expected {
                assert_eq!(
                    assigned
                        .iter()
                        .filter(|c| c.encoder == encoder)
                        .map(|c| c.label)
                        .collect::<Vec<_>>(),
                    vec![label]
                );
            }
            for empty in [
                0x80, 0x81, 0x82, 0x83, 0, 1, 2, 3, 4, 5, 0xcc, 0xcd, 0xd4, 0xd5,
            ] {
                assert!(!assigned.iter().any(|c| c.encoder == empty));
            }
        }
        let text = decode(&layer());
        assert!(!text.contains("Blur"));
        // This proves the personality has no placeholder control, not that MagicQ's own UI
        // suppresses its built-in grey Reserved caption for an unassigned wheel.
        assert!(!text.contains("Reserved"));
    }

    #[test]
    fn all_four_selectors_are_raw_bytes_with_native_dynamic_library_ranges() {
        let text = decode(&layer());
        let lines: Vec<_> = text.lines().collect();
        let range_rows = ranges(LAYER_CHANNELS);
        for spec in LAYER_CHANNELS.iter().filter(|s| indexed(s.name)) {
            assert_eq!(spec.resolution, Resolution::Byte);
            let channel = lines[4 + usize::from(spec.offset)];
            assert!(
                channel.contains(",00028012,"),
                "{} must retain the native slow byte encoder flags",
                spec.name
            );
            let selector_ranges: Vec<_> = range_rows
                .iter()
                .filter(|range| range.0 == spec.offset)
                .collect();
            assert_eq!(
                selector_ranges,
                vec![&(spec.offset, "Dynamic".to_owned(), 0, 255, false)]
            );
            assert_eq!(spec.default_value, 0);
        }
    }

    #[test]
    fn ipc_b_split_has_only_the_two_library_pairs_and_keeps_separate_encoders() {
        let text = decode(&layer());
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(LAYER_CHANNELS.len(), 59);
        for (name, attribute, encoder) in [
            ("File", 8, 0xc7),
            ("Folder", 10, 0xc6),
            ("Mask file", 9, 0xe7),
            ("Mask folder", 11, 0xe6),
            ("Play mode", 58, 0xc5),
            ("Speed multiplier", 59, 0xc4),
        ] {
            let c = control(name, false);
            assert_eq!((c.attribute, c.encoder), (attribute, encoder), "{name}");
            let spec = LAYER_CHANNELS
                .iter()
                .find(|s| s.name == name)
                .unwrap();
            let row: Vec<_> = lines[4 + usize::from(spec.offset)].split(',').collect();
            assert_eq!(
                u32::from_str_radix(row[2], 16).unwrap(),
                attribute,
                "{name} HED"
            );
        }
        for attribute in [8, 10, 9, 11] {
            let assigned: Vec<_> = LAYER_CHANNELS
                .iter()
                .filter(|s| control(source(LAYER_CHANNELS, s).name, false).attribute == attribute)
                .collect();
            assert_eq!(
                assigned.len(),
                1,
                "library attribute {attribute} must not activate unrelated channels"
            );
            assert!(indexed(assigned[0].name));
        }
    }

    #[test]
    fn grayscale_uses_independent_colour_attribute_and_speed_defaults_are_exact() {
        let gray = control("Grayscale", false);
        assert_eq!(gray.attribute, 6);
        assert_eq!(gray.encoder, 0x87);
        let speed = LAYER_CHANNELS
            .iter()
            .find(|s| s.name == "Speed multiplier")
            .unwrap();
        assert_eq!(raw_default(LAYER_CHANNELS, speed), 127);
        let speed_ranges: Vec<_> = ranges(LAYER_CHANNELS)
            .into_iter()
            .filter(|r| r.0 == speed.offset)
            .collect();
        assert!(!speed_ranges.is_empty());
        assert!(speed_ranges.iter().all(|r| r.4));
        let text = decode(&layer());
        // Exact record from MagicQ 1.9.8.3 after changing /16 to Index in View Ranges
        // and saving pixel-native-index-probe.hed during live interoperability testing.
        assert!(text.contains("0020,\"/16\",0000,0007,6000,00000000,"));
        assert!(
            text.contains("0020,007f,"),
            "Locate must preserve canonical byte 127"
        );
        assert!(
            text.contains("000000c4,017f,0000,0000,"),
            "Default must preserve canonical byte 127"
        );
    }

    #[test]
    fn native_ranges_are_in_ascending_non_overlapping_slot_order() {
        for table in [LAYER_CHANNELS, MASTER_CHANNELS] {
            let rows = ranges(table);
            for pair in rows.windows(2) {
                let (first, next) = (&pair[0], &pair[1]);
                assert!(
                    first.0 < next.0 || (first.0 == next.0 && first.3 < next.2),
                    "MagicQ rejects out-of-order Range Vals: {first:?} then {next:?}"
                );
            }
        }
    }

    #[test]
    fn master_contains_fixed_effect_and_does_not_shift_layer_block() {
        assert_eq!(LAYER_CHANNELS.len() * 8 + MASTER_CHANNELS.len(), 512);
        assert_eq!(MASTER_CHANNELS.len(), 40);
        let text = decode(&master());
        assert!(text.contains("\"Layer Opacity Cycle\""));
        assert!(
            !text.contains("Flip Mirror"),
            "mirroring is a negative scale"
        );
        for set in MASTER_CHANNELS[39].values.sets() {
            assert!(text.contains(&format!(
                "0027,\"{}\",{:04x},{:04x},",
                set.name, set.from, set.to
            )));
        }
    }
}
