//! Writing GDTF fixture types.
//!
//! A GDTF file is a zip whose `description.xml` describes a fixture and its DMX modes. This writes
//! one from a small model, so a product that already owns a channel table — a media server, a
//! generated fixture — can publish it to a console without anyone maintaining XML by hand.
//!
//! It is deliberately a *writer*, not a general GDTF implementation: what it emits is the subset a
//! console needs to patch a fixture and see its channels named correctly.

use std::io::Write as _;

pub mod profile;

/// How much of a value a channel carries, and therefore how many slots it occupies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Width {
    /// One slot.
    #[default]
    Byte,
    /// Two slots: a coarse byte and the fine byte immediately after it.
    Sixteen,
    /// Three slots, coarse to finest.
    TwentyFour,
    /// Four slots, coarse to finest.
    ThirtyTwo,
}

impl Width {
    pub const fn slots(self) -> u16 {
        match self {
            Self::Byte => 1,
            Self::Sixteen => 2,
            Self::TwentyFour => 3,
            Self::ThirtyTwo => 4,
        }
    }

    /// The largest raw value a channel of this width carries.
    pub const fn max_raw(self) -> u32 {
        match self {
            Self::Byte => 0xff,
            Self::Sixteen => 0xffff,
            Self::TwentyFour => 0x00ff_ffff,
            Self::ThirtyTwo => u32::MAX,
        }
    }
}

/// One DMX channel of a mode.
#[derive(Debug, Clone, PartialEq)]
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
    /// One-based slots of the finer bytes, coarse to fine, when they do not simply follow
    /// `offset`. Empty means consecutive.
    pub fine_offsets: Vec<u16>,
    /// One-based DMX break — the independently patched address block — `offset` belongs to.
    pub dmx_break: u16,
    /// The geometry this channel controls; `None` is the fixture body.
    pub geometry: Option<String>,
    /// `FeatureGroup.Feature` of the attribute; `None` leaves the choice to the writer.
    pub feature: Option<String>,
    /// Raw value, in this channel's own resolution, that Highlight sends.
    pub highlight: Option<u32>,
    /// Physical values at the bottom and top of the range; `None` is 0 to 1.
    pub physical: Option<(f32, f32)>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            name: String::new(),
            attribute: String::new(),
            offset: 1,
            width: Width::Byte,
            default: 0,
            sets: Vec::new(),
            fine_offsets: Vec::new(),
            dmx_break: 1,
            geometry: None,
            feature: None,
            highlight: None,
            physical: None,
        }
    }
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
        let mut offsets = vec![self.offset];
        if self.fine_offsets.len() + 1 == usize::from(self.width.slots()) {
            offsets.extend(&self.fine_offsets);
        } else {
            offsets.extend((1..self.width.slots()).map(|index| self.offset + index));
        }
        offsets
    }
}

/// One patchable mode.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mode {
    pub name: String,
    pub channels: Vec<Channel>,
}

impl Mode {
    /// How many slots this mode occupies, which is what an operator patches.
    pub fn footprint(&self) -> u16 {
        self.channels
            .iter()
            .flat_map(Channel::offsets)
            .max()
            .unwrap_or(0)
    }
}

/// A fixture type, as a console imports it.
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// Body length, width and height in metres. `None` writes no model, which is right for a
    /// product that has no physical body, such as a media server.
    pub body_size: Option<[f32; 3]>,
    /// Light-emitting geometries below the body, one per independently controlled head.
    pub beams: Vec<String>,
}

/// The geometry every channel of these fixtures belongs to unless it names a beam.
///
/// A media server has no moving parts to model, so one body geometry is the honest description.
const GEOMETRY: &str = "Body";

/// The model every beam geometry draws with.
const BEAM_MODEL: &str = "Beam";

const IDENTITY: &str = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";

