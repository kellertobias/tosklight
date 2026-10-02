use super::*;
use crate::{ChannelBehavior, ChannelFunctionBehavior, ChannelResolution, CompiledPhysicalMapping};
use std::io::Write;

const ID: &str = "684af0b8-5e84-4e28-a8a2-687647b2b515";

fn fixture(channels: &str) -> String {
    format!(
        r#"<GDTF DataVersion="1.2"><FixtureType Name="Contract" Manufacturer="Test" FixtureTypeID="{ID}"><AttributeDefinitions><Attributes><Attribute Name="Pan" PhysicalUnit="Angle"/><Attribute Name="Zoom" PhysicalUnit="Angle"/><Attribute Name="Color1"/><Attribute Name="Dimmer"/><Attribute Name="Focus1" PhysicalUnit="Percent"/><Attribute Name="NoFeature"/></Attributes></AttributeDefinitions><DMXModes><DMXMode Name="Exact"><DMXChannels>{channels}</DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"#
    )
}

fn channel(offset: &str, default: &str) -> String {
    format!(
        r#"<DMXChannel Offset="{offset}" Geometry="Head" Highlight="255/1"><LogicalChannel Attribute="Pan"><ChannelFunction Name="Position" Attribute="Pan" DMXFrom="0/1" Default="{default}" PhysicalFrom="720" PhysicalTo="-720"/></LogicalChannel></DMXChannel>"#
    )
}

#[test]
fn imports_all_native_widths_and_directed_function_ranges() {
    for (offsets, default, width) in [
        ("1", "253/1", ChannelResolution::U8),
        ("1,3", "32769/2", ChannelResolution::U16),
        ("4,2,1", "1193046/3", ChannelResolution::U24),
        ("2,4,1,3", "4294967294/4", ChannelResolution::U32),
    ] {
        let imported = from_xml(&fixture(&channel(offsets, default))).unwrap();
        let mode = &imported.profile.modes[0];
        let channel = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0.as_ref() == "pan")
            .unwrap();
        assert_eq!(channel.resolution, width);
        assert_eq!(
            channel.default_raw,
            default.split('/').next().unwrap().parse::<u32>().unwrap()
        );
        assert_eq!(channel.highlight_raw, width.max_raw());
        let mapping = CompiledPhysicalMapping::compile(channel, &channel.functions[0])
            .unwrap()
            .unwrap();
        assert_eq!(mapping.physical_for_raw(0).physical, 720.0);
        assert_eq!(mapping.physical_for_raw(width.max_raw()).physical, -720.0);
        let mut frame = [0; 512];
        mode.encode_channel(&mut frame, 1, channel, channel.default_raw)
            .unwrap();
        for (index, slot) in offsets
            .split(',')
            .map(|s| s.parse::<usize>().unwrap())
            .enumerate()
        {
            assert_eq!(
                frame[slot - 1],
                ((channel.default_raw >> (8 * (width.bytes() - index - 1))) & 255) as u8
            );
        }
    }
}

#[test]
fn independent_breaks_and_sparse_slots_do_not_move_channels() {
    let first = channel("3,1", "255/1s");
    let second = channel("1,2", "255/1")
        .replace("<DMXChannel ", "<DMXChannel DMXBreak=\"2\" ")
        .replace("Head", "Other");
    let imported = from_xml(&fixture(&format!("{first}{second}"))).unwrap();
    let mode = &imported.profile.modes[0];
    assert_eq!(
        mode.splits
            .iter()
            .map(|split| (split.number, split.footprint))
            .collect::<Vec<_>>(),
        vec![(1, 3), (2, 2)]
    );
    let primary = mode.primary_slots().unwrap();
    let pan = mode
        .channels
        .iter()
        .filter(|channel| channel.attribute.0.as_ref() == "pan")
        .collect::<Vec<_>>();
    assert_eq!(primary[&pan[0].id], 3);
    assert_eq!(pan[0].default_raw, 65280);
    assert_eq!(primary[&pan[1].id], 1);
    assert_eq!(pan[1].default_raw, 65535);
    assert!(mode.channels.iter().any(|channel| channel.behavior == ChannelBehavior::Static && primary[&channel.id] == 2));
}

