use super::*;

#[cfg(test)]
mod color_range_tests {
    use super::{PickerColor, color_range_color, hsv_to_rgb};

    fn hues(start: (f32, f32), end: (f32, f32), travel: f32, count: usize) -> Vec<f32> {
        (0..count)
            .map(|index| {
                let color = color_range_color(start, end, travel, 1.0, index, count);
                (color.hue * 360.0 * 10.0).round() / 10.0
            })
            .collect()
    }

    #[test]
    fn one_full_revolution_from_red_distributes_the_wheel_without_repeating() {
        // Maintainer-pinned: 3 fixtures, red → red with one revolution → 0°, 120°, 240°.
        assert_eq!(hues((0.0, 1.0), (0.0, 1.0), 1.0, 3), [0.0, 120.0, 240.0]);
        // Two revolutions cover the wheel twice across six fixtures.
        assert_eq!(
            hues((0.0, 1.0), (0.0, 1.0), 2.0, 6),
            [0.0, 120.0, 240.0, 0.0, 120.0, 240.0]
        );
        // Reverse direction winds the other way.
        assert_eq!(hues((0.0, 1.0), (0.0, 1.0), -1.0, 3), [0.0, 240.0, 120.0]);
    }

    #[test]
    fn open_arcs_pin_both_endpoints_and_wrap_through_the_seam() {
        // Straight short-way range keeps the former client semantics: endpoints exact.
        assert_eq!(
            hues((0.0, 1.0), (2.0 / 3.0, 1.0), 2.0 / 3.0, 3),
            [0.0, 120.0, 240.0]
        );
        // The long way around from red to blue passes through the wrap seam.
        let long_way = hues((0.0, 1.0), (2.0 / 3.0, 1.0), -1.0 / 3.0, 3);
        assert_eq!(long_way, [0.0, 300.0, 240.0]);
        // Saturation interpolates linearly while brightness stays uniform.
        let middle = color_range_color((0.0, 0.2), (0.5, 0.8), 0.5, 0.4, 1, 3);
        assert_eq!(middle.saturation, 0.5);
        assert_eq!(middle.brightness, 0.4);
    }

    #[test]
    fn single_and_zero_counts_resolve_to_the_end_color() {
        let single = color_range_color((0.1, 0.3), (0.6, 0.9), 0.5, 0.7, 0, 1);
        assert_eq!((single.hue, single.saturation), (0.6, 0.9));
        let none = color_range_color((0.1, 0.3), (0.6, 0.9), 0.5, 0.7, 0, 0);
        assert_eq!((none.hue, none.saturation), (0.6, 0.9));
    }

    #[test]
    fn hsv_conversion_matches_the_former_client_table() {
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 0.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 1.0 / 3.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [0.0, 1.0, 0.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 2.0 / 3.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 0.0,
                saturation: 0.0,
                brightness: 0.5
            }),
            [0.5, 0.5, 0.5]
        );
    }
}

#[cfg(test)]
mod canonical_migration_tests {
    use super::*;

    #[test]
    fn canonical_migration_table_covers_inverse_cmy_and_identity_consolidations() {
        for (source, target) in [
            ("color.cyan", "color.red"),
            ("color.magenta", "color.green"),
            ("color.yellow", "color.blue"),
        ] {
            assert_eq!(
                canonical_attribute_migration(&AttributeKey(source.into())),
                Some((
                    AttributeKey(target.into()),
                    CanonicalAttributeTransform::InvertNormalized
                ))
            );
        }
        assert_eq!(
            canonical_attribute_migration(&AttributeKey("color.cold_white".into())),
            Some((
                AttributeKey("color.white".into()),
                CanonicalAttributeTransform::Identity
            ))
        );
        assert_eq!(
            canonical_attribute_migration(&AttributeKey("color.warm_white".into())),
            Some((
                AttributeKey("color.amber".into()),
                CanonicalAttributeTransform::Identity
            ))
        );
        assert_eq!(
            canonical_attribute_migration(&AttributeKey("strobe".into())),
            Some((
                AttributeKey("shutter".into()),
                CanonicalAttributeTransform::Identity
            ))
        );
        assert_eq!(
            canonical_attribute_migration(&AttributeKey("vendor.strobe".into())),
            None,
            "custom identities never migrate by label"
        );
    }

