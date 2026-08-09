//! Writing GDTF fixture types.
//!
//! A GDTF file is a zip whose `description.xml` describes a fixture and its DMX modes. This writes
//! one from a small model, so a product that already owns a channel table — a media server, a
//! generated fixture — can publish it to a console without anyone maintaining XML by hand.
//!
//! It is deliberately a *writer*, not a general GDTF implementation: what it emits is the subset a
//! console needs to patch a fixture and see its channels named correctly.

use std::io::Write as _;

/// How much of a value a channel carries, and therefore how many slots it occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// One slot.
    Byte,
    /// Two slots: a coarse byte and the fine byte immediately after it.
    Sixteen,
}

impl Width {
    pub const fn slots(self) -> u16 {
        match self {
            Self::Byte => 1,
            Self::Sixteen => 2,
        }
    }
}

/// One DMX channel of a mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// The operator-visible name, which is what a console's channel list shows.
    pub name: String,
    /// The GDTF attribute this channel drives. Free text: a media server's channels have no
    /// standard attribute, and inventing a wrong standard one is worse than a clear custom name.
    pub attribute: String,
    /// One-based offset of the coarse slot within the mode.
    pub offset: u16,
    pub width: Width,
    /// Complete raw value in this channel's own resolution.
    pub default: u32,
    /// Ordered raw ranges an operator selects. The end of one set is immediately before the next.
    pub sets: Vec<ChannelSet>,
}

/// One named range within a channel function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSet {
    pub name: String,
    /// Inclusive first raw value in the channel's own resolution.
    pub from: u32,
}

impl Channel {
    /// The offsets this channel occupies, one-based, coarse first.
    pub fn offsets(&self) -> Vec<u16> {
        match self.width {
            Width::Byte => vec![self.offset],
            Width::Sixteen => vec![self.offset, self.offset + 1],
        }
    }
}

/// One patchable mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode {
    pub name: String,
    pub channels: Vec<Channel>,
}

impl Mode {
    /// How many slots this mode occupies, which is what an operator patches.
    pub fn footprint(&self) -> u16 {
        self.channels
            .iter()
            .map(|channel| channel.offset + channel.width.slots() - 1)
            .max()
            .unwrap_or(0)
    }
}

/// A fixture type, as a console imports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureType {
    pub name: String,
    /// What a console shows where there is no room for the full name.
    pub short_name: String,
    pub manufacturer: String,
    pub description: String,
    /// Stable across releases: a console keys a patched fixture on it, so a new build must not
    /// change it or every existing patch becomes a different fixture.
    pub id: uuid::Uuid,
    pub modes: Vec<Mode>,
}

/// The geometry every channel of these fixtures belongs to.
///
/// A media server has no moving parts to model, so one body geometry is the honest description.
const GEOMETRY: &str = "Body";