#[test]
fn function_boundaries_initial_reference_and_default_remain_full_width() {
    let xml = fixture(
        r#"<DMXChannel Geometry="Head" Offset="1,2" InitialFunction="Head_Zoom.Zoom.Narrow" Highlight="65534/2"><LogicalChannel Attribute="Zoom"><ChannelFunction Name="Wide" Attribute="Zoom" DMXFrom="0/1" Default="10/2" PhysicalFrom="60" PhysicalTo="30"/><ChannelFunction Name="Narrow" Attribute="Zoom" DMXFrom="32768/2" Default="40000/2" PhysicalFrom="30" PhysicalTo="5"/></LogicalChannel></DMXChannel>"#,
    );
    let imported = from_xml(&xml).unwrap();
    let channel = &imported.profile.modes[0].channels[0];
    assert_eq!(channel.default_raw, 40000);
    assert_eq!(channel.highlight_raw, 65534);
    assert_eq!(
        channel
            .functions
            .iter()
            .map(|function| (function.dmx_from, function.dmx_to))
            .collect::<Vec<_>>(),
        [(0, 32767), (32768, 65535)]
    );
    assert!(matches!(
        channel.functions[1].behavior,
        ChannelFunctionBehavior::Continuous {
            physical_min: 30.0,
            physical_max: 5.0,
            ..
        }
    ));
}

#[test]
fn sets_use_next_start_and_function_end_without_eight_bit_clamping() {
    let xml = fixture(
        r#"<DMXChannel Geometry="Head" Offset="1,2"><LogicalChannel Attribute="Color1"><ChannelFunction Name="Wheel" Attribute="Color1" DMXFrom="0/1" Default="0/1"><ChannelSet Name="Open" DMXFrom="0/1"/><ChannelSet Name="Red" DMXFrom="32768/2"/></ChannelFunction></LogicalChannel></DMXChannel>"#,
    );
    let result = from_xml(&xml).unwrap();
    let functions = &result.profile.modes[0].channels[0].functions;
    assert_eq!((functions[0].dmx_from, functions[0].dmx_to), (0, 32767));
    assert_eq!((functions[1].dmx_from, functions[1].dmx_to), (32768, 65535));
}

#[test]
fn identity_survives_reimport_and_profile_serialization() {
    let xml = fixture(&channel("1,2", "1/2"));
    let a = from_xml(&xml).unwrap().profile;
    let b = from_xml(&xml).unwrap().profile;
    assert_eq!(a.id.0, Uuid::parse_str(ID).unwrap());
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    let restored: FixtureProfile =
        serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    assert_eq!(
        restored.modes[0].channels[0].functions[0].id,
        a.modes[0].channels[0].functions[0].id
    );
}

#[test]
fn unsupported_or_invalid_relationships_fail_instead_of_flattening() {
    for xml in [
        fixture(&channel("1,2", "32768/1")),
        fixture(&channel("1,1", "0/1")),
        fixture(
            &channel("1", "0/1").replace("PhysicalFrom=", "DMXProfile=\"Curve\" PhysicalFrom="),
        ),
        fixture(
            &channel("1", "0/1").replace("PhysicalFrom=", "ModeMaster=\"Other\" PhysicalFrom="),
        ),
        fixture(&channel("1", "0/1").replace(
            "<DMXChannel ",
            "<DMXChannel InitialFunction=\"Missing.Function\" ",
        )),
    ] {
        assert!(from_xml(&xml).is_err(), "{xml}");
    }
}

#[test]
fn constant_physical_output_is_not_changed_to_a_fake_range() {
    let result = from_xml(&fixture(
        &channel("1", "0/1").replace("PhysicalTo=\"-720\"", "PhysicalTo=\"720\""),
    ))
    .unwrap();
    let function = &result.profile.modes[0].channels[0].functions[0];
    assert!(matches!(
        function.behavior,
        ChannelFunctionBehavior::Fixed { .. }
    ));
    assert!(function.physical_mapping.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|warning| warning.message.contains("Constant physical output"))
    );
}

#[test]
fn archive_entry_and_xml_structure_are_validated() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("description.xml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(fixture(&channel("1", "0/1")).as_bytes())
        .unwrap();
    assert!(import_profile(&zip.finish().unwrap().into_inner()).is_ok());
    assert!(from_xml("<!DOCTYPE GDTF><GDTF DataVersion=\"1.2\"/>").is_err());
    assert!(from_xml("<GDTF><FixtureType></GDTF>").is_err());
    assert!(from_xml("<GDTF/><GDTF/>").is_err());
}