    #[test]
    fn inverse_transform_handles_scalar_and_spread_values_without_double_inversion() {
        let mut scalar = AttributeValue::Normalized(0.2);
        transform_canonical_value(&mut scalar, CanonicalAttributeTransform::InvertNormalized)
            .unwrap();
        assert_eq!(scalar, AttributeValue::Normalized(0.8));

        let mut spread = AttributeValue::Spread(vec![0.0, 0.25, 1.0]);
        transform_canonical_value(&mut spread, CanonicalAttributeTransform::InvertNormalized)
            .unwrap();
        assert_eq!(spread, AttributeValue::Spread(vec![1.0, 0.75, 0.0]));

        let mut discrete = AttributeValue::Discrete("open".into());
        assert!(
            transform_canonical_value(&mut discrete, CanonicalAttributeTransform::InvertNormalized)
                .is_err()
        );
        assert_eq!(discrete, AttributeValue::Discrete("open".into()));
    }
}

#[cfg(test)]
mod spread_tests {
    use super::{resolve_spread, spread_position};

    fn percentages(points: &[f32], count: usize) -> Vec<f32> {
        resolve_spread(points, count)
            .into_iter()
            .map(|value| (value * 100.0 * 10.0).round() / 10.0)
            .collect()
    }

    #[test]
    fn normative_table_for_100_thru_0_thru_100() {
        let points = [1.0, 0.0, 1.0];
        assert_eq!(percentages(&points, 4), [100.0, 0.0, 0.0, 100.0]);
        assert_eq!(percentages(&points, 5), [100.0, 50.0, 0.0, 50.0, 100.0]);
        assert_eq!(
            percentages(&points, 6),
            [100.0, 50.0, 0.0, 0.0, 50.0, 100.0]
        );
        assert_eq!(
            percentages(&points, 10),
            [100.0, 75.0, 50.0, 25.0, 0.0, 0.0, 25.0, 50.0, 75.0, 100.0]
        );
    }

    #[test]
    fn asymmetric_and_four_point_vectors_place_every_anchor_exactly() {
        // 10 THRU 80 THRU 20 over 5: interior ideal position 2 is an exact item.
        assert_eq!(
            percentages(&[0.1, 0.8, 0.2], 5),
            [10.0, 45.0, 80.0, 50.0, 20.0]
        );
        // Four points over 7: ideals 0, 2, 4, 6 are all integer anchors.
        assert_eq!(
            percentages(&[0.0, 1.0, 0.25, 0.75], 7),
            [0.0, 50.0, 100.0, 62.5, 25.0, 50.0, 75.0]
        );
        // Non-half nearest anchor: 3 points over 7 → interior ideal 3 exact.
        assert_eq!(
            percentages(&[0.0, 1.0, 0.0], 7),
            [0.0, 33.3, 66.7, 100.0, 66.7, 33.3, 0.0]
        );
    }

    #[test]
    fn reversed_control_points_mirror_the_resolution() {
        for count in [4_usize, 5, 6, 9, 10] {
            let forward = resolve_spread(&[0.1, 0.9, 0.3], count);
            let mut mirrored = resolve_spread(&[0.3, 0.9, 0.1], count);
            mirrored.reverse();
            assert_eq!(forward, mirrored, "count {count}");
        }
    }