/// Renders `description.xml`.
pub fn description_xml(fixture: &FixtureType) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<GDTF DataVersion=\"1.2\">\n");
    xml.push_str(&format!(
        "  <FixtureType Name=\"{}\" ShortName=\"{}\" LongName=\"{}\" Manufacturer=\"{}\" \
         Description=\"{}\" FixtureTypeID=\"{}\" RefFT=\"\">\n",
        escape(&fixture.name),
        escape(&fixture.short_name),
        escape(&fixture.name),
        escape(&fixture.manufacturer),
        escape(&fixture.description),
        fixture.id
    ));

    xml.push_str("    <AttributeDefinitions>\n      <ActivationGroups/>\n      <FeatureGroups>\n");
    xml.push_str(
        "        <FeatureGroup Name=\"Control\" Pretty=\"Control\">\n          <Feature Name=\"Control\"/>\n        </FeatureGroup>\n",
    );
    xml.push_str("      </FeatureGroups>\n      <Attributes>\n");
    for attribute in attributes(fixture) {
        xml.push_str(&format!(
            "        <Attribute Name=\"{}\" Pretty=\"{}\" Feature=\"Control.Control\"/>\n",
            escape(&attribute),
            escape(&attribute)
        ));
    }
    xml.push_str("      </Attributes>\n    </AttributeDefinitions>\n");

    xml.push_str("    <Wheels/>\n    <PhysicalDescriptions/>\n    <Models/>\n");
    xml.push_str(&format!(
        "    <Geometries>\n      <Geometry Name=\"{GEOMETRY}\" Position=\"{IDENTITY}\"/>\n    </Geometries>\n",
        IDENTITY = IDENTITY_MATRIX
    ));

    xml.push_str("    <DMXModes>\n");
    for mode in &fixture.modes {
        xml.push_str(&format!(
            "      <DMXMode Name=\"{}\" Geometry=\"{GEOMETRY}\">\n        <DMXChannels>\n",
            escape(&mode.name)
        ));
        for channel in &mode.channels {
            xml.push_str(&channel_xml(channel));
        }
        xml.push_str(
            "        </DMXChannels>\n        <Relations/>\n        <FTMacros/>\n      </DMXMode>\n",
        );
    }
    xml.push_str("    </DMXModes>\n    <Revisions/>\n    <FTPresets/>\n    <Protocols/>\n");
    xml.push_str("  </FixtureType>\n</GDTF>\n");
    xml
}

/// No transform: the fixture is a body with nothing to place relative to it.
const IDENTITY_MATRIX: &str = "{1.000000,0.000000,0.000000}{0.000000,1.000000,0.000000}{0.000000,0.000000,1.000000}{0.000000,0.000000,0.000000}";

fn channel_xml(channel: &Channel) -> String {
    let offsets = channel
        .offsets()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let resolution = match channel.width {
        Width::Byte => 1,
        Width::Sixteen => 2,
    };
    let default = format!("{}/{resolution}", channel.default);
    let mut xml = format!(
        "          <DMXChannel DMXBreak=\"1\" Offset=\"{offsets}\" Default=\"{default}\" \
         Highlight=\"None\" Geometry=\"{GEOMETRY}\">\n\
         \x20           <LogicalChannel Attribute=\"{attribute}\" Snap=\"No\" Master=\"None\" \
         MibFade=\"0.000000\" DMXChangeTimeLimit=\"0.000000\">\n\
         \x20             <ChannelFunction Name=\"{name}\" Attribute=\"{attribute}\" \
         OriginalAttribute=\"\" DMXFrom=\"0/1\" Default=\"{default}\" PhysicalFrom=\"0.000000\" \
         PhysicalTo=\"1.000000\" RealFade=\"0.000000\">\n",
        attribute = escape(&channel.attribute),
        name = escape(&channel.name),
    );
    for set in &channel.sets {
        xml.push_str(&format!(
            "              <ChannelSet Name=\"{}\" DMXFrom=\"{}/{resolution}\"/>\n",
            escape(&set.name),
            set.from,
        ));
    }
    xml.push_str(
        "            </ChannelFunction>\n          </LogicalChannel>\n          </DMXChannel>\n",
    );
    xml
}

/// Every attribute the fixture's channels name, once each, in the order they first appear.
fn attributes(fixture: &FixtureType) -> Vec<String> {
    let mut seen = Vec::new();
    for mode in &fixture.modes {
        for channel in &mode.channels {
            if !seen.contains(&channel.attribute) {
                seen.push(channel.attribute.clone());
            }
        }
    }
    seen
}

