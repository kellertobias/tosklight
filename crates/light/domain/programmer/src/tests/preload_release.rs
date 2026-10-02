use super::*;
use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality, programming::*};
use light_dynamics::DynamicSemanticValue;
use uuid::Uuid;

fn semantic(uv: f32) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: uv },
            ..Default::default()
        },
    }))
}

fn direct() -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 3,
                profile_digest: "retained-profile".into(),
                mode_id: Uuid::from_u128(2),
                head_id: Uuid::from_u128(3),
                path_id: Uuid::from_u128(4),
                model_revision: 2,
                native_layout_signature: "rgb-wheel-uv".into(),
            },
            channels: vec![NativeColorValue {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 2,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.8,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Unknown,
            limitations: vec!["visible appearance unknown".into()],
        },
    }))
}

fn release(registry: &ProgrammerRegistry, session: SessionId, fixture: FixtureId) -> bool {
    registry.apply_release_values(
        session,
        &[ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: ProgrammingOwner::Color.key(),
        }],
        &[ReleaseProgrammerGroupValue {
            group_id: "front".into(),
            attribute: ProgrammingOwner::Color.key(),
        }],
    )
}

#[test]
fn pending_release_retains_complete_sources_and_stamps_without_recording_them() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let color = ProgrammingOwner::Color.key();
    registry.start(session);
    registry.arm_preload(session, true);
    registry.set_faded_with_timing(
        session,
        fixture,
        color.clone(),
        direct(),
        Some(300),
        Some(50),
    );
    registry.apply_dynamic_values(
        session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: color.clone(),
            value: DynamicSemanticValue::Static {
                value: semantic(0.7),
                timing: Default::default(),
            },
        }],
        None,
    );
    let assignment = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
        owner: ProgrammingOwner::Color,
        template: semantic(0.3),
        members: [(fixture.0, direct())].into(),
    }));
    registry.set_group(session, "front".into(), color.clone(), assignment);
    let before = registry.get(session).unwrap();
    clock.advance_millis(20);
    assert!(release(&registry, session, fixture));
    let after = registry.get(session).unwrap();
    let retained = &after.preload_released_colors;
    assert_eq!(
        retained.fixtures[&fixture].value.as_ref(),
        Some(&before.preload_pending[0])
    );
    assert_eq!(
        retained.fixtures[&fixture].fixed.as_ref(),
        Some(&before.preload_dynamic_pending[0])
    );
    assert_eq!(
        retained.groups["front"],
        before.preload_group_pending["front"][&color]
    );
    assert_eq!(after.required_programming_contract(), 1);
    after.validate_programming().unwrap();
    assert!(!release(&registry, session, fixture));
    assert!(Arc::ptr_eq(
        retained,
        &registry.get(session).unwrap().preload_released_colors
    ));
    let encoded = serde_json::to_vec(&after).unwrap();
    let decoded: ProgrammerState = serde_json::from_slice(&encoded).unwrap();
    decoded.validate_programming().unwrap();
    assert_eq!(decoded.preload_released_colors, *retained);
    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert!(captured.fixture_values.is_empty());
    assert!(captured.group_values.is_empty());
    assert!(
        captured
            .dynamic_values
            .iter()
            .all(|v| matches!(v.value, DynamicSemanticValue::Release))
    );
    assert_eq!(captured.group_release_values.len(), 1);
    registry.undo(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
    registry.redo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_released_colors,
        *retained
    );
    registry.clear_normal_values(session);
    assert_eq!(
        registry.get(session).unwrap().preload_released_colors,
        *retained
    );
    registry.activate_preload(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
    registry.undo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_released_colors,
        *retained
    );
    registry.clear_preload_pending(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
}

