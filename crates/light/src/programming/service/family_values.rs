//! One shared planner for Normal/Blind and pending Preload. The complete transaction is planned
//! before mutation, so a missing adoption/model cannot leave some selected lamps edited.
use crate::{
    ActionError, ActionErrorKind, ProgrammingFamilyContext, ProgrammingValueIntent,
    ProgrammingValueMutation, ProgrammingValuesEnvironment,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, programming::*};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

type FixtureValues = HashMap<(FixtureId, AttributeKey), AttributeValue>;
type GroupValues = HashMap<(String, AttributeKey), AttributeValue>;

pub(super) fn plan_component_edits(
    intent: &ProgrammingValueIntent,
    edits: &[ComponentEdit],
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    validate_component_edits(edits).map_err(invalid)?;
    if !position_takeover_available(intent, edits, environment, active, groups) {
        return Ok(vec![]);
    }
    if edits.is_empty() {
        return Ok(vec![]);
    }
    if let Some(group_id) = &intent.group_id {
        return plan_group(intent, edits, environment, active, groups, group_id);
    }
    let count = intent.fixture_ids.len();
    let ranked = rank_edits(edits, count)?;
    let mut result = Vec::new();
    for (rank, fixture) in intent.fixture_ids.iter().enumerate() {
        let address = (*fixture, intent.attribute.clone());
        let seed = active
            .get(&address)
            .or_else(|| environment.current_values.get(&address))
            .or_else(|| environment.default_values.get(&address))
            .ok_or_else(|| {
                invalid(IntentError(
                    "component edit requires a complete current or default family".into(),
                ))
            })?;
        let fallback = ProgrammingFamilyContext::default();
        let context = environment
            .family_contexts
            .get(fixture)
            .unwrap_or(&fallback);
        for edit in edits {
            if let ComponentEdit::Native {
                binding,
                operation: NativeColorEdit::Spread(points),
            } = edit
            {
                let descriptor = context
                    .native_model
                    .as_ref()
                    .and_then(|model| model.descriptor(*binding))
                    .ok_or_else(|| {
                        invalid(IntentError(
                            "native spread requires a verified continuous source function".into(),
                        ))
                    })?;
                NativeColorSpread {
                    binding: *binding,
                    points: points.clone(),
                }
                .validate(descriptor)
                .map_err(invalid)?;
            }
        }
        // TL-554: a native edit starts from the captured Direct seed unless the target is
        // already Direct of the pinned source (edited in place, never reseeded).
        let base = context.edit_base(seed, edits);
        let value = edit_family(base, &ranked[rank], &context.borrowed()).map_err(invalid)?;
        if preserves_target(seed, &value, edits) {
            continue;
        }
        append_component_releases(&mut result, *fixture, edits[0].owner(), active);
        result.push(ProgrammingValueMutation::SetFixture {
            fixture_id: *fixture,
            attribute: intent.attribute.clone(),
            value,
            timing: intent.timing,
        });
    }
    Ok(result)
}