/// Packages a fixture type as a `.gdtf` archive.
pub fn package(fixture: &FixtureType) -> std::io::Result<Vec<u8>> {
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive.start_file(
        "description.xml",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    archive.write_all(description_xml(fixture).as_bytes())?;
    Ok(archive.finish()?.into_inner())
}

/// XML-escapes attribute text.
///
/// A fixture name an operator typed can contain anything; an unescaped ampersand would produce a
/// file a console refuses to import with no useful message.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    fn fixture() -> FixtureType {
        FixtureType {
            name: "Test Fixture".into(),
            short_name: "Test".into(),
            manufacturer: "ToskLight".into(),
            description: "A fixture for a test".into(),
            id: uuid::Uuid::from_u128(1),
            modes: vec![Mode {
                name: "Mode".into(),
                channels: vec![
                    Channel {
                        name: "Dimmer".into(),
                        attribute: "Dimmer".into(),
                        offset: 1,
                        width: Width::Byte,
                        default: 255,
                        sets: vec![],
                    },
                    Channel {
                        name: "Position".into(),
                        attribute: "Position".into(),
                        offset: 2,
                        width: Width::Sixteen,
                        default: 32_768,
                        sets: vec![],
                    },
                ],
            }],
        }
    }

    #[test]
    fn a_sixteen_bit_channel_occupies_both_of_its_slots() {
        let mode = &fixture().modes[0];
        assert_eq!(mode.channels[0].offsets(), vec![1]);
        assert_eq!(mode.channels[1].offsets(), vec![2, 3]);
        assert_eq!(mode.footprint(), 3, "the fine byte counts toward the patch");
    }

    #[test]
    fn the_description_names_every_attribute_once() {
        let mut fixture = fixture();
        fixture.modes[0].channels.push(Channel {
            name: "Second dimmer".into(),
            attribute: "Dimmer".into(),
            offset: 4,
            width: Width::Byte,
            default: 0,
            sets: vec![],
        });

        let xml = description_xml(&fixture);
        assert_eq!(
            xml.matches("<Attribute Name=\"Dimmer\"").count(),
            1,
            "two channels sharing an attribute declare it once"
        );
        assert!(xml.contains("<Attribute Name=\"Position\""));
    }

    #[test]
    fn a_channel_carries_its_offsets_and_a_default_in_its_own_resolution() {
        let xml = description_xml(&fixture());
        assert!(xml.contains("Offset=\"1\" Default=\"255/1\""), "{xml}");
        assert!(
            xml.contains("Offset=\"2,3\" Default=\"32768/2\""),
            "a 16-bit default is expressed across both bytes"
        );
    }

    #[test]
    fn channel_sets_are_nested_in_the_function_at_their_raw_boundaries() {
        let mut fixture = fixture();
        fixture.modes[0].channels[0].sets = vec![
            ChannelSet {
                name: "Closed".into(),
                from: 0,
            },
            ChannelSet {
                name: "Open & live".into(),
                from: 128,
            },
        ];
        let xml = description_xml(&fixture);
        assert!(xml.contains("<ChannelSet Name=\"Closed\" DMXFrom=\"0/1\"/>"));
        assert!(xml.contains("<ChannelSet Name=\"Open &amp; live\" DMXFrom=\"128/1\"/>"));
    }

    #[test]
    fn text_an_operator_typed_cannot_break_the_document() {
        let mut fixture = fixture();
        fixture.name = "Bars & \"Stripes\" <live>".into();
        let xml = description_xml(&fixture);

        assert!(xml.contains("Bars &amp; &quot;Stripes&quot; &lt;live&gt;"));
        assert!(
            !xml.contains("Bars & \""),
            "an unescaped ampersand makes a file a console silently refuses"
        );
    }

    #[test]
    fn the_package_is_an_archive_holding_the_description() {
        let bytes = package(&fixture()).expect("it packages");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a readable archive");

        let mut description = String::new();
        archive
            .by_name("description.xml")
            .expect("every GDTF holds one")
            .read_to_string(&mut description)
            .expect("it is text");

        assert!(description.starts_with("<?xml"));
        assert!(description.contains("DataVersion=\"1.2\""));
        assert!(description.contains("FixtureTypeID=\"00000000-0000-0000-0000-000000000001\""));
    }
}