#[test]
fn off_and_replacement_clear_pending_release_provenance_but_linked_dynamic_off_does_not() {
    for operation in 0..7 {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        let color = ProgrammingOwner::Color.key();
        registry.start(session);
        registry.arm_preload(session, true);
        registry.set_many(session, [(fixture, color.clone(), direct())]);
        registry.set_group(session, "front".into(), color.clone(), semantic(0.4));
        release(&registry, session, fixture);
        // Removing a particular Dynamic instance is independent of the whole-family Release.
        registry.apply_dynamic_values(
            session,
            &[DynamicProgrammerValueMutation::Release {
                fixture_id: fixture,
                attribute: color.clone(),
                instance_link: Some(Uuid::new_v4()),
            }],
            None,
        );
        assert!(
            !registry
                .get(session)
                .unwrap()
                .preload_released_colors
                .is_empty()
        );
        match operation {
            0 => {
                registry.release_fixture_attribute(session, fixture, &color);
                registry.release_group_attribute(session, "front", &color);
            }
            1 => {
                registry.set_many(session, [(fixture, color.clone(), semantic(0.2))]);
                registry.set_preload_group(session, "front".into(), color.clone(), semantic(0.3));
            }
            2 => {
                registry.apply_preload_values(
                    session,
                    &[
                        PreloadProgrammerValueMutation::ReleaseFixture {
                            fixture_id: fixture,
                            attribute: color.clone(),
                        },
                        PreloadProgrammerValueMutation::ReleaseGroup {
                            group_id: "front".into(),
                            attribute: color.clone(),
                        },
                    ],
                );
            }
            3 => {
                registry.apply_dynamic_values(
                    session,
                    &[DynamicProgrammerValueMutation::Release {
                        fixture_id: fixture,
                        attribute: color.clone(),
                        instance_link: None,
                    }],
                    None,
                );
                registry.release_group_attribute(session, "front", &color);
            }
            4 => {
                registry.release_preload(session);
            }
            5 => {
                registry.apply_preload_values(
                    session,
                    &[
                        PreloadProgrammerValueMutation::SetFixture {
                            fixture_id: fixture,
                            attribute: color.clone(),
                            value: semantic(0.2),
                            timing: Default::default(),
                        },
                        PreloadProgrammerValueMutation::SetGroup {
                            group_id: "front".into(),
                            attribute: color.clone(),
                            value: semantic(0.3),
                            timing: Default::default(),
                        },
                    ],
                );
            }
            _ => {
                registry.set_faded(session, fixture, color.clone(), semantic(0.2));
                registry.set_group(session, "front".into(), color, semantic(0.3));
            }
        }
        let after = registry.get(session).unwrap();
        assert!(
            after.preload_released_colors.is_empty(),
            "operation {operation}"
        );
        after.validate_programming().unwrap();
    }
}

#[test]
fn retained_release_validation_rejects_orphans_wrong_owners_and_hidden_contracts() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.arm_preload(session, true);
    registry.set_many(
        session,
        [(fixture, ProgrammingOwner::Color.key(), direct())],
    );
    release(&registry, session, fixture);
    let saved = registry.get(session).unwrap();
    let mut orphan = saved.clone();
    orphan.preload_dynamic_pending = Arc::default();
    assert!(orphan.validate_programming().is_err());
    let mut wrong = saved.clone();
    Arc::make_mut(&mut wrong.preload_released_colors)
        .fixtures
        .get_mut(&fixture)
        .unwrap()
        .value
        .as_mut()
        .unwrap()
        .attribute = ProgrammingOwner::Position.key();
    assert!(wrong.validate_programming().is_err());
    let mut newer = saved.clone();
    Arc::make_mut(&mut newer.preload_released_colors)
        .fixtures
        .get_mut(&fixture)
        .unwrap()
        .value
        .as_mut()
        .unwrap()
        .programmer_order = u64::MAX;
    assert!(newer.validate_programming().is_err());
    let mut no_history = saved.clone();
    no_history.undo.clear();
    no_history.redo.clear();
    assert_eq!(
        no_history.required_programming_contract(),
        1,
        "retained recipe alone requires contract support"
    );
    let mut legacy = serde_json::to_value(saved).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("preload_released_colors");
    assert!(
        serde_json::from_value::<ProgrammerState>(legacy)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
}

#[test]
fn fixed_track_release_only_retains_the_fixed_candidate_it_removed() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let color = ProgrammingOwner::Color.key();
    registry.start(session);
    registry.arm_preload(session, true);
    registry.set_many(session, [(fixture, color.clone(), semantic(0.5))]);
    let set = |value| DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: color.clone(),
        value,
    };
    registry.apply_dynamic_values(
        session,
        &[set(DynamicSemanticValue::Static {
            value: direct(),
            timing: Default::default(),
        })],
        None,
    );
    registry.apply_dynamic_values(session, &[set(DynamicSemanticValue::Release)], None);
    let state = registry.get(session).unwrap();
    assert_eq!(state.preload_pending.len(), 1);
    assert!(
        state.preload_released_colors.fixtures[&fixture]
            .value
            .is_none()
    );
    assert!(
        state.preload_released_colors.fixtures[&fixture]
            .fixed
            .is_some()
    );
    state.validate_programming().unwrap();
}

