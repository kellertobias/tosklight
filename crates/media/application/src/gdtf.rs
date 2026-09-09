//! The GDTF fixtures a console imports to patch this server.
//!
//! Both are generated from the canonical personality, never written out by hand: the channel table
//! in `media_domain::personality` is the single source the receivers, the API, the tests, and this
//! all read, so a channel cannot exist on the wire and be missing from a console's patch.
//!
//! Two fixtures, matching how a media server is patched: one layer, which an operator patches once
//! per layer, and one master. That is why the domain names a single-layer and a master-only
//! footprint.

use light_fixture::gdtf::{
    Channel, ChannelSet, FixtureType, Mode, Width, description_xml, package,
};
use media_domain::personality::channels::{LAYER_CHANNELS, MASTER_CHANNELS, Resolution};

/// Stable identifiers. A console keys a patched fixture on these, so a new build must never change
/// them or every existing patch becomes a different fixture.
const LAYER_ID: uuid::Uuid = uuid::Uuid::from_u128(0x746f_736b_6c69_6768_745f_6d65_6469_615f);
const MASTER_ID: uuid::Uuid = uuid::Uuid::from_u128(0x746f_736b_6c69_6768_745f_6d61_7374_6572);

const MANUFACTURER: &str = "ToskLight";

/// The layer fixture: the 39 slots one media layer occupies.
pub fn layer_fixture() -> FixtureType {
    FixtureType {
        name: "ToskLight Pixel Layer".into(),
        short_name: "TL Pixel".into(),
        manufacturer: MANUFACTURER.into(),
        description: "One media layer of ToskLight Pixel. Patch one per layer; the \
                      master fixture follows the layers."
            .into(),
        id: LAYER_ID,
        modes: vec![Mode {
            name: "Layer".into(),
            channels: channels(LAYER_CHANNELS),
        }],
    }
}

/// The complete 40-slot master fixture that begins immediately after the controlled layers.
pub fn master_fixture() -> FixtureType {
    FixtureType {
        name: "ToskLight Pixel Master".into(),
        short_name: "TL Master".into(),
        manufacturer: MANUFACTURER.into(),
        description: "The output section of ToskLight Pixel, which applies to the \
                      finished composite. Patch one, immediately after the layers."
            .into(),
        id: MASTER_ID,
        modes: vec![Mode {
            name: "Master".into(),
            channels: channels(MASTER_CHANNELS),
        }],
    }
}

/// Native console personalities plus the two GDTF fixtures, all generated or captured from the
/// same canonical channel table.
pub fn packages() -> std::io::Result<Vec<(String, Vec<u8>)>> {
    let layer = layer_fixture();
    let master = master_fixture();
    Ok(vec![
        ("ToskLight Pixel Layer.gdtf".into(), package(&layer)?),
        ("ToskLight Pixel Master.gdtf".into(), package(&master)?),
        (
            "ToskLight Pixel Layer.hed".into(),
            include_bytes!(
                "../../../../assets/media-personalities/magicq/ToskLight Pixel Layer.hed"
            )
            .to_vec(),
        ),
        (
            "ToskLight Pixel Master.hed".into(),
            include_bytes!(
                "../../../../assets/media-personalities/magicq/ToskLight Pixel Master.hed"
            )
            .to_vec(),
        ),
        (
            "tosklight@pixel_layer@39ch.xml".into(),
            grandma2_xml(&layer).into_bytes(),
        ),
        (
            "tosklight@pixel_master@41ch.xml".into(),
            grandma2_xml(&master).into_bytes(),
        ),
    ])
}