#[test]
fn generated_gdtf_keeps_continuous_position_and_zoom_operable() {
    for source_attribute in ["Pan", "Zoom"] {
        let original = from_xml(&fixture(
            &channel("1,2", "32769/2").replace("Pan", source_attribute),
        ))
        .unwrap()
        .profile;
        let exported = crate::gdtf::profile::package_profile(&original).unwrap();
        let imported = import_profile(&exported).unwrap().profile;
        let mode = &imported.modes[0];
        let channel = &mode.channels[0];
        let function = &channel.functions[0];
        assert!(matches!(
            function.behavior,
            ChannelFunctionBehavior::Continuous { .. }
        ));
        let mapping = CompiledPhysicalMapping::compile(channel, function)
            .unwrap()
            .unwrap();
        let result = mapping.raw_for_physical(0.0).unwrap();
        assert_eq!(result.raw, 32768);
        let mut frame = [0; 512];
        mode.encode_channel(&mut frame, 1, channel, result.raw)
            .unwrap();
        assert_eq!(&frame[..2], &[128, 0]);
        assert_eq!(channel.default_raw, 32769);
    }
}

#[test]
fn custom_names_and_explicit_aliases_never_merge_by_sanitizing() {
    let channels = channel("1", "0/1").replace("Pan", "Custom-A")
        + &channel("2", "0/1").replace("Pan", "Custom_A")
        + &channel("3", "0/1").replace("Pan", "PanAlias");
    let xml = fixture(&channels).replace("</Attributes>", r#"<Attribute Name="Custom-A"/><Attribute Name="Custom_A"/><Attribute Name="PanAlias" MainAttribute="Pan" PhysicalUnit="Angle"/></Attributes>"#);
    let profile = from_xml(&xml).unwrap().profile;
    let attributes = profile.modes[0]
        .channels
        .iter()
        .map(|channel| channel.attribute.0.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(attributes, ["gdtf.Custom-A", "gdtf.Custom_A", "pan"]);
    assert!(
        profile.modes[0].channels[2].functions[0]
            .angular_motion
            .is_some()
    );
}

#[test]
fn dangling_and_cyclic_attribute_aliases_and_invalid_function_defaults_fail() {
    let base = fixture(&channel("1", "0/1"));
    for xml in [
        base.replace("Attribute=\"Pan\"", "Attribute=\"Missing\""),
        base.replace("Name=\"Pan\"", "Name=\"Pan\" MainAttribute=\"Missing\""),
        base.replace("Name=\"Pan\"", "Name=\"Pan\" MainAttribute=\"Zoom\"").replace("Name=\"Zoom\"", "Name=\"Zoom\" MainAttribute=\"Pan\""),
        base.replace("</LogicalChannel>", r#"<ChannelFunction Name="Second" Attribute="Pan" DMXFrom="128/1" Default="1/1"/></LogicalChannel>"#),
    ] {
        assert!(from_xml(&xml).is_err(), "{xml}");
    }
}

#[test]
fn declared_master_policy_is_not_replaced_by_an_intensity_heuristic() {
    for (master, group, grand) in [
        ("None", false, false),
        ("Group", true, false),
        ("Grand", false, true),
    ] {
        let xml = fixture(&channel("1", "0/1").replace("Pan", "Dimmer").replace(
            "<LogicalChannel ",
            &format!("<LogicalChannel Master=\"{master}\" "),
        ));
        let profile = from_xml(&xml).unwrap().profile;
        let channel = &profile.modes[0].channels[0];
        assert_eq!(
            (
                channel.reacts_to_group_master,
                channel.reacts_to_grand_master
            ),
            (group, grand)
        );
        assert!(!channel.reacts_to_sequence_master);
    }
}

#[test]
fn mixed_unit_focus_modes_and_duplicate_pan_channels_roundtrip_with_aliases() {
    let mut profile = from_xml(&fixture(&channel("1", "0/1").replace("Pan", "Focus1")))
        .unwrap()
        .profile;
    let mut unknown_mode = profile.modes[0].clone();
    unknown_mode.id = Uuid::new_v4();
    unknown_mode.name = "Unknown units".into();
    let focus = &mut unknown_mode.channels[0];
    focus.unit = None;
    if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut focus.functions[0].behavior {
        *unit = None;
    }
    profile.modes.push(unknown_mode);
    let imported = import_profile(&crate::gdtf::profile::package_profile(&profile).unwrap())
        .unwrap()
        .profile;
    assert_eq!(imported.modes[0].channels[0].attribute.0.as_ref(), "focus");
    assert_eq!(imported.modes[1].channels[0].attribute.0.as_ref(), "focus");
    assert_eq!(
        imported.modes[0].channels[0].unit.as_deref(),
        Some("percent")
    );
    assert_eq!(imported.modes[1].channels[0].unit, None);

    let mut profile = from_xml(&fixture(&channel("1", "0/1"))).unwrap().profile;
    let mut second = profile.modes[0].channels[0].clone();
    second.id = Uuid::new_v4();
    second.functions[0].id = Uuid::new_v4();
    profile.modes[0].channels.push(second);
    profile.modes[0].splits[0].footprint = 2;
    let imported = import_profile(&crate::gdtf::profile::package_profile(&profile).unwrap())
        .unwrap()
        .profile;
    assert!(
        imported.modes[0]
            .channels
            .iter()
            .all(|channel| channel.attribute.0.as_ref() == "pan")
    );
}

#[test]
fn original_source_and_association_survive_package_transfer_and_profile_edits() {
    let profile = from_xml(&fixture(&channel("1,2", "32769/2")))
        .unwrap()
        .profile;
    let source = crate::gdtf::profile::package_profile(&profile).unwrap();
    let imported = import_profile(&source).unwrap().profile;
    let attachment = imported.source_gdtf.as_ref().unwrap();
    assert_eq!(attachment.decoded_archive().unwrap(), source);
    assert!(attachment.matches_profile(&imported).unwrap());
    let library = crate::FixtureLibrary::open(":memory:").unwrap();
    let saved = library
        .save_profile_with_source_gdtf(imported, 0, &source)
        .unwrap();
    let bytes = library
        .export_fixture_package(saved.id, saved.revision)
        .unwrap()
        .unwrap();
    let other = crate::FixtureLibrary::open(":memory:").unwrap();
    let transferred = other.import_fixture_package(&bytes).unwrap();
    let archive = transferred.source_gdtf.as_ref().unwrap();
    assert_eq!(archive.decoded_archive().unwrap(), source);
    assert!(archive.matches_profile(&transferred).unwrap());
    let document = serde_json::to_value(&transferred).unwrap();
    let reopened: FixtureProfile = serde_json::from_value(document).unwrap();
    assert_eq!(reopened.source_gdtf, transferred.source_gdtf);
    let mut edited = reopened;
    edited.modes[0].channels[0].functions[0].name = "Edited".into();
    assert!(
        !edited
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&edited)
            .unwrap()
    );
    let roundtrip =
        crate::read_fixture_package(&crate::write_fixture_package(&edited).unwrap()).unwrap();
    assert_eq!(roundtrip.source_gdtf, transferred.source_gdtf);
    assert!(
        !roundtrip
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&roundtrip)
            .unwrap()
    );
    let mut unverified = roundtrip.clone();
    unverified.source_gdtf.as_mut().unwrap().profile_fingerprint = None;
    assert!(
        !unverified
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&unverified)
            .unwrap()
    );
    let mut tampered = roundtrip;
    tampered.source_gdtf.as_mut().unwrap().archive_sha256 = "0".repeat(64);
    assert!(tampered.validate().is_err());
    assert!(crate::write_fixture_package(&tampered).is_err());
}