#[test]
fn replacement_preload_go_records_the_visible_set_without_an_obsolete_release() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let color = ProgrammingOwner::Color.key();
    registry.start(session);
    registry.arm_preload(session, true);
    release(&registry, session, fixture);
    registry.activate_preload(session);
    registry.arm_preload(session, true);
    registry.set_many(session, [(fixture, color.clone(), direct())]);
    registry.set_group(session, "front".into(), color, semantic(0.4));
    registry.activate_preload(session);
    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_eq!(captured.fixture_values[0].value, direct());
    assert_eq!(captured.group_values[0].value, semantic(0.4));
    assert!(captured.dynamic_values.is_empty());
    assert!(captured.group_release_values.is_empty());
    registry.undo(session);
    assert_eq!(
        registry
            .get(session)
            .unwrap()
            .preload_group_release_active
            .len(),
        1
    );
    registry.redo(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_group_release_active
            .is_empty()
    );
}

fn color_hold(
    component: Option<ProgrammingComponent>,
    family: AttributeValue,
) -> DynamicSemanticValue {
    let mut address =
        light_dynamics::DynamicValueAddress::whole_family(ProgrammingOwner::Color, &family)
            .unwrap();
    address.component = component;
    DynamicSemanticValue::ProgrammingFixAt {
        mask: light_dynamics::ProgrammingFamilyFixAt { address, family },
        timing: Default::default(),
    }
}

fn color_mutation(
    fixture: FixtureId,
    value: DynamicSemanticValue,
) -> DynamicProgrammerValueMutation {
    DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: ProgrammingOwner::Color.key(),
        value,
    }
}

#[test]
fn component_releases_retain_complete_recipes_independently_through_capture_and_undo() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let uv = Some(ProgrammingComponent::Color(ColorComponent::Uv));
    let native = Some(ProgrammingComponent::NativeColor(
        light_core::NativeColorBinding {
            channel_id: Uuid::from_u128(5),
            function_id: Uuid::from_u128(6),
        },
    ));
    registry.start(session);
    registry.arm_preload(session, true);
    registry.set_many(
        session,
        [(fixture, ProgrammingOwner::Color.key(), semantic(0.1))],
    );
    registry.apply_dynamic_values(
        session,
        &[
            color_mutation(fixture, color_hold(uv, semantic(0.8))),
            color_mutation(fixture, color_hold(native, direct())),
        ],
        None,
    );
    let before = registry.get(session).unwrap();
    registry.apply_dynamic_values(
        session,
        &[
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: uv },
            ),
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: native },
            ),
        ],
        None,
    );
    let released = registry.get(session).unwrap();
    released.validate_programming().unwrap();
    let retained = &released.preload_released_colors;
    let candidates = &retained.fixtures[&fixture];
    assert!(candidates.value.is_none());
    assert!(candidates.fixed.is_none());
    assert_eq!(candidates.fixed_components.len(), 2);
    for original in before.preload_dynamic_pending.iter() {
        assert!(
            candidates.fixed_components.contains(original),
            "complete family, exact recipe and edit stamp must survive"
        );
    }
    assert_eq!(released.preload_pending, before.preload_pending);
    let restored: ProgrammerState =
        serde_json::from_value(serde_json::to_value(&released).unwrap()).unwrap();
    restored.validate_programming().unwrap();
    assert_eq!(restored.preload_released_colors, *retained);
    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_eq!(captured.fixture_values.len(), 1);
    assert_eq!(captured.dynamic_values.len(), 2);
    assert!(
        captured
            .dynamic_values
            .iter()
            .all(|value| matches!(value.value, DynamicSemanticValue::ProgrammingRelease { .. }))
    );
    assert!(!registry.apply_dynamic_values(
        session,
        &[
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: uv }
            ),
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: native }
            ),
        ],
        None
    ));
    assert!(Arc::ptr_eq(
        retained,
        &registry.get(session).unwrap().preload_released_colors
    ));
    registry.undo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_dynamic_pending,
        before.preload_dynamic_pending
    );
    registry.redo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_released_colors,
        *retained
    );
    registry.activate_preload(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
    registry.undo(session);
    assert_eq!(
        registry.get(session).unwrap().preload_released_colors,
        *retained
    );
    registry.release_preload(session);
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
}