    #[test]
    fn boundaries_stay_total_and_established_meanings_hold() {
        assert!(resolve_spread(&[1.0, 0.0], 0).is_empty());
        assert_eq!(resolve_spread(&[0.4, 0.8], 1), [0.4]);
        assert_eq!(resolve_spread(&[0.7], 4), [0.7, 0.7, 0.7, 0.7]);
        // Established two-point interpolation is unchanged.
        assert_eq!(percentages(&[0.0, 1.0], 5), [0.0, 25.0, 50.0, 75.0, 100.0]);
        // Equal adjacent points expand as a flat plateau.
        assert_eq!(
            percentages(&[0.0, 0.0, 1.0], 5),
            [0.0, 0.0, 0.0, 50.0, 100.0]
        );
        // More points than items degrades to linear sampling for stored legacy spreads.
        assert_eq!(percentages(&[0.0, 1.0, 0.0, 1.0], 3), [0.0, 50.0, 100.0]);
        // Repeated evaluation is byte-for-byte stable and finite.
        let first = resolve_spread(&[0.2, 0.9, 0.1], 8);
        assert_eq!(first, resolve_spread(&[0.2, 0.9, 0.1], 8));
        assert!(first.iter().all(|value| value.is_finite()));
        assert_eq!(spread_position(&[0.2, 0.9, 0.1], 0, 8), first[0]);
    }
}

