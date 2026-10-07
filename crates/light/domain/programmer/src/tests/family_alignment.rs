use super::*;
use light_core::{
    NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality, programming::*,
};
use uuid::Uuid;

fn desk(count: usize) -> (ProgrammerRegistry, SessionId, Vec<FixtureId>) {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixtures = (0..count).map(|_| FixtureId::new()).collect::<Vec<_>>();
    registry.start(session);
    registry.select(session, fixtures.iter().copied());
    registry
        .activate_alignment(session, ProgrammerAlignmentMode::Left)
        .unwrap();
    (registry, session, fixtures)
}
fn base(
    fixture_id: FixtureId,
    rank: usize,
    value: AttributeValue,
) -> ProgrammerFamilyAlignmentBase {
    ProgrammerFamilyAlignmentBase {
        fixture_id,
        rank,
        value,
        context: Arc::new(OwnedFamilyEditContext::default()),
    }
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn plan(
    registry: &ProgrammerRegistry,
    session: SessionId,
    component: ProgrammingComponent,
    delta: f64,
    initial: Option<(usize, Vec<ProgrammerFamilyAlignmentBase>)>,
) -> ProgrammerFamilyAlignmentPlan {
    registry
        .plan_family_alignment_delta(
            session,
            component,
            ProgrammerAlignmentLane::Normal,
            ProgrammerFamilyAlignmentTarget::Fixtures,
            FamilyAlignmentInput::Scalar(delta),
            initial.map(Into::into),
        )
        .unwrap()
}

#[test]
fn physical_align_preserves_turns_equal_spatial_ranks_and_value_neutral_reanchoring() {
    let (registry, session, fixtures) = desk(3);
    let initial = vec![
        base(fixtures[0], 0, angles(720.0, -90.0)),
        base(fixtures[1], 2, angles(-720.0, 80.0)),
        base(fixtures[2], 2, angles(1080.0, 50.0)),
    ];
    let first = plan(
        &registry,
        session,
        ProgrammingComponent::Pan,
        90.0,
        Some((3, initial)),
    );
    assert_eq!(
        first
            .values
            .iter()
            .map(|v| v.value.clone())
            .collect::<Vec<_>>(),
        vec![
            angles(720.0, -90.0),
            angles(-630.0, 80.0),
            angles(1170.0, 50.0)
        ]
    );
    let current = first
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| base(v.fixture_id, if i == 0 { 0 } else { 2 }, v.value.clone()))
        .collect::<Vec<_>>();
    registry
        .commit_family_alignment_plan(session, first)
        .unwrap();
    let generation = registry.normal_values_generation(session);
    let undo = registry.undo_depth(session);
    registry
        .reanchor_family_alignment(session, ProgrammerAlignmentMode::Right, current)
        .unwrap();
    assert_eq!(registry.normal_values_generation(session), generation);
    assert_eq!(registry.undo_depth(session), undo);
    let next = plan(&registry, session, ProgrammingComponent::Pan, -180.0, None);
    assert_eq!(
        next.values
            .iter()
            .map(|v| v.value.clone())
            .collect::<Vec<_>>(),
        vec![
            angles(540.0, -90.0),
            angles(-630.0, 80.0),
            angles(1170.0, 50.0)
        ]
    );
}

#[test]
fn target_takeover_only_happens_on_actual_movement_and_is_not_reversed_by_zero_input() {
    let (registry, session, fixtures) = desk(2);
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.0, 5.0, 2.0],
    )));
    let initial = fixtures
        .iter()
        .enumerate()
        .map(|(rank, fixture)| {
            let mut b = base(*fixture, rank, target.clone());
            b.context = Arc::new(OwnedFamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 710.0,
                    tilt_degrees: -20.0,
                }),
                ..Default::default()
            });
            b
        })
        .collect();
    let first = plan(
        &registry,
        session,
        ProgrammingComponent::Pan,
        90.0,
        Some((2, initial)),
    );
    assert!(first.values[0].preserves_target);
    assert_eq!(first.values[0].value, target);
    assert_eq!(first.values[1].value, angles(800.0, -20.0));
    registry
        .commit_family_alignment_plan(session, first)
        .unwrap();
    let reverse = plan(&registry, session, ProgrammingComponent::Pan, -90.0, None);
    assert!(reverse.values[0].preserves_target);
    assert_eq!(reverse.values[1].value, angles(710.0, -20.0));
    assert!(!reverse.values[1].preserves_target);
}

