//! Optical reader repairs only for unchanged source-associated GDTF imports.
//! Immutable data, source bytes and fingerprints remain unchanged. No numeric heuristics or
//! fixture-name exceptions: the old reader must reproduce the exact model and its bindings.
use super::{FixtureProfile, OpticalSource, OpticalTransmission};

fn same<T: serde::Serialize>(a: &T, b: &T) -> bool {
    matches!((serde_json::to_value(a), serde_json::to_value(b)), (Ok(a), Ok(b)) if a == b)
}

fn channels_match(
    actual: &super::FixtureMode,
    old: &super::FixtureMode,
    amber_alias: bool,
) -> bool {
    if actual.channels.len() != old.channels.len() {
        return false;
    }
    let mut canonical = old.channels.clone();
    let allowed_alias = |old: &str, current: &str| {
        if old == current {
            return true;
        }
        if amber_alias && old == "gdtf.ColorAdd_RY" && current == "color.amber" {
            return true;
        }
        ["DimmerCurve", "ColorMacro1", "ColorMacro1Rate"]
            .iter()
            .any(|name| {
                old == format!("gdtf.{name}")
                    && current
                        .strip_prefix(&format!("custom.gdtf.{}.", name.to_ascii_lowercase()))
                        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok_and(|id| !id.is_nil()))
            })
    };
    for (source, target) in canonical.iter_mut().zip(&actual.channels) {
        if !allowed_alias(&source.attribute.0, &target.attribute.0)
            || source.functions.len() != target.functions.len()
        {
            return false;
        }
        source.attribute = target.attribute.clone();
        for (function, current) in source.functions.iter_mut().zip(&target.functions) {
            if !allowed_alias(&function.attribute.0, &current.attribute.0) {
                return false;
            }
            function.attribute = current.attribute.clone();
        }
    }
    same(&canonical, &actual.channels)
}

pub(super) fn apply(profile: &mut FixtureProfile) {
    project(profile, true);
}

/// Generated GDTF carries corrected optical units, never synthesized fallback fixture data.
pub(crate) fn for_export(profile: &mut FixtureProfile) {
    project(profile, false);
}

