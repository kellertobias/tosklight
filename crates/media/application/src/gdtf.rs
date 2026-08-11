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

/// The layer fixture: the 34 slots one media layer occupies.
pub fn layer_fixture() -> FixtureType {
    FixtureType {
        name: "ToskLight Media Layer".into(),
        short_name: "TL Media".into(),
        manufacturer: MANUFACTURER.into(),
        description: "One media layer of a ToskLight Media Server. Patch one per layer; the \
                      master fixture follows the layers."
            .into(),
        id: LAYER_ID,
        modes: vec![Mode {
            name: "Layer".into(),
            channels: channels(LAYER_CHANNELS),
        }],
    }
}

/// The master fixture: the 7 slots that begin immediately after the controlled layers.
pub fn master_fixture() -> FixtureType {
    FixtureType {
        name: "ToskLight Media Master".into(),
        short_name: "TL Master".into(),
        manufacturer: MANUFACTURER.into(),
        description: "The output section of a ToskLight Media Server, which applies to the \
                      finished composite. Patch one, immediately after the layers."
            .into(),
        id: MASTER_ID,
        modes: vec![Mode {
            name: "Master".into(),
            channels: channels(MASTER_CHANNELS),
        }],
    }
}

/// Both fixtures, as `.gdtf` archives ready to hand a console.
pub fn packages() -> std::io::Result<Vec<(String, Vec<u8>)>> {
    Ok(vec![
        (
            "ToskLight Media Layer.gdtf".into(),
            package(&layer_fixture())?,
        ),
        (
            "ToskLight Media Master.gdtf".into(),
            package(&master_fixture())?,
        ),
    ])
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
            name: spec.name.to_owned(),
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
/// A media server's channels have no standard GDTF attribute, so these are the channel's own name
/// without spaces. A wrong standard attribute would make a console apply the wrong semantics —
/// fading a folder number through a crossfade, for instance.
fn attribute_of(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().concat()
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
        for expected in ["Effect 1", "Effect 4", "Playback BPM", "Mask opacity"] {
            assert!(names.contains(&expected), "{expected} is missing");
        }
    }

    #[test]
    fn attributes_carry_no_spaces_and_no_standard_meaning_is_borrowed() {
        assert_eq!(attribute_of("Play mode"), "Playmode");
        assert_eq!(attribute_of("Mask scale X"), "MaskscaleX");

        // Nothing may claim a standard GDTF attribute a console would apply its own semantics to.
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

        assert_eq!(by_name("Folder"), 0, "nothing is selected");
        assert_eq!(by_name("File"), 0);
        assert_eq!(by_name("Dimmer"), 255, "but a selection appears at once");
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
        assert_eq!(packaged.len(), 2);
        for (name, bytes) in packaged {
            assert!(name.ends_with(".gdtf"), "{name}");
            assert!(bytes.len() > 100, "{name} is suspiciously small");
        }
        assert!(layer_description().contains("ToskLight Media Layer"));
    }

    #[test]
    fn normal_fixture_library_packages_match_the_canonical_media_wire() {
        for (filename, id, specs) in [
            (
                "tosklight--media-server-layer.toskfixture",
                LAYER_ID,
                LAYER_CHANNELS,
            ),
            (
                "tosklight--media-server-master.toskfixture",
                MASTER_ID,
                MASTER_CHANNELS,
            ),
        ] {
            let profile = shipped_tosklight_profile(filename);
            assert_eq!(profile.id.0, id);
            assert_eq!(profile.manufacturer, "ToskLight");
            let mode = &profile.modes[0];
            assert_eq!(usize::from(mode.splits[0].footprint), specs.len());

            for spec in specs {
                if spec.resolution == Resolution::Fine {
                    continue;
                }
                let channel = mode
                    .channels
                    .iter()
                    .find(|channel| channel.functions[0].name == spec.name)
                    .unwrap_or_else(|| panic!("{filename} is missing {}", spec.name));
                assert_eq!(channel.default_raw, u32::from(spec.default_value));
                assert_eq!(
                    channel.secondary_slots,
                    if spec.resolution == Resolution::Coarse {
                        vec![spec.offset + 2]
                    } else {
                        Vec::new()
                    },
                    "{} slot ownership",
                    spec.name
                );
            }
        }
    }
}