#[test]
fn metres_kelvin_and_duv_use_their_component_units_and_preserve_uv() {
    let (registry, session, fixtures) = desk(1);
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(19),
        },
        [-4.0, 2.0, 3.0],
    )));
    let result = plan(
        &registry,
        session,
        ProgrammingComponent::TargetX,
        -2.5,
        Some((1, vec![base(fixtures[0], 0, target)])),
    );
    assert_eq!(
        result.values[0].value,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: Uuid::from_u128(19)
            },
            [-6.5, 2.0, 3.0]
        )))
    );
    for (component, delta, kelvin, duv) in [
        (ColorComponent::Temperature, 500.0, 3700.0, 0.0),
        (ColorComponent::Duv, 0.001, 3200.0, 0.001),
    ] {
        let original = ColorIntent {
            white_target: WhiteTarget {
                kelvin: 3200.0,
                duv: 0.0,
            },
            uv: UvIntent { amount: 0.8 },
            ..Default::default()
        };
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: original.clone(),
        }));
        let result = plan(
            &registry,
            session,
            ProgrammingComponent::Color(component),
            delta,
            Some((1, vec![base(fixtures[0], 0, value)])),
        );
        let AttributeValue::ColorProgram(value) = &result.values[0].value else {
            panic!()
        };
        let ColorProgram::Semantic { intent } = value.as_ref() else {
            panic!()
        };
        assert_eq!(intent.white_target, WhiteTarget { kelvin, duv });
        assert_eq!(intent.uv, original.uv);
        assert_eq!(intent.base_xyz, original.base_xyz);
    }
}

struct NativeModel {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        Ok(portable())
    }
}
fn portable() -> PortableColorEstimate {
    PortableColorEstimate {
        model_revision: 1,
        visible: None,
        uv: None,
        quality: PhysicalDataQuality::Unknown,
        limitations: vec![],
    }
}
fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            profile_digest: "digest".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "layout".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::from_u128(5),
            function_id: Uuid::from_u128(6),
        },
    })
}
fn native_base(
    fixture: FixtureId,
    rank: usize,
    raw: u32,
    model: &Arc<NativeModel>,
) -> ProgrammerFamilyAlignmentBase {
    ProgrammerFamilyAlignmentBase {
        fixture_id: fixture,
        rank,
        value: AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: model.source.clone(),
                channels: vec![NativeColorValue {
                    channel_id: model.binding.channel_id,
                    function_id: model.binding.function_id,
                    raw,
                }],
                spreads: vec![],
            },
            portable: portable(),
        })),
        context: Arc::new(OwnedFamilyEditContext {
            native_model: Some(model.clone()),
            ..Default::default()
        }),
    }
}
fn raws(plan: &ProgrammerFamilyAlignmentPlan) -> Vec<u32> {
    plan.values
        .iter()
        .map(|v| {
            let AttributeValue::ColorProgram(p) = &v.value else {
                panic!()
            };
            let ColorProgram::Direct { recipe, .. } = p.as_ref() else {
                panic!()
            };
            recipe.channels[0].raw
        })
        .collect()
}