fn project(profile: &mut FixtureProfile, nominal_fallback: bool) {
    let Some(source) = &profile.source_gdtf else {
        return;
    };
    if source.matches_profile(profile).ok() != Some(true) {
        return;
    }
    let Ok(bytes) = source.decoded_archive() else {
        return;
    };
    let Ok(legacy) = crate::gdtf::read::import_legacy_optical_profile(&bytes) else {
        return;
    };
    let Ok(current) = crate::gdtf::read::import_profile(&bytes) else {
        return;
    };
    let amber_alias = crate::gdtf::read::declares_amber_alias(&bytes);
    for mode in &mut profile.modes {
        let (Some(old), Some(new)) = (legacy.mode(mode.id), current.profile.mode(mode.id)) else {
            continue;
        };
        if !channels_match(mode, old, amber_alias) || !same(&mode.heads, &old.heads) {
            continue;
        }
        // Normalized current imports also qualify for wholly-unknown fallback; any operator
        // change to the optical model, including provenance, is deliberately excluded.
        if !same(&mode.color_physical, &old.color_physical)
            && !same(&mode.color_physical, &new.color_physical)
        {
            continue;
        }
        mode.color_physical = new.color_physical.clone();
        // A valid retained-source association does not prove that an explicitly attached
        // legacy Color system came from that source. Preserve authored calibration rather
        // than interpreting it as an uncalibrated importer fallback.
        if !nominal_fallback || !same(&mode.color_systems, &old.color_systems) {
            continue;
        }
        let Some(model) = &mode.color_physical else {
            continue;
        };
        let unknown_heads = model
            .paths
            .iter()
            .filter(|path| {
                matches!(path.source, OpticalSource::Unknown)
                    && path.measurements.is_empty()
                    && path
                        .filters
                        .iter()
                        .all(|filter| matches!(filter.transmission, OpticalTransmission::Unknown))
            })
            .map(|path| path.head_id)
            .collect::<Vec<_>>();
        if unknown_heads.is_empty() {
            continue;
        }
        let mut unmodelled = mode.clone();
        unmodelled.color_physical = None;
        let Some(derived) = unmodelled.derived_color_physical() else {
            continue;
        };
        let model = mode.color_physical.as_mut().unwrap();
        for path in &mut model.paths {
            if unknown_heads.contains(&path.head_id) {
                if let Some(nominal) = derived.paths.iter().find(|p| p.head_id == path.head_id) {
                    *path = nominal.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::CompiledColorFitting;
    use light_core::programming::ColorIntent;
    use std::io::Write;

    fn archive() -> Vec<u8> {
        let xml = r#"<GDTF DataVersion="1.2"><FixtureType Name="Optical scale" Manufacturer="Contract" FixtureTypeID="684af0b8-5e84-4e28-a8a2-687647b2b515"><AttributeDefinitions><Attributes><Attribute Name="ColorAdd_R"/></Attributes></AttributeDefinitions><PhysicalDescriptions><Emitters><Emitter Name="Red" Color="0.64,0.33,21.26729"/></Emitters><ColorSpace Mode="Custom" Red="0.64,0.33,0.2126729" WhitePoint="0.3127,0.329,1"/></PhysicalDescriptions><DMXModes><DMXMode Name="RGB"><DMXChannels><DMXChannel Offset="1" Geometry="Head"><LogicalChannel Attribute="ColorAdd_R"><ChannelFunction Name="Red" Attribute="ColorAdd_R" DMXFrom="0/1" PhysicalFrom="0" PhysicalTo="1" Emitter="Red"/></LogicalChannel></DMXChannel></DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"#;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("description.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn unknown_cmy_archive() -> Vec<u8> {
        let attributes = ["ColorSub_C", "ColorSub_M", "ColorSub_Y", "Color1"]
            .into_iter()
            .map(|name| format!("<Attribute Name=\"{name}\"/>"))
            .collect::<String>();
        let channels = ["ColorSub_C", "ColorSub_M", "ColorSub_Y", "Color1"].into_iter().enumerate().map(|(i, name)| {
            let wheel = if i == 3 { " Wheel=\"Colors\"" } else { "" };
            let set = if i == 3 { "<ChannelSet Name=\"Open\" DMXFrom=\"0/1\" WheelSlotIndex=\"1\"/>" } else { "" };
            format!("<DMXChannel Offset=\"{}\" Geometry=\"Head\"><LogicalChannel Attribute=\"{name}\"><ChannelFunction Name=\"{name}\" Attribute=\"{name}\" DMXFrom=\"0/1\" PhysicalFrom=\"0\" PhysicalTo=\"1\"{wheel}>{set}</ChannelFunction></LogicalChannel></DMXChannel>", i + 1)
        }).collect::<String>();
        let xml = format!(
            "<GDTF DataVersion=\"1.2\"><FixtureType Name=\"Unknown CMY\" Manufacturer=\"Contract\" FixtureTypeID=\"684af0b8-5e84-4e28-a8a2-687647b2b515\"><AttributeDefinitions><Attributes>{attributes}</Attributes></AttributeDefinitions><Wheels><Wheel Name=\"Colors\"><Slot Name=\"Open\"/></Wheel></Wheels><DMXModes><DMXMode Name=\"CMY\"><DMXChannels>{channels}</DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"
        );
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("description.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn source_imported_unknown_cmy_uses_nominal_same_head_without_measured_claim() {
        let bytes = unknown_cmy_archive();
        let original = crate::gdtf::read::import_profile(&bytes).unwrap().profile;
        assert!(matches!(
            original.modes[0].color_physical.as_ref().unwrap().paths[0].source,
            OpticalSource::Unknown
        ));
        let mut projected = original.clone();
        apply(&mut projected);
        let mode = &projected.modes[0];
        let fitter = CompiledColorFitting::compile(&projected, mode.id, None)
            .unwrap()
            .unwrap();
        let mut workspace = fitter.create_workspace();
        let mut result = fitter.create_output(0).unwrap();
        let mut intent = ColorIntent::default();
        intent.recipe.rgb = [1., 0., 0.];
        intent.base_xyz = light_core::srgb_to_xyz(1., 0., 0.);
        fitter
            .fit(0, &[0; 4], &intent, &mut workspace, &mut result)
            .unwrap();
        assert!(result.visible.achieved.is_some());
        assert_eq!(
            result.visible.data_quality,
            super::super::PhysicalDataQuality::Unknown
        );
        assert!(result.visible.nominal);
        assert_eq!(result.head_id, original.modes[0].heads[0].id);
        for attr in ["color.green", "color.blue"] {
            let index = mode
                .channels
                .iter()
                .position(|c| c.attribute.0.as_ref() == attr)
                .unwrap();
            assert_eq!(
                result
                    .writes
                    .iter()
                    .find(|w| w.channel_index as usize == index)
                    .unwrap()
                    .raw,
                255
            );
        }
        assert_eq!(projected.source_gdtf, original.source_gdtf);
    }

    #[test]
    fn unknown_optical_import_preserves_authored_measured_legacy_color_system() {
        let bytes = unknown_cmy_archive();
        let mut profile = crate::gdtf::read::import_profile(&bytes).unwrap().profile;
        let mode = &mut profile.modes[0];
        mode.color_systems.push(super::super::HeadColorSystem {
            head_id: mode.heads[0].id,
            correction_matrix: [[1.01, 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            calibration: super::super::ColorSystemCalibration {
                status: super::super::ColorCalibrationStatus::Measured,
                revision: 7,
                source: Some("Operator spectrometer calibration".into()),
            },
            system: super::super::ColorSystem::Subtractive {
                cyan_channel_id: mode.channels[0].id,
                magenta_channel_id: mode.channels[1].id,
                yellow_channel_id: mode.channels[2].id,
                filters: Some(super::super::SubtractiveCalibration {
                    open_xyz: light_core::srgb_to_xyz(1., 1., 1.),
                    cyan_xyz: light_core::srgb_to_xyz(0., 1., 1.),
                    magenta_xyz: light_core::srgb_to_xyz(1., 0., 1.),
                    yellow_xyz: light_core::srgb_to_xyz(1., 1., 0.),
                }),
            },
        });
        profile.validate().unwrap();
        // The archive association itself can be valid after an explicit attachment. That
        // does not authorize replacing a color system the retained importer never declared.
        profile.source_gdtf = Some(crate::ProfileGdtfSource::associate(&profile, &bytes).unwrap());
        assert!(
            profile
                .source_gdtf
                .as_ref()
                .unwrap()
                .matches_profile(&profile)
                .unwrap()
        );
        let before = serde_json::to_value(&profile).unwrap();
        apply(&mut profile);
        assert_eq!(
            before,
            serde_json::to_value(&profile).unwrap(),
            "authored calibration, source association and Unknown optical path stay unchanged"
        );
        assert!(matches!(
            profile.modes[0].color_physical.as_ref().unwrap().paths[0].source,
            OpticalSource::Unknown
        ));
    }

    #[test]
    fn legacy_optical_projection_is_exact_guarded_and_idempotent() {
        let bytes = archive();
        let mut old = crate::gdtf::read::import_legacy_optical_profile(&bytes).unwrap();
        old.source_gdtf = Some(crate::ProfileGdtfSource::associate(&old, &bytes).unwrap());
        let before = serde_json::to_value(&old).unwrap();
        let mut projected = old.clone();
        apply(&mut projected);
        let OpticalSource::Additive { emitters } =
            &projected.modes[0].color_physical.as_ref().unwrap().paths[0].source
        else {
            panic!()
        };
        assert!((emitters[0].xyz.unwrap().y - 0.2126729).abs() < 1e-7);
        assert_eq!(before, serde_json::to_value(&old).unwrap());
        assert_eq!(projected.source_gdtf, old.source_gdtf);
        let generated = crate::gdtf::profile::package_profile(&old).unwrap();
        let imported = crate::gdtf::read::import_profile(&generated)
            .unwrap()
            .profile;
        assert!(same(
            &imported.modes[0].color_physical,
            &projected.modes[0].color_physical
        ));
        let xml = crate::gdtf::read::archive_xml(&bytes).unwrap();
        assert!(
            xml.contains("WhitePoint=\"0.3127,0.329,1\""),
            "ColorSpace white Y1 is retained verbatim, not treated as optical white100"
        );

        let once = serde_json::to_value(&projected).unwrap();
        apply(&mut projected);
        assert_eq!(once, serde_json::to_value(&projected).unwrap());

        // Explicitly attached operator-authored data does not qualify just because the archive
        // and association are valid: the old reader must reproduce the actual authored model.
        let OpticalSource::Additive { emitters } =
            &mut old.modes[0].color_physical.as_mut().unwrap().paths[0].source
        else {
            panic!()
        };
        emitters[0].maximum_level = 0.5;
        old.source_gdtf = Some(crate::ProfileGdtfSource::associate(&old, &bytes).unwrap());
        let authored = serde_json::to_value(&old).unwrap();
        apply(&mut old);
        assert_eq!(authored, serde_json::to_value(&old).unwrap());
    }

    #[test]
    fn arbitrary_semantic_remaps_and_native_changes_remain_blocked() {
        let bytes = archive();
        let old = crate::gdtf::read::import_legacy_optical_profile(&bytes).unwrap();
        let source = &old.modes[0];
        let mut edited = source.clone();
        edited.channels[0].attribute = light_core::AttributeKey("intensity".into());
        assert!(!channels_match(&edited, source, true));
        let mut known = source.clone();
        known.channels[0].attribute = light_core::AttributeKey("gdtf.DimmerCurve".into());
        known.channels[0].functions[0].attribute = known.channels[0].attribute.clone();
        let mut alias = known.clone();
        alias.channels[0].attribute = light_core::AttributeKey(
            format!("custom.gdtf.dimmercurve.{}", uuid::Uuid::new_v4()).into(),
        );
        alias.channels[0].functions[0].attribute = alias.channels[0].attribute.clone();
        assert!(channels_match(&alias, &known, false));
        alias.channels[0].functions[0].dmx_to = 254;
        assert!(!channels_match(&alias, &known, false));
        let mut amber = source.clone();
        amber.channels[0].attribute = light_core::AttributeKey("gdtf.ColorAdd_RY".into());
        amber.channels[0].functions[0].attribute = amber.channels[0].attribute.clone();
        let mut corrected = amber.clone();
        corrected.channels[0].attribute = light_core::AttributeKey("color.amber".into());
        corrected.channels[0].functions[0].attribute = corrected.channels[0].attribute.clone();
        assert!(!channels_match(&corrected, &amber, false));
        assert!(channels_match(&corrected, &amber, true));
    }

    #[test]
    fn stale_or_unassociated_import_is_not_reinterpreted() {
        let bytes = archive();
        let mut profile = crate::gdtf::read::import_legacy_optical_profile(&bytes).unwrap();
        let before = serde_json::to_value(&profile).unwrap();
        apply(&mut profile);
        assert_eq!(before, serde_json::to_value(&profile).unwrap());
        profile.source_gdtf = Some(crate::ProfileGdtfSource::associate(&profile, &bytes).unwrap());
        profile.notes.push_str(" operator edit");
        let edited = serde_json::to_value(&profile).unwrap();
        apply(&mut profile);
        assert_eq!(edited, serde_json::to_value(&profile).unwrap());
    }

    #[test]
    #[ignore = "explicit local acceptance input; contains no shipped manufacturer fixture data"]
    fn retained_manufacturer_import_actual_color_fit() {
        let path = std::env::var("LIGHT_FIXTURE_ACCEPTANCE_PROFILE")
            .expect("exact read-only profile export path");
        let mut profile: FixtureProfile =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        if std::env::var_os("LIGHT_FIXTURE_ACCEPTANCE_FRESH_SOURCE").is_some() {
            let bytes = profile
                .source_gdtf
                .as_ref()
                .unwrap()
                .decoded_archive()
                .unwrap();
            profile = crate::gdtf::read::import_profile(&bytes).unwrap().profile;
        }
        let source = profile.source_gdtf.as_ref().expect("retained source");
        assert!(
            source.matches_profile(&profile).unwrap(),
            "source association must still be valid"
        );
        let legacy =
            crate::gdtf::read::import_legacy_optical_profile(&source.decoded_archive().unwrap())
                .unwrap();
        for mode in &profile.modes {
            if let Some(old) = legacy.mode(mode.id) {
                eprintln!(
                    "compat {} channels={} heads={} optical={}",
                    mode.name,
                    same(&mode.channels, &old.channels),
                    same(&mode.heads, &old.heads),
                    same(&mode.color_physical, &old.color_physical)
                );
                if let Ok(dir) = std::env::var("LIGHT_FIXTURE_ACCEPTANCE_DIFF_DIR") {
                    std::fs::write(
                        std::path::Path::new(&dir).join(format!("old-{}.json", mode.name)),
                        serde_json::to_vec_pretty(old).unwrap(),
                    )
                    .unwrap();
                    std::fs::write(
                        std::path::Path::new(&dir).join(format!("installed-{}.json", mode.name)),
                        serde_json::to_vec_pretty(mode).unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        let mut projected = profile.clone();
        apply(&mut projected);
        let mut changed = false;
        for mode in &projected.modes {
            let Some(fitter) = CompiledColorFitting::compile(&projected, mode.id, None).unwrap()
            else {
                continue;
            };
            let mut workspace = fitter.create_workspace();
            let mut intent = ColorIntent::default();
            intent.recipe.rgb = [1., 0., 0.];
            intent.base_xyz = light_core::srgb_to_xyz(1., 0., 0.);
            for index in 0..mode.heads.len() {
                let Some(mut result) = fitter.create_output(index) else {
                    continue;
                };
                fitter
                    .fit(
                        index,
                        &vec![0; mode.channels.len()],
                        &intent,
                        &mut workspace,
                        &mut result,
                    )
                    .unwrap();
                eprintln!(
                    "{} {:?} {:?} {:?}",
                    mode.name, result.head_id, result.visible, result.writes
                );
                if result.visible.achieved.is_some() {
                    changed = true;
                }
            }
        }
        assert!(
            changed,
            "at least one physical color engine must now produce a known nominal output"
        );
        assert!(
            !same(&profile.modes, &projected.modes),
            "actual legacy model must be corrected"
        );
        assert_eq!(profile.source_gdtf, projected.source_gdtf);
        projected.validate().unwrap();
    }
}