/// Renders `description.xml`.
pub fn description_xml(fixture: &FixtureType) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<GDTF DataVersion=\"1.2\">\n");
    xml.push_str(&format!(
        "  <FixtureType Name=\"{}\" ShortName=\"{}\" LongName=\"{}\" Manufacturer=\"{}\" \
         Description=\"{}\" FixtureTypeID=\"{}\" RefFT=\"\">\n",
        escape(&gdtf_name(&fixture.name)),
        escape(&fixture.short_name),
        escape(&fixture.name),
        escape(&fixture.manufacturer),
        escape(&fixture.description),
        fixture.id
    ));
    push_attribute_definitions(&mut xml, fixture);
    xml.push_str("    <Wheels/>\n    <PhysicalDescriptions/>\n");
    push_models(&mut xml, fixture);
    push_geometries(&mut xml, fixture);

    xml.push_str("    <DMXModes>\n");
    for mode in &fixture.modes {
        xml.push_str(&format!(
            "      <DMXMode Name=\"{}\" Geometry=\"{GEOMETRY}\">\n        <DMXChannels>\n",
            escape(&gdtf_name(&mode.name))
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

fn push_attribute_definitions(xml: &mut String, fixture: &FixtureType) {
    let attributes = attributes(fixture);
    // Control is always declared: every attribute without a standard feature belongs to it.
    let mut groups: Vec<(&str, Vec<&str>)> = vec![("Control", vec!["Control"])];
    for (_, feature) in &attributes {
        let (group, name) = feature
            .split_once('.')
            .unwrap_or((feature.as_str(), feature.as_str()));
        match groups.iter_mut().find(|(known, _)| *known == group) {
            Some((_, features)) if !features.contains(&name) => features.push(name),
            Some(_) => {}
            None => groups.push((group, vec![name])),
        }
    }
    xml.push_str("    <AttributeDefinitions>\n      <ActivationGroups/>\n      <FeatureGroups>\n");
    for (group, features) in &groups {
        let group = escape(group);
        xml.push_str(&format!(
            "        <FeatureGroup Name=\"{group}\" Pretty=\"{group}\">\n"
        ));
        for feature in features {
            xml.push_str(&format!(
                "          <Feature Name=\"{}\"/>\n",
                escape(feature)
            ));
        }
        xml.push_str("        </FeatureGroup>\n");
    }
    xml.push_str("      </FeatureGroups>\n      <Attributes>\n");
    for (attribute, feature) in &attributes {
        let name = escape(attribute);
        xml.push_str(&format!(
            "        <Attribute Name=\"{name}\" Pretty=\"{name}\" Feature=\"{}\"/>\n",
            escape(feature)
        ));
    }
    xml.push_str("      </Attributes>\n    </AttributeDefinitions>\n");
}

fn push_models(xml: &mut String, fixture: &FixtureType) {
    let Some([length, width, height]) = fixture.body_size else {
        xml.push_str("    <Models/>\n");
        return;
    };
    xml.push_str("    <Models>\n");
    xml.push_str(&format!(
        "      <Model Name=\"{GEOMETRY}\" Length=\"{length:.6}\" Width=\"{width:.6}\" \
         Height=\"{height:.6}\" PrimitiveType=\"Cube\"/>\n"
    ));
    if !fixture.beams.is_empty() {
        let diameter = (length / fixture.beams.len() as f32).min(width) * 0.8;
        let depth = (height * 0.1).max(0.01);
        xml.push_str(&format!(
            "      <Model Name=\"{BEAM_MODEL}\" Length=\"{diameter:.6}\" Width=\"{diameter:.6}\" \
             Height=\"{depth:.6}\" PrimitiveType=\"Cylinder\"/>\n"
        ));
    }
    xml.push_str("    </Models>\n");
}

fn push_geometries(xml: &mut String, fixture: &FixtureType) {
    let modelled = fixture.body_size.is_some();
    let body_model = if modelled {
        format!(" Model=\"{GEOMETRY}\"")
    } else {
        String::new()
    };
    xml.push_str("    <Geometries>\n");
    if fixture.beams.is_empty() {
        xml.push_str(&format!(
            "      <Geometry Name=\"{GEOMETRY}\"{body_model} Position=\"{IDENTITY}\"/>\n"
        ));
        xml.push_str("    </Geometries>\n");
        return;
    }
    xml.push_str(&format!(
        "      <Geometry Name=\"{GEOMETRY}\"{body_model} Position=\"{IDENTITY}\">\n"
    ));
    let [length, _, height] = fixture.body_size.unwrap_or_default();
    let beam_model = if modelled {
        format!(" Model=\"{BEAM_MODEL}\"")
    } else {
        String::new()
    };
    let count = fixture.beams.len() as f32;
    for (index, beam) in fixture.beams.iter().enumerate() {
        // Side by side along the body and level with its underside; a beam emits along -Z.
        let x = length * ((index as f32 + 0.5) / count - 0.5);
        let z = -height / 2.0;
        xml.push_str(&format!(
            "        <Beam Name=\"{}\"{beam_model} \
             Position=\"{{1,0,0,{x:.6}}}{{0,1,0,0}}{{0,0,1,{z:.6}}}{{0,0,0,1}}\" \
             BeamType=\"Wash\"/>\n",
            escape(&gdtf_name(beam))
        ));
    }
    xml.push_str("      </Geometry>\n    </Geometries>\n");
}

fn channel_xml(channel: &Channel) -> String {
    let offsets = channel
        .offsets()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let resolution = channel.width.slots();
    let default = format!("{}/{resolution}", channel.default);
    let highlight = channel
        .highlight
        .map_or_else(|| "None".to_owned(), |raw| format!("{raw}/{resolution}"));
    let geometry = channel
        .geometry
        .as_deref()
        .map_or_else(|| GEOMETRY.to_owned(), |name| escape(&gdtf_name(name)));
    let (physical_from, physical_to) = channel.physical.unwrap_or((0.0, 1.0));
    let mut xml = format!(
        "          <DMXChannel DMXBreak=\"{dmx_break}\" Offset=\"{offsets}\" \
         Highlight=\"{highlight}\" Geometry=\"{geometry}\">\n\
         \x20           <LogicalChannel Attribute=\"{attribute}\" Snap=\"No\" Master=\"None\" \
         MibFade=\"0.000000\" DMXChangeTimeLimit=\"0.000000\">\n\
         \x20             <ChannelFunction Name=\"{name}\" Attribute=\"{attribute}\" \
         OriginalAttribute=\"\" DMXFrom=\"0/1\" Default=\"{default}\" \
         PhysicalFrom=\"{physical_from:.6}\" PhysicalTo=\"{physical_to:.6}\" \
         RealFade=\"0.000000\">\n",
        dmx_break = channel.dmx_break.max(1),
        attribute = escape(&gdtf_name(&channel.attribute)),
        name = escape(&gdtf_name(&channel.name)),
    );
    for set in &channel.sets {
        xml.push_str(&format!(
            "              <ChannelSet Name=\"{}\" DMXFrom=\"{}/{resolution}\"/>\n",
            escape(&gdtf_name(&set.name)),
            set.from,
        ));
    }
    xml.push_str(
        "            </ChannelFunction>\n          </LogicalChannel>\n          </DMXChannel>\n",
    );
    xml
}

/// Every attribute the fixture's channels name, once each in the order they first appear, with
/// the feature it belongs to.
fn attributes(fixture: &FixtureType) -> Vec<(String, String)> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for channel in fixture.modes.iter().flat_map(|mode| &mode.channels) {
        let name = gdtf_name(&channel.attribute);
        if !seen.iter().any(|(known, _)| *known == name) {
            let feature = channel
                .feature
                .clone()
                .unwrap_or_else(|| default_feature(&name).to_owned());
            seen.push((name, feature));
        }
    }
    seen
}

/// Indexed gobos use the standard feature MagicQ maps as media wheels; everything else a caller
/// did not classify is a control.
fn default_feature(attribute: &str) -> &'static str {
    if matches!(attribute, "Gobo1" | "Gobo2") {
        "Gobo.Gobo"
    } else {
        "Control.Control"
    }
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

/// GDTF `Name` values deliberately use a small ASCII character set. Keep authored labels readable
/// while ensuring one typographic dash or multiplication sign cannot make a console discard the
/// complete fixture type.
pub fn gdtf_name(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '—' | '–' => '-',
            '×' => 'x',
            allowed
                if allowed.is_ascii_alphanumeric() || "#%()*+-/:;<=>@_` \"'".contains(allowed) =>
            {
                allowed
            }
            _ => '_',
        })
        .collect()
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
                        ..Default::default()
                    },
                    Channel {
                        name: "Position".into(),
                        attribute: "Position".into(),
                        offset: 2,
                        width: Width::Sixteen,
                        default: 32_768,
                        ..Default::default()
                    },
                ],
            }],
            ..Default::default()
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
    fn explicit_fine_slots_and_breaks_are_written_as_the_profile_places_them() {
        let mut fixture = fixture();
        fixture.modes[0].channels[1] = Channel {
            name: "Pan".into(),
            attribute: "Pan".into(),
            offset: 2,
            width: Width::TwentyFour,
            fine_offsets: vec![7, 9],
            dmx_break: 2,
            highlight: Some(0x80_0000),
            physical: Some((-270.0, 270.0)),
            ..Default::default()
        };
        assert_eq!(fixture.modes[0].channels[1].offsets(), vec![2, 7, 9]);
        assert_eq!(fixture.modes[0].footprint(), 9);

        let xml = description_xml(&fixture);
        assert!(
            xml.contains("DMXBreak=\"2\" Offset=\"2,7,9\" Highlight=\"8388608/3\""),
            "{xml}"
        );
        assert!(xml.contains("PhysicalFrom=\"-270.000000\" PhysicalTo=\"270.000000\""));
    }

    #[test]
    fn beams_hang_below_a_modelled_body_and_channels_can_address_them() {
        let mut fixture = fixture();
        fixture.body_size = Some([0.4, 0.2, 0.3]);
        fixture.beams = vec!["Cell 1".into(), "Cell 2".into()];
        fixture.modes[0].channels[0].geometry = Some("Cell 2".into());
        fixture.modes[0].channels[0].feature = Some("Dimmer.Dimmer".into());

        let xml = description_xml(&fixture);
        assert!(
            xml.contains("<Model Name=\"Body\" Length=\"0.400000\""),
            "{xml}"
        );
        assert!(xml.contains("PrimitiveType=\"Cylinder\""));
        assert!(xml.contains("<Geometry Name=\"Body\" Model=\"Body\""));
        assert!(xml.contains("<Beam Name=\"Cell 1\" Model=\"Beam\""));
        assert!(xml.contains("Geometry=\"Cell 2\">"));
        assert!(xml.contains("<FeatureGroup Name=\"Dimmer\" Pretty=\"Dimmer\">"));
        assert!(
            xml.contains(
                "<Attribute Name=\"Dimmer\" Pretty=\"Dimmer\" Feature=\"Dimmer.Dimmer\"/>"
            )
        );
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
            ..Default::default()
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
    fn indexed_gobos_use_the_standard_gdtf_feature_magicq_maps_as_media_wheels() {
        let mut fixture = fixture();
        fixture.modes[0].channels[0].attribute = "Gobo2".into();
        fixture.modes[0].channels[1].attribute = "Gobo1".into();

        let xml = description_xml(&fixture);
        assert!(xml.contains("<FeatureGroup Name=\"Gobo\" Pretty=\"Gobo\">"));
        assert!(xml.contains("<Attribute Name=\"Gobo1\" Pretty=\"Gobo1\" Feature=\"Gobo.Gobo\"/>"));
        assert!(xml.contains("<Attribute Name=\"Gobo2\" Pretty=\"Gobo2\" Feature=\"Gobo.Gobo\"/>"));
    }

    #[test]
    fn a_channel_carries_its_offsets_and_a_default_in_its_own_resolution() {
        let xml = description_xml(&fixture());
        assert!(xml.contains("Offset=\"1\" Highlight=\"None\""), "{xml}");
        assert!(!xml.contains("<DMXChannel DMXBreak=\"1\" Offset=\"1\" Default="));
        assert!(xml.contains("<ChannelFunction Name=\"Dimmer\""));
        assert!(xml.contains("Default=\"255/1\""), "{xml}");
        assert!(
            xml.contains("Offset=\"2,3\" Highlight=\"None\""),
            "a 16-bit default is expressed across both bytes"
        );
        assert!(xml.contains("Default=\"32768/2\""));
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
        assert!(xml.contains("<ChannelSet Name=\"Open _ live\" DMXFrom=\"128/1\"/>"));
    }

    #[test]
    fn text_an_operator_typed_cannot_break_the_document() {
        let mut fixture = fixture();
        fixture.name = "Bars & \"Stripes\" <live>".into();
        let xml = description_xml(&fixture);

        assert!(xml.contains("LongName=\"Bars &amp; &quot;Stripes&quot; &lt;live&gt;\""));
        assert!(
            !xml.contains("Bars & \""),
            "an unescaped ampersand makes a file a console silently refuses"
        );
    }

    #[test]
    fn console_names_are_restricted_to_the_gdtf_name_character_set() {
        let mut fixture = fixture();
        fixture.name = "Spot — 2× café".into();
        fixture.modes[0].name = "16 bit – extended".into();
        fixture.modes[0].channels[0].name = "Once — 2× café".into();
        fixture.modes[0].channels[0].sets = vec![ChannelSet {
            name: "1–255 BPM".into(),
            from: 0,
        }];

        let xml = description_xml(&fixture);
        assert!(
            xml.contains("<FixtureType Name=\"Spot - 2x caf_\""),
            "{xml}"
        );
        assert!(xml.contains("<DMXMode Name=\"16 bit - extended\""), "{xml}");
        assert!(xml.contains("Name=\"Once - 2x caf_\""), "{xml}");
        assert!(xml.contains("Name=\"1-255 BPM\""), "{xml}");
    }

    #[test]
    fn generated_xml_resolves_attributes_geometry_and_feature_references() {
        use quick_xml::{Reader, events::Event};
        use std::collections::HashSet;
        let mut fixture = fixture();
        fixture.modes[0].channels[1].attribute = "Flip&mirror".into();
        fixture.beams = vec!["Beam".into()];
        fixture.modes[0].channels[1].geometry = Some("Beam".into());
        let xml = description_xml(&fixture);
        let mut reader = Reader::from_str(&xml);
        let mut attributes = HashSet::new();
        let mut features = HashSet::new();
        let mut geometries = HashSet::new();
        let mut group = String::new();
        let mut references = Vec::new();
        loop {
            match reader.read_event().expect("well-formed XML") {
                Event::Start(element) | Event::Empty(element) => {
                    let fields = element
                        .attributes()
                        .map(|attr| {
                            let attr = attr.unwrap();
                            (
                                String::from_utf8(attr.key.as_ref().to_vec()).unwrap(),
                                attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .unwrap()
                                    .into_owned(),
                            )
                        })
                        .collect::<std::collections::HashMap<_, _>>();
                    match element.name().as_ref() {
                        b"FeatureGroup" => group = fields["Name"].clone(),
                        b"Feature" => {
                            features.insert(format!("{group}.{}", fields["Name"]));
                        }
                        b"Attribute" => {
                            assert!(attributes.insert(fields["Name"].clone()));
                            references.push(("feature", fields["Feature"].clone()));
                        }
                        b"Geometry" | b"Beam" => {
                            geometries.insert(fields["Name"].clone());
                            assert!(fields["Position"].starts_with("{1,0,0,"));
                        }
                        b"LogicalChannel" | b"ChannelFunction" => {
                            references.push(("attribute", fields["Attribute"].clone()));
                        }
                        b"DMXMode" | b"DMXChannel" => {
                            references.push(("geometry", fields["Geometry"].clone()));
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        for (kind, value) in references {
            let known = match kind {
                "attribute" => &attributes,
                "feature" => &features,
                _ => &geometries,
            };
            assert!(known.contains(&value), "unresolved {kind}: {value}");
        }
        assert!(attributes.contains("Flip_mirror"));
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