#[test]
fn native_align_accumulates_before_rounding_and_keeps_all_32_bits() {
    let (registry, session, fixtures) = desk(4);
    let model = native_model();
    let starts = [16_777_217, 16_777_217, u32::MAX - 4, u32::MAX - 4];
    let initial = fixtures
        .iter()
        .enumerate()
        .map(|(rank, f)| native_base(*f, rank, starts[rank], &model))
        .collect::<Vec<_>>();
    let component = ProgrammingComponent::NativeColor(model.binding);
    let combined = registry
        .plan_family_alignment_delta(
            session,
            component,
            ProgrammerAlignmentLane::Preload,
            ProgrammerFamilyAlignmentTarget::Fixtures,
            FamilyAlignmentInput::Native(3),
            Some((4, initial.clone()).into()),
        )
        .unwrap();
    let mut first = Some((4, initial));
    let mut latest = vec![];
    for _ in 0..3 {
        let step = registry
            .plan_family_alignment_delta(
                session,
                component,
                ProgrammerAlignmentLane::Preload,
                ProgrammerFamilyAlignmentTarget::Fixtures,
                FamilyAlignmentInput::Native(1),
                first.take().map(Into::into),
            )
            .unwrap();
        latest = raws(&step);
        registry
            .commit_family_alignment_plan(session, step)
            .unwrap();
    }
    assert_eq!(latest, raws(&combined));
    assert_eq!(
        latest,
        vec![16_777_217, 16_777_218, u32::MAX - 2, u32::MAX - 1]
    );
    let saturated = registry
        .plan_family_alignment_delta(
            session,
            component,
            ProgrammerAlignmentLane::Preload,
            ProgrammerFamilyAlignmentTarget::Fixtures,
            FamilyAlignmentInput::Native(i128::from(u32::MAX)),
            None,
        )
        .unwrap();
    assert_eq!(raws(&saturated)[3], u32::MAX);
    let before = registry.alignment(session);
    assert!(
        registry
            .plan_family_alignment_delta(
                session,
                component,
                ProgrammerAlignmentLane::Preload,
                ProgrammerFamilyAlignmentTarget::Fixtures,
                FamilyAlignmentInput::Native(i128::MAX),
                None
            )
            .is_err()
    );
    assert_eq!(registry.alignment(session), before);
}

#[test]
fn typed_align_contexts_are_transactional_runtime_only_and_released_by_off() {
    let (registry, session, fixtures) = desk(1);
    let model = native_model();
    let weak = Arc::downgrade(&model);
    let component = ProgrammingComponent::NativeColor(model.binding);
    let initial = vec![native_base(fixtures[0], 0, u32::MAX - 5, &model)];
    let plan = registry
        .plan_family_alignment_delta(
            session,
            component,
            ProgrammerAlignmentLane::Normal,
            ProgrammerFamilyAlignmentTarget::Fixtures,
            FamilyAlignmentInput::Native(1),
            Some((1, initial).into()),
        )
        .unwrap();
    registry
        .commit_family_alignment_plan(session, plan)
        .unwrap();
    drop(model);
    assert!(weak.upgrade().is_some());
    let before = registry.alignment(session);
    let result = registry.with_transaction(session, || {
        registry.deactivate_alignment(session);
        Err::<(), _>("rollback")
    });
    assert_eq!(result, Err("rollback"));
    assert_eq!(registry.alignment(session), before);
    let restored = ProgrammerRegistry::default();
    restored.restore(registry.get(session).unwrap());
    assert!(restored.alignment(session).is_none());
    drop(before);
    registry.deactivate_alignment(session);
    assert!(weak.upgrade().is_none());
}

#[test]
fn fractions_match_the_established_shapes_and_invalid_plans_leave_align_unbound() {
    for count in 1..30 {
        for mode in [
            ProgrammerAlignmentMode::Left,
            ProgrammerAlignmentMode::Right,
            ProgrammerAlignmentMode::Out,
            ProgrammerAlignmentMode::In,
        ] {
            for rank in 0..count {
                let (a, b) = programmer_alignment_fraction(mode, rank, count).unwrap();
                assert!(
                    ((a as f32 / b as f32)
                        - programmer_alignment_weight(mode, rank, count).unwrap())
                    .abs()
                        < 0.000001
                );
            }
        }
    }
    let (registry, session, fixtures) = desk(1);
    let before = registry.alignment(session);
    let invalid = base(
        fixtures[0],
        0,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [0.0; 3],
        ))),
    );
    assert!(
        registry
            .plan_family_alignment_delta(
                session,
                ProgrammingComponent::Pan,
                ProgrammerAlignmentLane::Normal,
                ProgrammerFamilyAlignmentTarget::Fixtures,
                FamilyAlignmentInput::Scalar(1.0),
                Some((1, vec![invalid]).into())
            )
            .is_err()
    );
    assert_eq!(registry.alignment(session), before);
    assert_eq!(registry.undo_depth(session), Some(1));
}