#[test]
fn replacing_one_component_keeps_sibling_release_provenance_and_owner_release_collects_both() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let uv = Some(ProgrammingComponent::Color(ColorComponent::Uv));
    let white = Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend));
    registry.start(session);
    registry.arm_preload(session, true);
    registry.apply_dynamic_values(
        session,
        &[
            color_mutation(fixture, color_hold(uv, semantic(0.8))),
            color_mutation(fixture, color_hold(white, semantic(0.6))),
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: uv },
            ),
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: white },
            ),
        ],
        None,
    );
    let original = registry
        .get(session)
        .unwrap()
        .preload_released_colors
        .clone();
    registry.apply_dynamic_values(
        session,
        &[color_mutation(fixture, color_hold(uv, semantic(0.3)))],
        None,
    );
    let retained = &registry
        .get(session)
        .unwrap()
        .preload_released_colors
        .fixtures[&fixture]
        .fixed_components;
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].value.track_key().component, white);
    assert!(
        original.fixtures[&fixture]
            .fixed_components
            .contains(&retained[0])
    );
    // Editing the static base does not put a released hold back or erase its sibling's provenance.
    registry.set_many(
        session,
        [(fixture, ProgrammingOwner::Color.key(), direct())],
    );
    assert_eq!(
        &registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .fixtures[&fixture]
            .fixed_components,
        retained
    );
    registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: ProgrammingOwner::Color.key(),
            value: semantic(0.15),
            timing: Default::default(),
        }],
    );
    assert_eq!(
        &registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .fixtures[&fixture]
            .fixed_components,
        retained
    );
    registry.set_many(
        session,
        [(fixture, ProgrammingOwner::Color.key(), direct())],
    );
    assert!(release(&registry, session, fixture));
    let state = registry.get(session).unwrap();
    state.validate_programming().unwrap();
    let retained = &state.preload_released_colors.fixtures[&fixture];
    assert_eq!(retained.value.as_ref().unwrap().value, direct());
    assert_eq!(retained.fixed_components.len(), 2);
    assert!(
        retained
            .fixed_components
            .iter()
            .any(|v| v.value == color_hold(uv, semantic(0.3)))
    );
    assert_eq!(state.preload_dynamic_pending.len(), 1);
    assert!(matches!(
        state.preload_dynamic_pending[0].value,
        DynamicSemanticValue::Release
    ));
    // A static replacement removes the owner Release and all its retained candidates.
    registry.set_many(
        session,
        [(fixture, ProgrammingOwner::Color.key(), semantic(0.2))],
    );
    assert!(
        registry
            .get(session)
            .unwrap()
            .preload_released_colors
            .is_empty()
    );
}

#[test]
fn retained_component_validation_rejects_wrong_release_scope_duplicates_and_future_stamps() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let uv = Some(ProgrammingComponent::Color(ColorComponent::Uv));
    registry.start(session);
    registry.arm_preload(session, true);
    registry.apply_dynamic_values(
        session,
        &[
            color_mutation(fixture, color_hold(uv, semantic(0.8))),
            color_mutation(
                fixture,
                DynamicSemanticValue::ProgrammingRelease { component: uv },
            ),
        ],
        None,
    );
    let state = registry.get(session).unwrap();
    state.validate_programming().unwrap();
    for invalid in 0..5 {
        let mut damaged = state.clone();
        if invalid == 0 {
            Arc::make_mut(&mut damaged.preload_dynamic_pending)[0].value =
                DynamicSemanticValue::ProgrammingRelease {
                    component: Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend)),
                };
        } else {
            let candidate = Arc::make_mut(&mut damaged.preload_released_colors)
                .fixtures
                .get_mut(&fixture)
                .unwrap();
            match invalid {
                1 => candidate
                    .fixed_components
                    .push(candidate.fixed_components[0].clone()),
                2 => candidate.fixed_components[0].programmer_order = u64::MAX,
                3 => candidate.fixed_components[0].fixture_id = FixtureId::new(),
                _ => {
                    candidate.value = Some(light_core::TimedValue {
                        fixture_id: fixture,
                        attribute: ProgrammingOwner::Color.key(),
                        value: semantic(0.1),
                        changed_at: Utc::now() - chrono::Duration::seconds(1),
                        programmer_order: 0,
                        priority: 0,
                        merge_mode: light_core::MergeMode::Ltp,
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    })
                }
            }
        }
        assert!(
            damaged.validate_programming().is_err(),
            "invalid case {invalid}"
        );
    }
}

#[test]
fn retained_component_uses_edit_order_when_wall_clock_moves_backwards() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let uv = Some(ProgrammingComponent::Color(ColorComponent::Uv));
    registry.start(session);
    registry.arm_preload(session, true);
    registry.apply_dynamic_values(
        session,
        &[color_mutation(fixture, DynamicSemanticValue::Release)],
        None,
    );
    clock.advance_millis(-1000);
    registry.apply_dynamic_values(
        session,
        &[color_mutation(fixture, color_hold(uv, semantic(0.4)))],
        None,
    );
    registry.apply_dynamic_values(
        session,
        &[color_mutation(
            fixture,
            DynamicSemanticValue::ProgrammingRelease { component: uv },
        )],
        None,
    );
    let state = registry.get(session).unwrap();
    assert_eq!(state.preload_dynamic_pending.len(), 2);
    assert_eq!(
        state.preload_released_colors.fixtures[&fixture]
            .fixed_components
            .len(),
        1
    );
    state.validate_programming().unwrap();
}