/// Expected missing command-pose adoption is a passive hold of the whole selection, never
/// a partial edit, invented pose or user-facing error. Malformed intents still validate first.
/// Existing Angles remain editable even when the destination has no calibrated model.
pub(super) fn position_takeover_available(
    intent: &ProgrammingValueIntent,
    edits: &[ComponentEdit],
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> bool {
    let angles = edits.iter().any(|edit| {
        matches!(
            edit,
            ComponentEdit::ActivateAngles
                | ComponentEdit::Scalar {
                    component: ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
                    ..
                }
        )
    });
    if !angles {
        return true;
    }
    position_members_available(intent, environment, active, groups, true)
}

/// Group Align adopts a complete Group seed before selecting its active bases. Expected
/// missing physical seeds anywhere in that seed must hold before adoption can raise an error.
pub(super) fn position_missing_seed_available(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> bool {
    position_members_available(intent, environment, active, groups, false)
}

fn position_members_available(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
    require_target_pose: bool,
) -> bool {
    let group = intent.group_id.as_ref();
    let existing = group.and_then(|id| groups.get(&(id.clone(), intent.attribute.clone())));
    let members = group.map_or(intent.fixture_ids.as_slice(), |id| {
        environment
            .group_members
            .get(id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    });
    members.iter().all(|fixture| {
        let address = (*fixture, intent.attribute.clone());
        let seed = match existing {
            Some(AttributeValue::GroupFamily(assignment)) => Some(assignment.for_member(*fixture)),
            Some(value) => Some(value),
            None => active
                .get(&address)
                .or_else(|| environment.current_values.get(&address))
                .or_else(|| environment.default_values.get(&address))
                .or_else(|| {
                    group.and_then(|id| {
                        environment
                            .group_family_templates
                            .get(&(id.clone(), intent.attribute.clone()))
                    })
                }),
        };
        let context = environment
            .family_contexts
            .get(fixture)
            .or_else(|| group.and_then(|id| environment.group_family_contexts.get(id)));
        match seed {
            Some(AttributeValue::Position(position))
                if require_target_pose
                    && matches!(position.as_ref(), PositionIntent::Target { .. }) =>
            {
                context.and_then(|context| context.solved_angles).is_some()
            }
            // Only a real attempted capture makes absence passive. Other incomplete/invalid
            // environments retain their existing typed validation errors.
            None => !context.is_some_and(|context| {
                context.position_adoption_attempted && context.solved_angles.is_none()
            }),
            _ => true,
        }
    })
}

pub(super) fn adopt_group_family(
    intent: &ProgrammingValueIntent,
    edits: &[ComponentEdit],
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
    group_id: &str,
) -> Result<GroupFamilyAssignment, ActionError> {
    let owner = edits[0].owner();
    let address = (group_id.to_owned(), intent.attribute.clone());
    let existing = groups.get(&address);
    let members = environment
        .group_members
        .get(group_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let explicit_template = environment.group_family_templates.get(&address);
    let mut seeds = BTreeMap::new();
    for fixture in members {
        let address = (*fixture, intent.attribute.clone());
        let seed = match existing {
            Some(AttributeValue::GroupFamily(assignment)) => Some(assignment.for_member(*fixture)),
            Some(value) => Some(value),
            None => active
                .get(&address)
                .or_else(|| environment.current_values.get(&address))
                .or_else(|| environment.default_values.get(&address))
                .or(explicit_template),
        }
        .ok_or_else(|| {
            invalid(IntentError(
                "Group component edit requires complete member adoption".into(),
            ))
        })?;
        seeds.insert(fixture.0, seed.clone());
    }
    // Equality is checked across the entire membership. A shared value is a common adoption,
    // never an arbitrary first-lamp approximation of heterogeneous members.
    let common = seeds
        .values()
        .next()
        .filter(|first| seeds.values().all(|value| value == *first));
    let declared_default = match owner {
        ProgrammingOwner::Color => Some(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Semantic {
                intent: ColorIntent::default(),
            },
        ))),
        ProgrammingOwner::Position => Some(AttributeValue::Position(Arc::new(
            if edits.iter().any(|edit| {
                matches!(
                    edit,
                    ComponentEdit::Target { .. }
                        | ComponentEdit::Scalar {
                            component: ProgrammingComponent::TargetX
                                | ProgrammingComponent::TargetY
                                | ProgrammingComponent::TargetZ,
                            ..
                        }
                )
            }) {
                PositionIntent::target(TargetReference::Origin, [0.0; 3])
            } else {
                PositionIntent::angles(0.0, 0.0)
            },
        ))),
        ProgrammingOwner::Focus => Some(AttributeValue::Normalized(0.0)),
        ProgrammingOwner::Zoom => None,
    };
    let template_seed = match existing {
        Some(AttributeValue::GroupFamily(assignment)) => Some(&assignment.template),
        Some(value) => Some(value),
        None => explicit_template.or(common).or(declared_default.as_ref()),
    }
    .ok_or_else(|| {
        invalid(IntentError(
            "Group family needs an explicit complete template".into(),
        ))
    })?;
    let template = template_seed.clone();
    let mut retained = match existing {
        Some(AttributeValue::GroupFamily(assignment)) => assignment.members.clone(),
        _ => BTreeMap::new(),
    };
    retained.extend(seeds);
    Ok(GroupFamilyAssignment {
        owner,
        template,
        members: retained,
    })
}

fn plan_group(
    intent: &ProgrammingValueIntent,
    edits: &[ComponentEdit],
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
    group_id: &str,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    let seed = adopt_group_family(intent, edits, environment, active, groups, group_id)?;
    let owner = seed.owner;
    let template_seed = &seed.template;
    let members = environment
        .group_members
        .get(group_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let fallback = ProgrammingFamilyContext::default();
    let group_context = environment
        .group_family_contexts
        .get(group_id)
        .unwrap_or(&fallback);
    // A removed member contributes nothing, but its retained exception remains part of the
    // stored Group. Partial edits apply only to current members; whole-family replacement clears it.
    let mut edited = seed.members.clone();
    let mut every_member_preserves_target = !members.is_empty();
    for fixture in members {
        let member_seed = seed.for_member(*fixture);
        let context = environment
            .family_contexts
            .get(fixture)
            .unwrap_or(group_context);
        let base = context.edit_base(member_seed, edits);
        let value = edit_family(base, edits, &context.borrowed()).map_err(invalid)?;
        every_member_preserves_target &= preserves_target(member_seed, &value, edits);
        edited.insert(fixture.0, value);
    }
    if every_member_preserves_target {
        return Ok(vec![]);
    }
    // A template has no mounting pose. When taking a Target template into Angles, its future
    // members start from the declared zero joint pose; current members retain their own captured
    // solved joints above. An explicitly supplied common pose takes precedence.
    let template_angles;
    let template_seed = if group_context.solved_angles.is_none()
        && matches!(template_seed, AttributeValue::Position(position) if matches!(position.as_ref(), PositionIntent::Target { .. }))
        && edits.iter().any(|edit| {
            matches!(
                edit,
                ComponentEdit::ActivateAngles
                    | ComponentEdit::Scalar {
                        component: ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
                        ..
                    }
            )
        }) {
        template_angles = AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0)));
        &template_angles
    } else {
        template_seed
    };
    let template = edit_family(
        group_context.edit_base(template_seed, edits),
        edits,
        &group_context.borrowed(),
    )
    .map_err(invalid)?;
    let mut assignment = GroupFamilyAssignment {
        owner,
        template,
        members: edited,
    };
    assignment.remove_redundant_exceptions();
    assignment.validate().map_err(invalid)?;
    let value = if assignment.members.is_empty() {
        assignment.template
    } else {
        AttributeValue::GroupFamily(Arc::new(assignment))
    };
    let mut result = groups
        .keys()
        .filter(|(group, attribute)| {
            group == group_id && independent_programming_component(attribute, owner)
        })
        .map(|(_, attribute)| ProgrammingValueMutation::ReleaseGroup {
            group_id: group_id.to_owned(),
            attribute: attribute.clone(),
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| release_key(a).cmp(release_key(b)));
    result.push(ProgrammingValueMutation::SetGroup {
        group_id: group_id.to_owned(),
        attribute: intent.attribute.clone(),
        value,
        timing: intent.timing,
    });
    Ok(result)
}

fn preserves_target(
    seed: &AttributeValue,
    value: &AttributeValue,
    edits: &[ComponentEdit],
) -> bool {
    seed == value
        && matches!(seed, AttributeValue::Position(position) if matches!(position.as_ref(), PositionIntent::Target { .. }))
        && edits.iter().any(|edit| {
            matches!(
                edit,
                ComponentEdit::Scalar {
                    component: ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
                    ..
                }
            )
        })
        && !edits
            .iter()
            .any(|edit| matches!(edit, ComponentEdit::ActivateAngles))
}

pub(super) fn append_component_releases(
    result: &mut Vec<ProgrammingValueMutation>,
    fixture: FixtureId,
    owner: ProgrammingOwner,
    active: &FixtureValues,
) {
    let mut attributes = active
        .keys()
        .filter(|(id, attribute)| {
            *id == fixture && independent_programming_component(attribute, owner)
        })
        .map(|(_, attribute)| attribute.clone())
        .collect::<Vec<_>>();
    attributes.sort();
    result.extend(attributes.into_iter().map(|attribute| {
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id: fixture,
            attribute,
        }
    }));
}
fn release_key(value: &ProgrammingValueMutation) -> &str {
    match value {
        ProgrammingValueMutation::ReleaseGroup { attribute, .. } => &attribute.0,
        _ => unreachable!(),
    }
}

