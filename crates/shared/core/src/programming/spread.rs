//! Compile complete family curves once when an ordered selection changes. Runtime consumers
//! index the result by rank; they never allocate or reinterpret physical units while sampling.
use super::*;
use crate::AttributeValue;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum RankedProgrammingValue {
    Constant(AttributeValue),
    Ranks(Vec<AttributeValue>),
}
impl RankedProgrammingValue {
    pub fn at_rank(&self, rank: usize) -> Option<&AttributeValue> {
        match self {
            Self::Constant(value) => Some(value),
            Self::Ranks(values) => values.get(rank),
        }
    }
}

/// The source stays immutable and recordable. Direct curves require their pinned forward model
/// so each sampled native recipe gets its own portable estimate, including independently known UV.
pub fn compile_programming_spread(
    value: &AttributeValue,
    rank_count: usize,
    context: &FamilyEditContext<'_>,
) -> Result<RankedProgrammingValue, IntentError> {
    if value.spread_control_points() == 0 {
        return compile_programming_ranks(value, rank_count, &[], context);
    }
    compile_programming_ranks(
        value,
        rank_count,
        &(0..rank_count).collect::<Vec<_>>(),
        context,
    )
}

/// The returned ranks follow `ranks` order, not global rank indices. Used for heterogeneous
/// Group exceptions so one fixture never causes materialization of every other fixture's rank.
pub fn compile_programming_ranks(
    value: &AttributeValue,
    rank_count: usize,
    ranks: &[usize],
    context: &FamilyEditContext<'_>,
) -> Result<RankedProgrammingValue, IntentError> {
    require(
        ranks.iter().all(|rank| *rank < rank_count),
        "spread rank is outside its selection",
    )?;
    require(
        !matches!(value, AttributeValue::GroupFamily(_)),
        "resolve Group membership before compiling family ranks",
    )?;
    if let AttributeValue::Spread(points) = value {
        ScalarIntent::Spread(points.clone()).validate(ScalarDomain::UNIT)?;
    }
    if let Some(owner) = value.programming_owner() {
        value.validate_programming_address(&owner.key())?;
    }
    if value.spread_control_points() == 0 {
        return Ok(RankedProgrammingValue::Constant(value.clone()));
    }
    let resolved = match value {
        AttributeValue::Spread(points) => {
            ScalarIntent::Spread(points.clone()).validate(ScalarDomain::UNIT)?;
            resolve_legacy_selected(points, rank_count, ranks)
                .into_iter()
                .map(AttributeValue::Normalized)
                .collect()
        }
        AttributeValue::Position(position) => match position.as_ref() {
            PositionIntent::Angles {
                pan_degrees,
                tilt_degrees,
            } => {
                let pan = pan_degrees.resolve_selected(rank_count, ranks, false)?;
                let tilt = tilt_degrees.resolve_selected(rank_count, ranks, false)?;
                pan.into_iter()
                    .zip(tilt)
                    .map(|(pan, tilt)| {
                        AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
                    })
                    .collect()
            }
            PositionIntent::Target {
                reference,
                offset_metres,
            } => {
                let offsets = [
                    offset_metres[0].resolve_selected(rank_count, ranks, false)?,
                    offset_metres[1].resolve_selected(rank_count, ranks, false)?,
                    offset_metres[2].resolve_selected(rank_count, ranks, false)?,
                ];
                (0..ranks.len())
                    .map(|rank| {
                        AttributeValue::Position(Arc::new(PositionIntent::target(
                            *reference,
                            offsets.each_ref().map(|axis| axis[rank]),
                        )))
                    })
                    .collect()
            }
        },
        AttributeValue::Zoom(zoom) => zoom
            .opening_degrees
            .resolve_selected(rank_count, ranks, false)?
            .into_iter()
            .map(|value| {
                AttributeValue::Zoom(Arc::new(ZoomIntent {
                    opening_degrees: ScalarIntent::Value(value),
                    convention: zoom.convention,
                }))
            })
            .collect(),
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => {
                // Set saturation before hue so an achromatic starting RGB can acquire the
                // requested hue when both curves are present. RGB/HS conflicts were validated.
                let mut spreads = intent.spreads.iter().collect::<Vec<_>>();
                spreads.sort_by_key(|spread| u8::from(spread.component == ColorComponent::Hue));
                let curves = spreads
                    .iter()
                    .map(|spread| {
                        ScalarIntent::Spread(spread.points.clone()).resolve_selected(
                            rank_count,
                            ranks,
                            spread.component == ColorComponent::Hue,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut seed = intent.clone();
                seed.spreads.clear();
                let seed =
                    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent: seed }));
                (0..ranks.len())
                    .map(|rank| {
                        let edits = spreads
                            .iter()
                            .zip(&curves)
                            .map(|(spread, values)| ComponentEdit::Scalar {
                                component: ProgrammingComponent::Color(spread.component),
                                operation: ScalarEdit::Set(ScalarIntent::Value(values[rank])),
                            })
                            .collect::<Vec<_>>();
                        edit_family(&seed, &edits, context)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            ColorProgram::Direct { recipe, portable } => {
                let model = context.native_model.ok_or_else(|| {
                    IntentError("native spread requires its verified source model".into())
                })?;
                require(
                    model.source() == &recipe.source,
                    "native spread source identity changed",
                )?;
                for spread in &recipe.spreads {
                    let descriptor = model.descriptor(spread.binding).ok_or_else(|| {
                        IntentError("native spread binding is absent from source".into())
                    })?;
                    spread.validate(descriptor)?;
                }
                let curves = recipe
                    .spreads
                    .iter()
                    .map(|spread| spread.resolve_selected(rank_count, ranks))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut seed = recipe.clone();
                seed.spreads.clear();
                let seed = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                    recipe: seed,
                    portable: portable.clone(),
                }));
                (0..ranks.len())
                    .map(|rank| {
                        let edits = recipe
                            .spreads
                            .iter()
                            .zip(&curves)
                            .map(|(spread, values)| ComponentEdit::Native {
                                binding: spread.binding,
                                operation: NativeColorEdit::Set(values[rank]),
                            })
                            .collect::<Vec<_>>();
                        edit_family(&seed, &edits, context)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        },
        _ => unreachable!("spread_control_points is exhaustive"),
    };
    Ok(RankedProgrammingValue::Ranks(resolved))
}

fn resolve_legacy_selected(points: &[f32], count: usize, ranks: &[usize]) -> Vec<f32> {
    let layout = super::ranks::SpreadRankLayout::new(points.len(), count);
    ranks
        .iter()
        .map(|rank| {
            if points.len() > count && count > 1 {
                let position = *rank as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
                let left = position.floor() as usize;
                let right = position.ceil() as usize;
                points[left] + (points[right] - points[left]) * (position - left as f32)
            } else {
                let (left, right, step, span) = layout.weights(*rank);
                if left == right {
                    points[left]
                } else {
                    (points[left] * (span - step) as f32 + points[right] * step as f32)
                        / span as f32
                }
            }
        })
        .collect()
}

/// `members` is authoritative current membership, already ranked. Exceptions never create
/// fixture ownership outside it. Models are selected by the complete source, not destination.
pub fn compile_group_member_values<'a>(
    value: &AttributeValue,
    members: &[(crate::FixtureId, usize)],
    rank_count: usize,
    context: impl Fn(&AttributeValue) -> Result<FamilyEditContext<'a>, IntentError>,
) -> Result<Vec<(crate::FixtureId, AttributeValue)>, IntentError> {
    let mut identities = std::collections::HashSet::with_capacity(members.len());
    require(
        members
            .iter()
            .all(|(id, rank)| *rank < rank_count && identities.insert(*id)),
        "Group members require unique identities and valid ranks",
    )?;
    let (template, exceptions) = match value {
        AttributeValue::GroupFamily(assignment) => {
            assignment.validate()?;
            (&assignment.template, Some(&assignment.members))
        }
        _ => (value, None),
    };
    let template_members = members
        .iter()
        .filter(|(id, _)| !exceptions.is_some_and(|values| values.contains_key(&id.0)))
        .copied()
        .collect::<Vec<_>>();
    let ranks = template_members
        .iter()
        .map(|(_, rank)| *rank)
        .collect::<Vec<_>>();
    let mut result = std::collections::HashMap::with_capacity(members.len());
    if !ranks.is_empty() {
        let compiled =
            compile_programming_ranks(template, rank_count, &ranks, &context(template)?)?;
        for (index, (id, _)) in template_members.into_iter().enumerate() {
            result.insert(
                id,
                compiled
                    .at_rank(index)
                    .expect("requested rank was compiled")
                    .clone(),
            );
        }
    }
    if let Some(exceptions) = exceptions {
        for (id, rank) in members {
            if let Some(value) = exceptions.get(&id.0) {
                let compiled =
                    compile_programming_ranks(value, rank_count, &[*rank], &context(value)?)?;
                result.insert(
                    *id,
                    compiled
                        .at_rank(0)
                        .expect("member rank was compiled")
                        .clone(),
                );
            }
        }
    }
    Ok(members
        .iter()
        .map(|(id, _)| (*id, result.remove(id).expect("all members were compiled")))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_ranks_keep_multi_turn_angles_and_referenced_offsets() {
        let position = AttributeValue::Position(Arc::new(PositionIntent::Angles {
            pan_degrees: ScalarIntent::Spread(vec![-720.0, 720.0]),
            tilt_degrees: ScalarIntent::Value(90.0),
        }));
        let ranks = compile_programming_spread(&position, 3, &Default::default()).unwrap();
        for (rank, pan) in [-720.0, 0.0, 720.0].into_iter().enumerate() {
            assert_eq!(
                ranks.at_rank(rank),
                Some(&AttributeValue::Position(Arc::new(PositionIntent::angles(
                    pan, 90.0
                ))))
            );
        }
        let reference = TargetReference::Point {
            point_id: uuid::Uuid::new_v4(),
        };
        let target = AttributeValue::Position(Arc::new(PositionIntent::Target {
            reference,
            offset_metres: [
                ScalarIntent::Spread(vec![-10.0, 20.0]),
                ScalarIntent::Value(2.0),
                ScalarIntent::Value(3.0),
            ],
        }));
        let ranks = compile_programming_spread(&target, 3, &Default::default()).unwrap();
        assert_eq!(
            ranks.at_rank(1),
            Some(&AttributeValue::Position(Arc::new(PositionIntent::target(
                reference,
                [5.0, 2.0, 3.0]
            ))))
        );
        assert_eq!(
            target.spread_control_points(),
            2,
            "recorded curve remains intact"
        );
        assert!(ranks.at_rank(3).is_none());
    }

    #[test]
    fn white_balance_and_uv_curves_preserve_black_and_visible_output() {
        let intent = ColorIntent {
            recipe: VirtualColorRecipe {
                rgb: [0.0; 3],
                ..Default::default()
            },
            base_xyz: crate::Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            relative_output: 0.0,
            spreads: vec![
                ColorComponentSpread {
                    component: ColorComponent::Temperature,
                    points: vec![2000.0, 10000.0],
                },
                ColorComponentSpread {
                    component: ColorComponent::Uv,
                    points: vec![0.0, 1.0],
                },
            ],
            ..Default::default()
        };
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
        let ranks = compile_programming_spread(&value, 3, &Default::default()).unwrap();
        let AttributeValue::ColorProgram(program) = ranks.at_rank(1).unwrap() else {
            panic!()
        };
        let ColorProgram::Semantic { intent } = program.as_ref() else {
            panic!()
        };
        assert_eq!(
            intent.base_xyz,
            crate::Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(intent.relative_output, 0.0);
        assert_eq!(intent.uv.amount, 0.5);
        assert_eq!(intent.white_target.kelvin, 6000.0);
        assert!(intent.spreads.is_empty());
    }

    #[test]
    fn constant_families_share_storage_instead_of_allocating_per_rank() {
        let position = Arc::new(PositionIntent::angles(20.0, 30.0));
        let value = AttributeValue::Position(position.clone());
        let ranks = compile_programming_spread(&value, 1000, &Default::default()).unwrap();
        let RankedProgrammingValue::Constant(AttributeValue::Position(result)) = ranks else {
            panic!()
        };
        assert!(Arc::ptr_eq(&position, &result));
    }
}

#[cfg(test)]
mod selected_tests {
    use super::*;
    #[test]
    fn selected_ranks_match_existing_full_scalar_and_exact_native_layouts() {
        for count in 1..65 {
            for points in 2..=12 {
                let physical = (0..points)
                    .map(|index| index as f32 * -113.125 + 720.0)
                    .collect::<Vec<_>>();
                let scalar = ScalarIntent::Spread(physical.clone());
                let ranks = (0..count).rev().collect::<Vec<_>>();
                let full = scalar.resolve(count, false);
                assert_eq!(
                    scalar.resolve_selected(count, &ranks, false).unwrap(),
                    ranks.iter().map(|rank| full[*rank]).collect::<Vec<_>>()
                );
                let normalized = (0..points)
                    .map(|index| index as f32 / (points - 1) as f32)
                    .collect::<Vec<_>>();
                let legacy = crate::attributes::resolve_spread(&normalized, count);
                assert_eq!(
                    resolve_legacy_selected(&normalized, count, &ranks),
                    ranks.iter().map(|rank| legacy[*rank]).collect::<Vec<_>>()
                );
                let native = NativeColorSpread {
                    binding: crate::NativeColorBinding {
                        channel_id: uuid::Uuid::new_v4(),
                        function_id: uuid::Uuid::new_v4(),
                    },
                    points: (0..points).map(|index| u32::MAX - index as u32).collect(),
                };
                let full = native.resolve(count).unwrap();
                assert_eq!(
                    native.resolve_selected(count, &ranks).unwrap(),
                    ranks.iter().map(|rank| full[*rank]).collect::<Vec<_>>()
                );
            }
        }
    }
    #[test]
    fn membership_precedes_exceptions_and_equal_ranks_keep_different_targets() {
        let a = crate::FixtureId::new();
        let b = crate::FixtureId::new();
        let removed = crate::FixtureId::new();
        let added = crate::FixtureId::new();
        let target = |id| {
            AttributeValue::Position(Arc::new(PositionIntent::Target {
                reference: TargetReference::Point { point_id: id },
                offset_metres: [
                    ScalarIntent::Spread(vec![-2.0, 2.0]),
                    ScalarIntent::Value(0.0),
                    ScalarIntent::Value(0.0),
                ],
            }))
        };
        let template = AttributeValue::Position(Arc::new(PositionIntent::angles(90.0, 0.0)));
        let value = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: template.clone(),
            members: std::collections::BTreeMap::from([
                (a.0, target(uuid::Uuid::new_v4())),
                (b.0, target(uuid::Uuid::new_v4())),
                (removed.0, target(uuid::Uuid::new_v4())),
            ]),
        }));
        let result = compile_group_member_values(&value, &[(a, 0), (b, 0), (added, 1)], 2, |_| {
            Ok(FamilyEditContext::default())
        })
        .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[2], (added, template));
        assert_ne!(result[0].1, result[1].1);
        assert!(
            result
                .iter()
                .all(|(id, value)| *id != removed && value.spread_control_points() == 0)
        );
    }
}