/// grandMA2's native fixture-library XML. The two media selectors use MA's dedicated media
/// attributes; the remaining server-specific controls keep their operator names as custom control
/// channels while retaining the exact coarse/fine offsets and defaults.
fn grandma2_xml(fixture: &FixtureType) -> String {
    let mode = &fixture.modes[0];
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <MA xmlns=\"http://schemas.malighting.de/grandma2/xml/MA\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xsi:schemaLocation=\"http://schemas.malighting.de/grandma2/xml/MA \
         http://schemas.malighting.de/grandma2/xml/3.9.60/MA.xsd\" \
         major_vers=\"3\" minor_vers=\"9\" stream_vers=\"60\">\n\
         \x20 <FixtureType index=\"0\" name=\"{}\" mode=\"{}\">\n\
         \x20   <short_name>{}</short_name>\n\
         \x20   <manufacturer>{}</manufacturer>\n\
         \x20   <short_manufacturer>{}</short_manufacturer>\n\
         \x20   <Modules index=\"0\">\n\
         \x20     <Module index=\"0\" name=\"Main Module\" class=\"None\" beamtype=\"None\">\n",
        xml_escape(&fixture.name),
        xml_escape(&mode.name),
        xml_escape(&fixture.short_name),
        xml_escape(&fixture.manufacturer),
        xml_escape(&fixture.manufacturer),
    );
    for (index, channel) in mode.channels.iter().enumerate() {
        xml.push_str(&grandma2_channel_xml(index, channel));
    }
    xml.push_str(
        "      </Module>\n\
         \x20   </Modules>\n\
         \x20   <Instances index=\"1\"><Instance index=\"0\" module_index=\"0\"/></Instances>\n\
         \x20   <Wheels index=\"2\"/>\n\
         \x20   <VirtualFunctionBlocks index=\"3\"/>\n\
         \x20   <AutoPresets index=\"4\"/>\n\
         \x20   <FixtureMacroCollect index=\"5\"/>\n\
         \x20   <RdmNotifications index=\"6\"/>\n\
         \x20 </FixtureType>\n\
         </MA>\n",
    );
    xml
}