#[test]
fn runtime_projections_omit_the_archive_but_full_definitions_preserve_it() {
    let original = from_xml(&fixture(&channel("1", "0/1"))).unwrap().profile;
    let mut profile = import_profile(&crate::gdtf::profile::package_profile(&original).unwrap())
        .unwrap()
        .profile;
    let id = profile.modes[0].id;
    let full = profile.resolved_definition(id).unwrap();
    let compact = profile
        .compact_resolved_definition_from_validated_profile(id)
        .unwrap();
    assert!(compact.profile_snapshot.unwrap().source_gdtf.is_none());
    let full = full.profile_snapshot.unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &full.source_gdtf.as_ref().unwrap().archive_asset,
        &profile.source_gdtf.as_ref().unwrap().archive_asset
    ));
    let mut second = profile.modes[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Second".into();
    profile.modes.push(second);
    let exported = crate::gdtf::profile::package_profile(&profile).unwrap();
    assert_eq!(import_profile(&exported).unwrap().profile.modes.len(), 2);
}

#[test]
fn cmy_and_additive_controls_keep_separate_native_identity() {
    let mut channels = String::new();
    let mut definitions = String::new();
    for (index, source) in [
        "ColorSub_C",
        "ColorSub_M",
        "ColorSub_Y",
        "ColorAdd_R",
        "ColorAdd_W",
        "ColorAdd_UV",
    ]
    .into_iter()
    .enumerate()
    {
        channels.push_str(&channel(&(index + 1).to_string(), "0/1").replace("Pan", source));
        definitions.push_str(&format!("<Attribute Name=\"{source}\"/>"));
    }
    let xml = fixture(&channels).replace("</Attributes>", &(definitions + "</Attributes>"));
    let profile = from_xml(&xml).unwrap().profile;
    let channels = &profile.modes[0].channels;
    assert_eq!(channels[0].fixture_attribute.0.as_ref(), "color.cyan");
    assert_eq!(channels[0].attribute.0.as_ref(), "color.red");
    assert_eq!(
        channels[0].canonical_transform,
        crate::CanonicalTransform::InvertNormalized
    );
    assert_eq!(channels[3].fixture_attribute.0.as_ref(), "color.red");
    assert_eq!(
        channels[3].canonical_transform,
        crate::CanonicalTransform::Identity
    );
    assert_ne!(channels[0].id, channels[3].id);
    assert_eq!(channels[4].attribute.0.as_ref(), "color.white");
    assert_eq!(channels[5].attribute.0.as_ref(), "color.uv");
    assert!(profile.modes[0].color_physical.is_none());
}