#[cfg(test)]
mod attribute_registry_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn built_in_ids_are_unique_and_only_recordable_continuous_values_are_scalar_lanes() {
        let mut ids = HashSet::new();
        for descriptor in ATTRIBUTE_REGISTRY {
            assert!(ids.insert(descriptor.id), "duplicate {}", descriptor.id);
            assert!(!descriptor.id.trim().is_empty());
            assert_eq!(descriptor.default_unit, descriptor.display_unit);
            if descriptor.normalized_bounds.is_some() {
                assert_eq!(descriptor.value_type, AttributeValueType::Continuous);
                assert!(descriptor.recordable);
            }
            if let Some(bounds) = descriptor.normalized_bounds {
                assert!(bounds.min.is_finite());
                assert!(bounds.max.is_finite());
                assert!(bounds.min < bounds.max);
            }
        }
    }

    #[test]
    fn unknown_identity_is_retained_but_is_not_assumed_safe_for_dynamics() {
        let key = AttributeKey("vendor.custom.zoomish".into());
        let descriptor = attribute_descriptor(&key);
        assert_eq!(descriptor.id, key.0);
        assert_eq!(descriptor.label, key.0);
        assert_eq!(descriptor.family, AttributeClass::Custom);
        assert!(!descriptor.built_in);
        assert!(!descriptor.recordable);
        assert_eq!(descriptor.normalized_bounds, None);
    }

    #[test]
    fn recommended_configuration_is_complete_stable_and_symmetric() {
        let configuration = AttributeConfiguration::recommended();
        configuration.validate().unwrap();
        assert_eq!(configuration.version, ATTRIBUTE_CONFIGURATION_VERSION);
        assert_eq!(
            configuration.placements.len(),
            ATTRIBUTE_REGISTRY
                .iter()
                .filter(|descriptor| {
                    !built_in_attribute_is_retired(descriptor.id)
                        && !built_in_attribute_is_projection_only(descriptor.id)
                })
                .count()
        );
        for id in PROJECTION_ONLY_BUILT_IN_ATTRIBUTES {
            let key = AttributeKey((*id).into());
            let descriptor = attribute_descriptor(&key);
            assert!(descriptor.built_in);
            assert!(!descriptor.recordable);
            assert_eq!(configuration.placement_for(&key), None);
            assert_eq!(configuration.activation_group_for(&key), None);
        }
        assert_eq!(
            configuration.placement_for(&AttributeKey("color.red".into())),
            Some(EncoderPlacement::new(EncoderGroup::Color, 1, 1))
        );
        assert_eq!(
            configuration.placement_for(&AttributeKey("iris".into())),
            Some(EncoderPlacement::new(EncoderGroup::Shapers, 1, 1))
        );
        assert_eq!(
            configuration.placement_for(&AttributeKey("media.folder".into())),
            Some(EncoderPlacement::new(EncoderGroup::Media, 1, 1))
        );
        assert!(
            attribute_descriptor(&AttributeKey("media.rotation".into())).cyclic,
            "canonical rotation metadata must wrap instead of clamp"
        );
        assert_eq!(
            configuration
                .placements
                .iter()
                .map(|placement| placement.encoder.group)
                .collect::<HashSet<_>>(),
            HashSet::from([
                EncoderGroup::Intensity,
                EncoderGroup::Color,
                EncoderGroup::Position,
                EncoderGroup::Beam,
                EncoderGroup::Shapers,
                EncoderGroup::Focus,
                EncoderGroup::Control,
                EncoderGroup::Media,
            ])
        );
        assert_eq!(
            configuration
                .activation_group_for(&AttributeKey("color.wheel.1".into()))
                .unwrap()
                .members,
            [
                AttributeKey("color.wheel.1".into()),
                AttributeKey("color.wheel.1.rotation".into())
            ]
        );
        let links = configuration.activation_links();
        assert_eq!(
            links[&AttributeKey("pan".into())],
            [
                AttributeKey("tilt".into()),
                AttributeKey("position.rotation".into())
            ]
        );
        assert_eq!(
            links[&AttributeKey("tilt".into())],
            [
                AttributeKey("pan".into()),
                AttributeKey("position.rotation".into())
            ]
        );
        assert_eq!(
            links[&AttributeKey("media.folder".into())],
            [AttributeKey("media.file".into())]
        );
        assert!(links[&AttributeKey("intensity".into())].is_empty());
        assert!(!links.contains_key(&AttributeKey("control".into())));
    }

    #[test]
    fn saved_legacy_catalog_configuration_adds_new_built_ins_without_losing_custom_choices() {
        let legacy_ids = HashSet::from([
            "intensity",
            "color",
            "color.red",
            "color.green",
            "color.blue",
            "color.cyan",
            "color.magenta",
            "color.yellow",
            "color.amber",
            "color.white",
            "color.uv",
            "color.wheel.1",
            "color.wheel.2",
            "pan",
            "tilt",
            "beam",
            "focus",
            "zoom",
            "iris",
            "gobo.1",
            "gobo.2",
            "shutter",
            "strobe",
            "control",
        ]);
        let mut legacy = AttributeConfiguration::recommended();
        legacy
            .placements
            .retain(|placement| legacy_ids.contains(placement.attribute.0.as_str()));
        for group in &mut legacy.activation_groups {
            group
                .members
                .retain(|member| legacy_ids.contains(member.0.as_str()));
        }
        legacy
            .activation_groups
            .retain(|group| !group.members.is_empty());
        add_custom(
            &mut legacy,
            custom_descriptor("vendor.media_surface", AttributeValueType::Continuous, true),
            EncoderPlacement::new(EncoderGroup::Media, 1, 1),
        );

        let upgraded = legacy.with_current_built_ins();
        upgraded.validate().unwrap();
        assert_eq!(
            upgraded.placement_for(&AttributeKey("media.folder".into())),
            Some(EncoderPlacement::new(EncoderGroup::Media, 1, 1))
        );
        assert_eq!(
            upgraded.placement_for(&AttributeKey("vendor.media_surface".into())),
            Some(EncoderPlacement::new(EncoderGroup::Media, 4, 1))
        );
        assert_eq!(
            upgraded
                .activation_group_for(&AttributeKey("vendor.media_surface".into()))
                .unwrap()
                .members,
            [AttributeKey("vendor.media_surface".into())]
        );
        assert_eq!(upgraded.clone().with_current_built_ins(), upgraded);
    }

    #[test]
    fn encoder_group_vocabulary_is_exact_and_configuration_is_clone_stable() {
        let groups = [
            EncoderGroup::Intensity,
            EncoderGroup::Color,
            EncoderGroup::Position,
            EncoderGroup::Beam,
            EncoderGroup::Shapers,
            EncoderGroup::Focus,
            EncoderGroup::Control,
            EncoderGroup::Media,
        ];
        assert_eq!(groups.len(), 8);
        let configuration = AttributeConfiguration::recommended();
        assert_eq!(configuration.clone(), configuration);
    }

    #[test]
    fn retired_placeholder_built_ins_leave_new_pages_but_old_shows_remain_valid() {
        let recommended = AttributeConfiguration::recommended();
        for retired in RETIRED_BUILT_IN_ATTRIBUTES {
            assert!(
                ATTRIBUTE_REGISTRY
                    .iter()
                    .any(|descriptor| descriptor.id == *retired),
                "{retired} remains decodable for compatibility"
            );
            assert!(
                recommended
                    .attribute_placement_for(&AttributeKey((*retired).into()))
                    .is_none(),
                "{retired} must not reserve a new encoder slot"
            );
        }

        let mut legacy = recommended;
        legacy.placements.push(AttributePlacement {
            attribute: AttributeKey("beam".into()),
            encoder: EncoderPlacement::new(EncoderGroup::Beam, 99, 1),
            push_turn_of: None,
        });
        legacy.activation_groups.push(AttributeActivationGroup {
            id: "legacy.beam".into(),
            label: "Legacy Beam".into(),
            members: vec![AttributeKey("beam".into())],
        });
        let upgraded = legacy.with_current_built_ins();
        upgraded.validate().unwrap();
        assert_eq!(
            upgraded.placement_for(&AttributeKey("beam".into())),
            Some(EncoderPlacement::new(EncoderGroup::Beam, 99, 1))
        );
    }

    #[test]
    fn retired_consolidation_placeholders_remain_decodable_in_old_shows() {
        let retired = [
            "control.mode",
            "control.speed",
            "pan.time",
            "tilt.time",
            "shaper.keystone.x",
            "shaper.keystone.y",
        ];
        let mut legacy = AttributeConfiguration::recommended();
        for (index, attribute) in retired.into_iter().enumerate() {
            legacy.placements.push(AttributePlacement {
                attribute: AttributeKey(attribute.into()),
                encoder: EncoderPlacement::new(
                    EncoderGroup::Control,
                    90 + u16::try_from(index).unwrap(),
                    1,
                ),
                push_turn_of: None,
            });
            legacy.activation_groups.push(AttributeActivationGroup {
                id: format!("legacy.{attribute}"),
                label: attribute.into(),
                members: vec![AttributeKey(attribute.into())],
            });
        }

        let upgraded = legacy.with_current_built_ins();
        upgraded.validate().unwrap();
        for (index, attribute) in retired.into_iter().enumerate() {
            assert_eq!(
                upgraded.placement_for(&AttributeKey(attribute.into())),
                Some(EncoderPlacement::new(
                    EncoderGroup::Control,
                    90 + u16::try_from(index).unwrap(),
                    1,
                )),
                "old shows retain the explicit {attribute} placement"
            );
        }
    }

    #[test]
    fn retired_strobe_keeps_an_explicit_old_show_placement() {
        let mut legacy = AttributeConfiguration::recommended();
        legacy.placements.push(AttributePlacement {
            attribute: AttributeKey("strobe".into()),
            encoder: EncoderPlacement::new(EncoderGroup::Intensity, 99, 1),
            push_turn_of: None,
        });
        legacy.activation_groups.push(AttributeActivationGroup {
            id: "legacy.strobe".into(),
            label: "Legacy Strobe".into(),
            members: vec![AttributeKey("strobe".into())],
        });

        let upgraded = legacy.with_current_built_ins();
        upgraded.validate().unwrap();
        assert_eq!(
            upgraded.placement_for(&AttributeKey("strobe".into())),
            Some(EncoderPlacement::new(EncoderGroup::Intensity, 99, 1))
        );
    }

    #[test]
    fn legacy_default_cmy_controls_retire_into_existing_rgb_controls() {
        let mut legacy = AttributeConfiguration::recommended();
        for (source, encoder, label) in [
            (
                "color.cyan",
                EncoderPlacement::new(EncoderGroup::Color, 4, 1),
                "Cyan",
            ),
            (
                "color.magenta",
                EncoderPlacement::new(EncoderGroup::Color, 4, 2),
                "Magenta",
            ),
            (
                "color.yellow",
                EncoderPlacement::new(EncoderGroup::Color, 4, 3),
                "Yellow",
            ),
        ] {
            legacy.placements.push(AttributePlacement {
                attribute: AttributeKey(source.into()),
                encoder,
                push_turn_of: None,
            });
            legacy.activation_groups.push(AttributeActivationGroup {
                id: source.into(),
                label: label.into(),
                members: vec![AttributeKey(source.into())],
            });
        }

        let migrated = legacy.migrate_canonical_attributes().unwrap();
        migrated.validate().unwrap();
        for source in ["color.cyan", "color.magenta", "color.yellow"] {
            assert!(
                migrated
                    .attribute_placement_for(&AttributeKey(source.into()))
                    .is_none()
            );
            assert!(
                migrated
                    .activation_group_for(&AttributeKey(source.into()))
                    .is_none()
            );
        }
        assert_eq!(
            migrated.clone().migrate_canonical_attributes().unwrap(),
            migrated
        );
    }

    #[test]
    fn legacy_cold_and_warm_white_controls_join_existing_white_and_amber_controls() {
        let mut legacy = AttributeConfiguration::recommended();
        for (source, encoder) in [
            (
                "color.cold_white",
                EncoderPlacement::new(EncoderGroup::Color, 2, 1),
            ),
            (
                "color.warm_white",
                EncoderPlacement::new(EncoderGroup::Color, 2, 2),
            ),
        ] {
            legacy.placements.push(AttributePlacement {
                attribute: AttributeKey(source.into()),
                encoder,
                push_turn_of: None,
            });
            legacy
                .activation_groups
                .iter_mut()
                .find(|group| group.id == "color_mix")
                .unwrap()
                .members
                .push(AttributeKey(source.into()));
        }

        let migrated = legacy.migrate_canonical_attributes().unwrap();
        migrated.validate().unwrap();
        for source in ["color.cold_white", "color.warm_white"] {
            assert!(
                migrated
                    .attribute_placement_for(&AttributeKey(source.into()))
                    .is_none()
            );
            assert!(
                migrated
                    .activation_group_for(&AttributeKey(source.into()))
                    .is_none()
            );
        }
        assert_eq!(
            migrated.clone().migrate_canonical_attributes().unwrap(),
            migrated
        );
    }

    #[test]
    fn customized_cmy_and_rgb_controls_report_an_explicit_conflict() {
        let mut legacy = AttributeConfiguration::recommended();
        legacy.placements.push(AttributePlacement {
            attribute: AttributeKey("color.cyan".into()),
            encoder: EncoderPlacement::new(EncoderGroup::Color, 9, 1),
            push_turn_of: None,
        });
        legacy.activation_groups.push(AttributeActivationGroup {
            id: "authored.cmy".into(),
            label: "Authored CMY".into(),
            members: vec![AttributeKey("color.cyan".into())],
        });

        assert!(matches!(
            legacy.migrate_canonical_attributes(),
            Err(AttributeConfigurationError::CanonicalPlacementConflict {
                legacy,
                canonical,
            }) if legacy == "color.cyan" && canonical == "color.red"
        ));
    }

    #[test]
    fn prism_and_animation_rotations_are_validated_push_turn_companions() {
        let recommended = AttributeConfiguration::recommended();
        for (companion, parent) in [
            ("prism.1.rotation", "prism.1"),
            ("prism.2.rotation", "prism.2"),
            ("animation.1.rotation", "animation.1"),
        ] {
            assert_eq!(
                recommended
                    .attribute_placement_for(&AttributeKey(companion.into()))
                    .and_then(|placement| placement.push_turn_of.as_ref())
                    .map(|attribute| attribute.0.as_str()),
                Some(parent)
            );
        }
        recommended.validate().unwrap();

        let mut legacy = recommended.clone();
        for placement in &mut legacy.placements {
            placement.push_turn_of = None;
        }
        assert!(
            legacy
                .placements
                .iter()
                .all(|placement| placement.push_turn_of.is_none())
        );
        assert_eq!(legacy.with_current_built_ins(), recommended);
    }

    #[test]
    fn push_turn_companions_reject_missing_chained_cross_group_and_duplicate_parents() {
        let companion_index = |configuration: &AttributeConfiguration| {
            configuration
                .placements
                .iter()
                .position(|placement| placement.attribute.0 == "prism.1.rotation")
                .unwrap()
        };

        let mut missing = AttributeConfiguration::recommended();
        let index = companion_index(&missing);
        missing.placements[index].push_turn_of = Some(AttributeKey("missing".into()));
        assert!(matches!(
            missing.validate(),
            Err(AttributeConfigurationError::InvalidPushTurnParent { .. })
        ));

        let mut chained = AttributeConfiguration::recommended();
        let parent = chained
            .placements
            .iter_mut()
            .find(|placement| placement.attribute.0 == "prism.1")
            .unwrap();
        parent.push_turn_of = Some(AttributeKey("gobo.1".into()));
        assert!(matches!(
            chained.validate(),
            Err(AttributeConfigurationError::InvalidPushTurnParent { .. })
        ));

        let mut cross_group = AttributeConfiguration::recommended();
        let index = companion_index(&cross_group);
        cross_group.placements[index].push_turn_of = Some(AttributeKey("pan".into()));
        assert!(matches!(
            cross_group.validate(),
            Err(AttributeConfigurationError::CrossEncoderPushTurn { .. })
        ));

        let mut duplicate = AttributeConfiguration::recommended();
        let animation = duplicate
            .placements
            .iter_mut()
            .find(|placement| placement.attribute.0 == "animation.1.rotation")
            .unwrap();
        animation.push_turn_of = Some(AttributeKey("prism.1".into()));
        assert!(matches!(
            duplicate.validate(),
            Err(AttributeConfigurationError::DuplicatePushTurnCompanion { .. })
        ));
    }

    fn custom_descriptor(
        id: &str,
        value_type: AttributeValueType,
        recordable: bool,
    ) -> CustomAttributeDescriptor {
        CustomAttributeDescriptor {
            id: AttributeKey(id.into()),
            label: "Custom Feature".into(),
            value_type,
            display_unit: Some("percent".into()),
            physical_unit: None,
            normalized_bounds: (value_type == AttributeValueType::Continuous)
                .then_some(AttributeBounds { min: 0.0, max: 1.0 }),
            domain_bounds: None,
            cyclic: false,
            recordable,
            lifecycle: CustomAttributeLifecycle::Active,
        }
    }

    fn add_custom(
        configuration: &mut AttributeConfiguration,
        descriptor: CustomAttributeDescriptor,
        encoder: EncoderPlacement,
    ) {
        let id = descriptor.id.clone();
        configuration.custom_attributes.push(descriptor);
        configuration.placements.push(AttributePlacement {
            attribute: id.clone(),
            encoder,
            push_turn_of: None,
        });
        if configuration
            .custom_attributes
            .last()
            .is_some_and(|descriptor| descriptor.recordable)
        {
            configuration
                .activation_groups
                .push(AttributeActivationGroup {
                    id: format!("activation.{}", id.0),
                    label: "Custom Feature".into(),
                    members: vec![id],
                });
        }
    }

    #[test]
    fn custom_metadata_accepts_namespaced_ids_and_retirement_without_changing_identity() {
        let mut configuration = AttributeConfiguration::recommended();
        let mut descriptor =
            custom_descriptor("vendor.feature-name", AttributeValueType::Continuous, true);
        descriptor.lifecycle = CustomAttributeLifecycle::Retired;
        add_custom(
            &mut configuration,
            descriptor,
            EncoderPlacement::new(EncoderGroup::Beam, 9, 1),
        );
        configuration.validate().unwrap();
        assert_eq!(
            configuration.custom_attributes[0].id,
            AttributeKey("vendor.feature-name".into())
        );
        let generated = CustomAttributeDescriptor::generated_id();
        assert!(generated.0.starts_with("custom."));
        assert!(valid_custom_attribute_id(&generated.0));
    }

    #[test]
    fn custom_ids_cannot_shadow_built_ins_or_use_unstable_syntax() {
        let mut shadow = AttributeConfiguration::recommended();
        add_custom(
            &mut shadow,
            custom_descriptor("color.red", AttributeValueType::Continuous, true),
            EncoderPlacement::new(EncoderGroup::Color, 9, 1),
        );
        assert_eq!(
            shadow.validate(),
            Err(AttributeConfigurationError::BuiltInShadow(
                "color.red".into()
            ))
        );

        let mut invalid = AttributeConfiguration::recommended();
        add_custom(
            &mut invalid,
            custom_descriptor("Feature", AttributeValueType::Continuous, true),
            EncoderPlacement::new(EncoderGroup::Beam, 9, 1),
        );
        assert_eq!(
            invalid.validate(),
            Err(AttributeConfigurationError::InvalidCustomId(
                "Feature".into()
            ))
        );
    }

    #[test]
    fn placements_are_one_based_six_slot_unique_and_complete() {
        let mut invalid_slot = AttributeConfiguration::recommended();
        invalid_slot.placements[0].encoder.slot = ENCODER_SLOTS_PER_PAGE + 1;
        assert!(matches!(
            invalid_slot.validate(),
            Err(AttributeConfigurationError::InvalidPlacement(_))
        ));

        let mut occupied = AttributeConfiguration::recommended();
        occupied.placements[1].encoder = occupied.placements[0].encoder;
        assert!(matches!(
            occupied.validate(),
            Err(AttributeConfigurationError::OccupiedPlacement { .. })
        ));

        let mut duplicate = AttributeConfiguration::recommended();
        duplicate.placements.push(duplicate.placements[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(AttributeConfigurationError::DuplicatePlacement(_))
        ));

        let mut missing = AttributeConfiguration::recommended();
        missing.placements.pop();
        assert!(matches!(
            missing.validate(),
            Err(AttributeConfigurationError::MissingPlacement(_))
        ));
    }

    #[test]
    fn activation_groups_are_exclusive_recordable_and_within_one_encoder_group() {
        let mut overlap = AttributeConfiguration::recommended();
        overlap.activation_groups.push(AttributeActivationGroup {
            id: "second.intensity".into(),
            label: "Second Intensity".into(),
            members: vec![AttributeKey("intensity".into())],
        });
        assert_eq!(
            overlap.validate(),
            Err(AttributeConfigurationError::OverlappingActivationGroup(
                "intensity".into()
            ))
        );

        let mut cross_group = AttributeConfiguration::recommended();
        let position = cross_group
            .activation_groups
            .iter_mut()
            .find(|group| group.id == "position")
            .unwrap();
        position.members.push(AttributeKey("color.red".into()));
        assert!(matches!(
            cross_group.validate(),
            Err(AttributeConfigurationError::CrossEncoderActivationGroup { .. })
        ));

        let mut control = AttributeConfiguration::recommended();
        control.activation_groups.push(AttributeActivationGroup {
            id: "control".into(),
            label: "Control".into(),
            members: vec![AttributeKey("control".into())],
        });
        assert_eq!(
            control.validate(),
            Err(AttributeConfigurationError::IneligibleActivationMember(
                "control".into()
            ))
        );

        let mut missing = AttributeConfiguration::recommended();
        missing
            .activation_groups
            .retain(|group| !group.members.contains(&AttributeKey("intensity".into())));
        assert_eq!(
            missing.validate(),
            Err(AttributeConfigurationError::MissingActivationGroup(
                "intensity".into()
            ))
        );
    }

    #[test]
    fn non_recordable_custom_controls_are_placed_but_never_activated() {
        let mut configuration = AttributeConfiguration::recommended();
        add_custom(
            &mut configuration,
            custom_descriptor("vendor.reset", AttributeValueType::Control, false),
            EncoderPlacement::new(EncoderGroup::Control, 2, 2),
        );
        configuration.validate().unwrap();
        assert!(
            configuration
                .activation_group_for(&AttributeKey("vendor.reset".into()))
                .is_none()
        );

        configuration
            .activation_groups
            .push(AttributeActivationGroup {
                id: "vendor.reset".into(),
                label: "Reset".into(),
                members: vec![AttributeKey("vendor.reset".into())],
            });
        assert_eq!(
            configuration.validate(),
            Err(AttributeConfigurationError::IneligibleActivationMember(
                "vendor.reset".into()
            ))
        );
    }
}