fn grandma2_channel_xml(index: usize, channel: &Channel) -> String {
    let (attribute, feature, preset, subattribute) = match channel.name.as_str() {
        "Media Library" => (
            "MEDIASERVERINPUTDIRECTORY",
            "MEDIA",
            "GOBO",
            "MEDIASERVERINPUTDIRECTORYSELECT",
        ),
        "Media Visual" => (
            "MEDIASERVERINPUT",
            "MEDIA",
            "GOBO",
            "MEDIASERVERINPUTFILESELECT",
        ),
        "Dimmer" => ("DIM", "DIMMER", "DIMMER", "DIM"),
        _ => ("DUMMY", "CONTROL", "CONTROL", "NOFEATURE"),
    };
    let fine = match channel.width {
        Width::Byte => String::new(),
        Width::Sixteen => format!(" fine=\"{}\"", channel.offset + 1),
    };
    let max = match channel.width {
        Width::Byte => 255,
        Width::Sixteen => 65_535,
    };
    let mut xml = format!(
        "        <ChannelType index=\"{index}\" attribute=\"{attribute}\" feature=\"{feature}\" \
         preset=\"{preset}\" coarse=\"{}\"{fine} default=\"{}\">\n\
         \x20         <ChannelFunction index=\"0\" from=\"0\" to=\"100\" min_dmx_24=\"0\" \
         max_dmx_24=\"16777215\" physfrom=\"0\" physto=\"{max}\" \
         subattribute=\"{subattribute}\" subattribute_user_name=\"{}\" attribute=\"{attribute}\" \
         attribute_user_name=\"{}\" feature=\"{feature}\" feature_user_name=\"{}\" \
         preset=\"{preset}\" preset_user_name=\"{}\">\n",
        channel.offset,
        channel.default,
        xml_escape(&channel.name),
        xml_escape(&channel.name),
        if feature == "MEDIA" {
            "Media"
        } else {
            "Control"
        },
        if preset == "GOBO" { "Gobo" } else { "Control" },
    );
    for (set_index, set) in channel.sets.iter().enumerate() {
        let to = channel
            .sets
            .get(set_index + 1)
            .map(|next| next.from.saturating_sub(1))
            .unwrap_or(max);
        xml.push_str(&format!(
            "            <ChannelSet index=\"{set_index}\" name=\"{}\" from_dmx=\"{}\" to_dmx=\"{to}\"/>\n",
            xml_escape(&set.name),
            set.from,
        ));
    }
    xml.push_str("          </ChannelFunction>\n        </ChannelType>\n");
    xml
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn layer_description() -> String {
    description_xml(&layer_fixture())
}

pub fn master_description() -> String {
    description_xml(&master_fixture())
}

/// Turns the canonical table into GDTF channels.
///
/// A fine byte is not a channel of its own: it belongs to the coarse channel before it, which is
/// what makes a console show one 16-bit control rather than two unrelated bytes.
fn channels(table: &[media_domain::personality::channels::ChannelSpec]) -> Vec<Channel> {
    table
        .iter()
        .filter(|spec| spec.resolution != Resolution::Fine)
        .map(|spec| Channel {
            name: console_name_of(spec.name).to_owned(),
            attribute: attribute_of(spec.name),
            // GDTF offsets are one-based; the table's are zero-based.
            offset: spec.offset + 1,
            width: match spec.resolution {
                Resolution::Coarse => Width::Sixteen,
                Resolution::Byte | Resolution::Fine => Width::Byte,
            },
            default: u32::from(spec.default_value),
            sets: channel_sets(spec),
        })
        .collect()
}

/// Projects the canonical decoder ranges into the ordered GDTF boundaries a console reads.
///
/// A stepped range such as flip/mirror's modulo-four mapping cannot be represented as one GDTF
/// interval, so each matching byte becomes a one-byte set. Sorting all starts together preserves
/// the decoder exactly rather than turning four interleaved values into four false blocks.
fn channel_sets(spec: &media_domain::personality::channels::ChannelSpec) -> Vec<ChannelSet> {
    if matches!(spec.name, "Folder" | "File") {
        return (0_u16..=255)
            .map(|value| ChannelSet {
                name: format!("{} {value:03}", spec.name),
                from: u32::from(value),
            })
            .collect();
    }

    let mut projected = spec
        .values
        .sets()
        .into_iter()
        .flat_map(|set| {
            let starts: Vec<u16> = if set.step == 1 {
                vec![set.from]
            } else {
                (set.from..=set.to).step_by(usize::from(set.step)).collect()
            };
            starts.into_iter().map(move |from| ChannelSet {
                name: set.name.clone(),
                from: u32::from(from),
            })
        })
        .collect::<Vec<_>>();
    projected.sort_by_key(|set| set.from);
    projected
}

/// The GDTF attribute name for a channel.
///
/// Folder and file deliberately use the indexed-wheel attributes understood by MagicQ's Media
/// window. Other controls retain descriptive custom attributes instead of borrowing unrelated
/// fixture semantics.
fn attribute_of(name: &str) -> String {
    match name {
        "Folder" => "Gobo2".into(),
        "File" => "Gobo1".into(),
        _ => name.split_whitespace().collect::<Vec<_>>().concat(),
    }
}

/// Names MagicQ recognises when associating the two media selector wheels with CITP thumbnails.
fn console_name_of(name: &str) -> &str {
    match name {
        "Folder" => "Media Library",
        "File" => "Media Visual",
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::personality::SlotFootprint;
    use std::path::Path;

    fn shipped_tosklight_profile(filename: &str) -> light_fixture::FixtureProfile {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("assets/fixture-library")
            .join(filename);
        light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn each_fixture_patches_exactly_the_slots_the_personality_says() {
        assert_eq!(
            layer_fixture().modes[0].footprint(),
            SlotFootprint::SINGLE_LAYER.total(),
            "a layer fixture must occupy one layer's slots exactly"
        );
        assert_eq!(
            master_fixture().modes[0].footprint(),
            SlotFootprint::MASTER_ONLY.total()
        );
    }

    #[test]
    fn a_fine_byte_belongs_to_its_coarse_channel_rather_than_standing_alone() {
        let mode = &layer_fixture().modes[0];
        assert!(
            !mode
                .channels
                .iter()
                .any(|channel| channel.name.contains("fine")),
            "a console must see one 16-bit control, not two bytes"
        );

        let scale = mode
            .channels
            .iter()
            .find(|channel| channel.name == "Scale X")
            .expect("the table has one");
        assert_eq!(scale.width, Width::Sixteen);
        assert_eq!(scale.offsets(), vec![4, 5], "one-based, coarse then fine");
    }

    #[test]
    fn every_channel_in_the_table_reaches_the_fixture() {
        let coarse_or_byte = LAYER_CHANNELS
            .iter()
            .filter(|spec| spec.resolution != Resolution::Fine)
            .count();
        assert_eq!(layer_fixture().modes[0].channels.len(), coarse_or_byte);

        // Including the ones that are declared and not yet implemented: a console patches the
        // whole footprint, and a hole in it would shift every channel after it.
        let fixture = layer_fixture();
        let names: Vec<&str> = fixture.modes[0]
            .channels
            .iter()
            .map(|channel| channel.name.as_str())
            .collect();
        for expected in [
            "Effect 1 Select",
            "Effect 2 Strength",
            "Playback BPM",
            "Mask opacity",
        ] {
            assert!(names.contains(&expected), "{expected} is missing");
        }
    }

    #[test]
    fn media_selectors_use_console_media_semantics_and_other_attributes_stay_descriptive() {
        assert_eq!(attribute_of("Play mode"), "Playmode");
        assert_eq!(attribute_of("Mask scale X"), "MaskscaleX");
        assert_eq!(attribute_of("Folder"), "Gobo2");
        assert_eq!(attribute_of("File"), "Gobo1");

        let layer = layer_fixture();
        let folder = &layer.modes[0].channels[0];
        let file = &layer.modes[0].channels[1];
        assert_eq!(folder.name, "Media Library");
        assert_eq!(file.name, "Media Visual");
        assert_eq!(folder.sets.len(), 256);
        assert_eq!(folder.sets[7].name, "Folder 007");
        assert_eq!(file.sets.len(), 256);
        assert_eq!(file.sets[255].name, "File 255");

        for channel in &layer_fixture().modes[0].channels {
            assert_ne!(channel.attribute, "Pan");
            assert_ne!(channel.attribute, "Tilt");
        }
    }

    #[test]
    fn a_freshly_patched_fixture_does_nothing_visible() {
        let mode = &layer_fixture().modes[0];
        let by_name = |name: &str| {
            mode.channels
                .iter()
                .find(|channel| channel.name == name)
                .expect("the table has it")
                .default
        };

        assert_eq!(by_name("Media Library"), 0, "nothing is selected");
        assert_eq!(by_name("Media Visual"), 0);
        assert_eq!(by_name("Dimmer"), 0, "a fresh layer remains transparent");
        assert_eq!(by_name("Scale X"), 32_768, "at its neutral scale");
        assert_eq!(by_name("Mask opacity"), 0, "and unmasked");
        assert_eq!(by_name("Cyan"), 0, "colour defaults match the decoder");
        assert_eq!(by_name("Magenta"), 0);
        assert_eq!(by_name("Yellow"), 0);
    }

    #[test]
    fn canonical_value_sets_reach_the_fixture_without_restatement() {
        let layer = layer_fixture();
        let play = layer.modes[0]
            .channels
            .iter()
            .find(|channel| channel.name == "Play mode")
            .unwrap();
        assert_eq!(play.sets[0].name, "Loop");
        assert_eq!(play.sets[0].from, 0);
        assert!(play.sets.iter().any(|set| set.name == "Once — Transparent"));

        let master = master_fixture();
        let flip = master.modes[0]
            .channels
            .iter()
            .find(|channel| channel.name == "Flip/mirror")
            .unwrap();
        assert_eq!(flip.sets.len(), 256, "every modulo-four byte stays exact");
        assert_eq!(flip.sets[0].name, "None");
        assert_eq!(flip.sets[1].name, "Horizontal");
        assert_eq!(flip.sets[2].name, "Vertical");
        assert_eq!(flip.sets[3].name, "Both");
    }

    #[test]
    fn the_two_fixtures_are_not_the_same_fixture() {
        assert_ne!(layer_fixture().id, master_fixture().id);
        assert_ne!(layer_fixture().name, master_fixture().name);
    }

    #[test]
    fn both_package_as_archives_a_console_can_import() {
        let packaged = packages().expect("they package");
        assert_eq!(packaged.len(), 6);
        for (name, bytes) in packaged.iter().filter(|(name, _)| name.ends_with(".gdtf")) {
            assert!(bytes.len() > 100, "{name} is suspiciously small");
        }
        assert!(layer_description().contains("ToskLight Pixel Layer"));
    }

    #[test]
    fn grandma2_personalities_preserve_media_selectors_offsets_and_fine_bytes() {
        let xml = grandma2_xml(&layer_fixture());
        assert!(xml.contains("name=\"ToskLight Pixel Layer\" mode=\"Layer\""));
        assert!(xml.contains("attribute=\"MEDIASERVERINPUTDIRECTORY\""));
        assert!(xml.contains("attribute=\"MEDIASERVERINPUT\""));
        assert!(xml.contains("coarse=\"4\" fine=\"5\" default=\"32768\""));
        assert!(xml.contains("name=\"Folder 007\" from_dmx=\"7\" to_dmx=\"7\""));
        assert!(xml.ends_with("</MA>\n"));
    }

    #[test]
    fn normal_fixture_library_package_matches_both_canonical_media_personalities() {
        let profile = shipped_tosklight_profile("tosklight--media-server.toskfixture");
        assert_eq!(profile.manufacturer, "ToskLight");
        assert_eq!(profile.name, "Media Server");
        assert_eq!(profile.modes.len(), 2);

        for (mode, layer_count) in profile.modes.iter().zip([2_u16, 8]) {
            let expected_footprint = layer_count * SlotFootprint::SINGLE_LAYER.total()
                + SlotFootprint::MASTER_ONLY.total();
            assert_eq!(
                mode.splits,
                vec![light_fixture::FixtureSplit {
                    number: 1,
                    footprint: expected_footprint,
                }]
            );
            assert_eq!(mode.heads.len(), usize::from(layer_count + 1));
            assert_eq!(
                mode.heads.iter().filter(|head| head.master_shared).count(),
                1
            );
            assert_eq!(mode.heads[0].name, "Master");

            let primary_slots = mode.primary_slots().unwrap();
            for layer in 0..layer_count {
                let head = &mode.heads[usize::from(layer + 1)];
                assert_eq!(head.name, format!("Layer {}", layer + 1));
                assert!(!head.master_shared);
                assert_wire_block(
                    mode,
                    head.id,
                    LAYER_CHANNELS,
                    layer * SlotFootprint::SINGLE_LAYER.total(),
                    &primary_slots,
                );
            }
            assert_wire_block(
                mode,
                mode.heads[0].id,
                MASTER_CHANNELS,
                layer_count * SlotFootprint::SINGLE_LAYER.total(),
                &primary_slots,
            );
        }
    }

    fn assert_wire_block(
        mode: &light_fixture::FixtureMode,
        head_id: uuid::Uuid,
        specs: &[media_domain::personality::channels::ChannelSpec],
        block_offset: u16,
        primary_slots: &std::collections::HashMap<uuid::Uuid, u16>,
    ) {
        for spec in specs {
            if spec.resolution == Resolution::Fine {
                continue;
            }
            // A wire block is defined by where its controls sit, so the spec is matched to the
            // channel occupying its slot. Matching by label instead would tie this test to how a
            // channel happens to be named, which is not what it is checking.
            let slot = block_offset + spec.offset + 1;
            let channel = mode
                .channels
                .iter()
                .find(|channel| {
                    channel.head_id == head_id && primary_slots.get(&channel.id) == Some(&slot)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{} has no channel at slot {slot} for {}",
                        mode.name, spec.name
                    )
                });
            assert_eq!(channel.split, 1);
            assert_eq!(channel.default_raw, u32::from(spec.default_value));
            // One function labels its channel. A channel that names its value bands instead —
            // Play mode names all twenty, so an operator reads Stop rather than a percentage — is
            // labelled by those, and the fixture-package tests cover those names.
            if let [only] = channel.functions.as_slice() {
                assert_eq!(only.name, spec.name, "{} label", mode.name);
            }
            assert_eq!(
                channel.secondary_slots,
                if spec.resolution == Resolution::Coarse {
                    vec![block_offset + spec.offset + 2]
                } else {
                    Vec::new()
                },
                "{} / {} slot ownership",
                mode.name,
                spec.name
            );
        }
    }
}