#[test]
fn native_hue_saturation_retains_a_whole_color_binding_without_measured_claims() {
    let mut channels = String::new();
    let mut definitions = String::new();
    for (index, source) in ["ColorHSB_Hue", "ColorHSB_Saturation", "ColorHSB_Brightness"]
        .into_iter()
        .enumerate()
    {
        channels.push_str(&channel(&(index + 1).to_string(), "0/1").replace("Pan", source));
        definitions.push_str(&format!("<Attribute Name=\"{source}\"/>"));
    }
    let xml = fixture(&channels).replace("</Attributes>", &(definitions + "</Attributes>"));
    let imported = from_xml(&xml).unwrap();
    let mode = &imported.profile.modes[0];
    assert!(
        matches!(mode.color_systems[0].system, crate::ColorSystem::HueSaturation { hue_channel_id, saturation_channel_id, intensity_channel_id: Some(intensity) }
        if hue_channel_id == mode.channels[0].id && saturation_channel_id == mode.channels[1].id && intensity == mode.channels[2].id)
    );
    assert!(mode.color_physical.is_none());
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("nominal and unmeasured"))
    );
}

#[test]
fn atomic_source_save_associates_the_normalized_profile_and_catalog_omits_archives() {
    let original = from_xml(&fixture(&channel("1", "0/1"))).unwrap().profile;
    let source = crate::gdtf::profile::package_profile(&original).unwrap();
    let mut profile = import_profile(&source).unwrap().profile;
    profile.schema_version = 2;
    profile.reserved_source = Some("vendor-catalog".into());
    let library = crate::FixtureLibrary::open(":memory:").unwrap();
    let saved = library
        .save_profile_with_source_gdtf(profile, 0, &source)
        .unwrap();
    assert_eq!(
        saved.schema_version,
        crate::profile::FIXTURE_PROFILE_SCHEMA_VERSION
    );
    assert_eq!(saved.reserved_source, None);
    assert!(
        saved
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&saved)
            .unwrap()
    );
    assert!(
        library
            .patchable_definitions()
            .unwrap()
            .iter()
            .all(|definition| definition
                .profile_snapshot
                .as_ref()
                .unwrap()
                .source_gdtf
                .is_none())
    );
    assert_eq!(
        library
            .profile(saved.id, saved.revision)
            .unwrap()
            .unwrap()
            .source_gdtf,
        saved.source_gdtf
    );
    // A DB-only write must not report successful replacement while portable metadata keeps the
    // old attachment. Nor may a later edit silently certify the old bytes again.
    assert!(
        library
            .set_profile_source_gdtf(saved.id, saved.revision, b"different")
            .is_err()
    );
    let mut edited = saved.clone();
    edited.name.push_str(" edited");
    let edited = library.save_profile(edited, saved.revision).unwrap();
    assert!(
        library
            .set_profile_source_gdtf(edited.id, edited.revision, &source)
            .is_err()
    );
    assert_eq!(
        library
            .profile_source_gdtf(edited.id, edited.revision)
            .unwrap()
            .unwrap(),
        source
    );
}