/// Compile edited curves once over the selection. Different complete seeds then receive the
/// same rank operation while retaining their own untouched components and source identities.
fn rank_edits(
    edits: &[ComponentEdit],
    count: usize,
) -> Result<Vec<Vec<ComponentEdit>>, ActionError> {
    let mut ranks = vec![Vec::with_capacity(edits.len()); count];
    for edit in edits {
        match edit {
            ComponentEdit::Scalar {
                component,
                operation: ScalarEdit::Set(ScalarIntent::Spread(points)),
            } => {
                let circular =
                    component.descriptor().interpolation == ScalarInterpolation::ShortestArc;
                for (rank, value) in ranks
                    .iter_mut()
                    .zip(ScalarIntent::Spread(points.clone()).resolve(count, circular))
                {
                    rank.push(ComponentEdit::Scalar {
                        component: *component,
                        operation: ScalarEdit::Set(ScalarIntent::Value(value)),
                    });
                }
            }
            ComponentEdit::Native {
                binding,
                operation: NativeColorEdit::Spread(points),
            } => {
                let spread = NativeColorSpread {
                    binding: *binding,
                    points: points.clone(),
                };
                for (rank, value) in ranks
                    .iter_mut()
                    .zip(spread.resolve(count).map_err(invalid)?)
                {
                    rank.push(ComponentEdit::Native {
                        binding: *binding,
                        operation: NativeColorEdit::Set(value),
                    });
                }
            }
            _ => {
                for rank in &mut ranks {
                    rank.push(edit.clone());
                }
            }
        }
    }
    Ok(ranks)
}
fn invalid(error: IntentError) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProgrammingValueOperation, ProgrammingValueTiming};
    fn request(owner: ProgrammingOwner, edits: Vec<ComponentEdit>) -> ProgrammingValueIntent {
        ProgrammingValueIntent {
            fixture_ids: vec![],
            group_id: Some("1".into()),
            attribute: owner.key(),
            operation: ProgrammingValueOperation::ComponentEdits(edits),
            undo_group: Some("gesture".into()),
            timing: ProgrammingValueTiming::default(),
            displayed_source: None,
            color_adoption: Default::default(),
        }
    }
    fn run(
        intent: &ProgrammingValueIntent,
        environment: &ProgrammingValuesEnvironment,
        groups: &GroupValues,
    ) -> AttributeValue {
        let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
            unreachable!()
        };
        let result =
            plan_component_edits(intent, edits, environment, &HashMap::new(), groups).unwrap();
        assert_eq!(
            result.len(),
            1,
            "one Group write, no fixture ownership leaks"
        );
        let ProgrammingValueMutation::SetGroup { value, .. } = &result[0] else {
            panic!("Group write expected")
        };
        value.clone()
    }
    fn semantic(rgb: [f32; 3], uv: f32) -> AttributeValue {
        let mut intent = ColorIntent::default();
        intent.recipe.rgb = rgb;
        intent.base_xyz = VirtualColorAuthoringV1::recipe_xyz(&intent.recipe).unwrap();
        intent.uv.amount = uv;
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
    }
    #[test]
    fn heterogeneous_group_edit_preserves_members_and_does_not_resample_output() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let mut environment = ProgrammingValuesEnvironment::default();
        environment.group_members.insert("1".into(), vec![a, b]);
        let initial_a = semantic([1.0, 0.0, 1.0], 0.2);
        let initial_b = semantic([1.0, 0.4, 0.1], 0.8);
        environment
            .current_values
            .insert((a, ProgrammingOwner::Color.key()), initial_a.clone());
        environment
            .current_values
            .insert((b, ProgrammingOwner::Color.key()), initial_b.clone());
        let intent = request(
            ProgrammingOwner::Color,
            vec![ComponentEdit::Scalar {
                component: ProgrammingComponent::Color(ColorComponent::WhiteBlend),
                operation: ScalarEdit::Set(ScalarIntent::Value(0.5)),
            }],
        );
        let value = run(&intent, &environment, &HashMap::new());
        let AttributeValue::GroupFamily(assignment) = &value else {
            panic!("heterogeneous assignment expected")
        };
        for (id, original) in [(a, initial_a), (b, initial_b)] {
            let AttributeValue::ColorProgram(original) = original else {
                unreachable!()
            };
            let ColorProgram::Semantic { intent: original } = original.as_ref() else {
                unreachable!()
            };
            let AttributeValue::ColorProgram(edited) = assignment.for_member(id) else {
                unreachable!()
            };
            let ColorProgram::Semantic { intent: edited } = edited.as_ref() else {
                unreachable!()
            };
            assert_eq!(edited.base_xyz, original.base_xyz);
            assert_eq!(edited.uv, original.uv);
            assert_eq!(edited.white_blend, 0.5);
        }
        environment.current_values.insert(
            (a, ProgrammingOwner::Color.key()),
            semantic([0.0, 1.0, 0.0], 0.0),
        );
        assert_eq!(
            run(
                &intent,
                &environment,
                &HashMap::from([(("1".into(), ProgrammingOwner::Color.key()), value.clone())])
            ),
            value
        );
    }
    #[test]
    fn target_takeover_captures_each_mount_and_keeps_zero_reset_value_neutral() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let mut environment = ProgrammingValuesEnvironment::default();
        environment.group_members.insert("1".into(), vec![a, b]);
        for (id, pan) in [(a, -450.0), (b, 720.0)] {
            environment.current_values.insert(
                (id, ProgrammingOwner::Position.key()),
                AttributeValue::Position(Arc::new(PositionIntent::target(
                    TargetReference::Point {
                        point_id: uuid::Uuid::new_v4(),
                    },
                    [1.0, 2.0, 3.0],
                ))),
            );
            environment.family_contexts.insert(
                id,
                ProgrammingFamilyContext {
                    solved_angles: Some(JointAngles {
                        pan_degrees: pan,
                        tilt_degrees: 30.0,
                    }),
                    ..Default::default()
                },
            );
        }
        let intent = request(
            ProgrammingOwner::Position,
            vec![ComponentEdit::Scalar {
                component: ProgrammingComponent::Pan,
                operation: ScalarEdit::Relative(90.0),
            }],
        );
        let value = run(&intent, &environment, &HashMap::new());
        let AttributeValue::GroupFamily(assignment) = value else {
            panic!("different joints expected")
        };
        assert_eq!(
            assignment.for_member(a),
            &AttributeValue::Position(Arc::new(PositionIntent::angles(-360.0, 30.0)))
        );
        assert_eq!(
            assignment.for_member(b),
            &AttributeValue::Position(Arc::new(PositionIntent::angles(810.0, 30.0)))
        );
        for context in environment.family_contexts.values_mut() {
            context.solved_angles.as_mut().unwrap().pan_degrees = 0.0;
        }
        let edits = vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Set(ScalarIntent::Value(0.0)),
        }];
        let intent = request(ProgrammingOwner::Position, edits.clone());
        assert!(
            plan_component_edits(
                &intent,
                &edits,
                &environment,
                &HashMap::new(),
                &HashMap::new()
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn different_target_references_adopt_offsets_and_keep_dormant_members() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let mut environment = ProgrammingValuesEnvironment::default();
        environment.group_members.insert("1".into(), vec![a, b]);
        let key = ProgrammingOwner::Position.key();
        let point_a = uuid::Uuid::new_v4();
        let point_b = uuid::Uuid::new_v4();
        for (id, point) in [(a, point_a), (b, point_b)] {
            environment.current_values.insert(
                (id, key.clone()),
                AttributeValue::Position(Arc::new(PositionIntent::target(
                    TargetReference::Point { point_id: point },
                    [1.0, 2.0, 3.0],
                ))),
            );
        }
        let intent = request(
            ProgrammingOwner::Position,
            vec![ComponentEdit::Scalar {
                component: ProgrammingComponent::TargetX,
                operation: ScalarEdit::Relative(1.0),
            }],
        );
        let value = run(&intent, &environment, &HashMap::new());
        let AttributeValue::GroupFamily(first) = &value else {
            panic!("different frames retained")
        };
        assert_eq!(
            first.for_member(a),
            &AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Point { point_id: point_a },
                [2.0, 2.0, 3.0]
            )))
        );
        assert_eq!(
            first.template,
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [1.0, 0.0, 0.0]
            )))
        );
        environment.group_members.insert("1".into(), vec![a]);
        let edited = run(
            &intent,
            &environment,
            &HashMap::from([(("1".into(), key), value.clone())]),
        );
        let AttributeValue::GroupFamily(second) = edited else {
            panic!("dormant frame retained")
        };
        assert_eq!(second.for_member(b), first.for_member(b));
        assert_eq!(
            second.for_member(a),
            &AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Point { point_id: point_a },
                [3.0, 2.0, 3.0]
            )))
        );
    }
}
