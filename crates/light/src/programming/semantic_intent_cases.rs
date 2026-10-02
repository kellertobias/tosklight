//! Deterministic semantic programming intent shared by persistence and selective-import tests.
//!
//! Every case is requested intent, never a fitted native channel or an achieved approximation.
//! Values are chosen away from their defaults so a dropped field decodes visibly differently:
//! `relative_output` defaults to one and `uv` to zero, so zero output and nonzero UV are both
//! represented. Payload equality proves retained authoring only; it does not prove physical output.
use light_core::{
    AttributeValue, OpeningConvention,
    programming::{
        ColorAllocation, ColorIntent, ColorProgram, GroupFamilyAssignment, PositionIntent,
        ProgrammingOwner, ScalarIntent, TargetReference, UvIntent, VirtualColorAuthoringV1,
        VirtualColorRecipe, WhiteTarget, ZoomIntent,
    },
};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

/// Exact virtual recipe, so validation also checks the authoritative XYZ against the recipe.
fn intent(rgb: [f32; 3], amber: f32) -> ColorIntent {
    let recipe = VirtualColorRecipe {
        version: 1,
        rgb,
        amber,
        approximate: false,
    };
    ColorIntent {
        base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe).expect("valid virtual recipe"),
        recipe,
        ..ColorIntent::default()
    }
}

/// Magenta with a nondefault White Blend, white target and relative output. UV stays zero.
pub(crate) fn magenta() -> ColorIntent {
    ColorIntent {
        white_blend: 0.25,
        white_target: WhiteTarget {
            kelvin: 5600.0,
            duv: -0.004,
        },
        relative_output: 0.8,
        ..intent([1.0, 0.0, 1.0], 0.0)
    }
}

/// Magenta with an independent nonzero UV component.
pub(crate) fn magenta_with_uv() -> ColorIntent {
    ColorIntent {
        uv: UvIntent { amount: 0.45 },
        ..magenta()
    }
}

/// 3200 K warm white: CCT/Duv target, nondefault White Blend, allocation and amber recipe.
pub(crate) fn warm_white_3200() -> ColorIntent {
    ColorIntent {
        white_blend: 0.85,
        white_target: WhiteTarget {
            kelvin: 3200.0,
            duv: 0.0035,
        },
        relative_output: 0.6,
        allocation: ColorAllocation::PreferWhite,
        ..intent([1.0, 0.72, 0.42], 0.3)
    }
}

/// The same warm white requested at zero relative output. Zero must not decode as the default one.
pub(crate) fn warm_white_zero_output() -> ColorIntent {
    ColorIntent {
        relative_output: 0.0,
        ..warm_white_3200()
    }
}

/// Black visible color with only UV requested.
pub(crate) fn uv_only_black() -> ColorIntent {
    ColorIntent {
        relative_output: 0.0,
        uv: UvIntent { amount: 0.9 },
        ..intent([0.0, 0.0, 0.0], 0.0)
    }
}

pub(crate) fn color(intent: ColorIntent) -> AttributeValue {
    let program = ColorProgram::Semantic { intent };
    program.validate().expect("valid semantic Color case");
    AttributeValue::ColorProgram(Arc::new(program))
}

pub(crate) fn angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25)))
}

/// A tagged Point target with a nonzero XYZ offset.
pub(crate) fn point_target(point_id: Uuid) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id },
        [0.5, -1.25, 2.0],
    )))
}

pub(crate) fn focus() -> AttributeValue {
    AttributeValue::Normalized(0.37)
}

pub(crate) fn zoom() -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(23.5),
        convention: OpeningConvention::Beam,
    }))
}

pub(crate) fn group_family(
    owner: ProgrammingOwner,
    template: AttributeValue,
    members: impl IntoIterator<Item = (Uuid, AttributeValue)>,
) -> AttributeValue {
    let assignment = GroupFamilyAssignment {
        owner,
        template,
        members: members.into_iter().collect::<BTreeMap<_, _>>(),
    };
    assignment.validate().expect("valid live Group case");
    AttributeValue::GroupFamily(Arc::new(assignment))
}

/// Rewrites only identity fields: Point targets and live-Group member keys. All other intent,
/// including every Color field, must survive unchanged for the remapped value to compare equal.
pub(crate) fn remap(value: &AttributeValue, ids: &BTreeMap<Uuid, Uuid>) -> AttributeValue {
    let id = |value: &Uuid| ids.get(value).copied().unwrap_or(*value);
    match value {
        AttributeValue::Position(position) => match position.as_ref() {
            PositionIntent::Target {
                reference: TargetReference::Point { point_id },
                offset_metres,
            } => AttributeValue::Position(Arc::new(PositionIntent::Target {
                reference: TargetReference::Point {
                    point_id: id(point_id),
                },
                offset_metres: offset_metres.clone(),
            })),
            _ => value.clone(),
        },
        AttributeValue::GroupFamily(assignment) => {
            AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
                owner: assignment.owner,
                template: remap(&assignment.template, ids),
                members: assignment
                    .members
                    .iter()
                    .map(|(member, value)| (id(member), remap(value, ids)))
                    .collect(),
            }))
        }
        _ => value.clone(),
    }
}

/// Persisted semantic records contain no native recipe, estimate or wheel substitution.
/// `values` is one persisted attribute map (for example `/values/<fixture>` of a Preset).
pub(crate) fn assert_semantic_attribute_map(values: &Value, allowed: &[&str]) {
    let map = values.as_object().expect("persisted attribute map");
    for (attribute, value) in map {
        assert!(
            allowed.contains(&attribute.as_str()),
            "native or unrelated attribute {attribute} was persisted instead of intent: {map:?}"
        );
        assert_semantic_value(value);
    }
}

pub(crate) fn assert_semantic_value(value: &Value) {
    match value["kind"].as_str() {
        Some("color_program") => {
            assert_eq!(value["value"]["kind"], "semantic", "{value}");
            let intent = &value["value"]["intent"];
            for native in ["recipe/source", "channels", "portable"] {
                assert!(intent.pointer(&format!("/{native}")).is_none(), "{value}");
            }
            assert!(intent.get("wheel_constraints").is_none(), "{value}");
        }
        Some("group_family") => {
            assert_semantic_value(&value["value"]["template"]);
            for member in value["value"]["members"]
                .as_object()
                .into_iter()
                .flat_map(|members| members.values())
            {
                assert_semantic_value(member);
            }
        }
        Some("position" | "zoom" | "normalized") => {}
        other => panic!("unexpected persisted programming kind {other:?}: {value}"),
    }
}

/// Persisted-body equality at the precision the typed contract owns. Intent scalars are `f32`; the
/// show store reparses its JSON text as `f64` without `serde_json/float_roundtrip`, which can move
/// the 17th significant digit. Such a number still decodes to the identical `f32`, so it is equal
/// here, while any structural, key, string or `f32`-visible difference still fails.
pub(crate) fn assert_persisted_eq(actual: &Value, expected: &Value) {
    assert!(
        persisted_eq(actual, expected),
        "persisted body differs\n actual: {actual}\nexpected: {expected}"
    );
}

fn persisted_eq(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            left == right
                || matches!(
                    (left.as_f64(), right.as_f64()),
                    (Some(left), Some(right)) if left as f32 == right as f32
                )
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(l, r)| persisted_eq(l, r))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, l)| right.get(key).is_some_and(|r| persisted_eq(l, r)))
        }
        _ => actual == expected,
    }
}

#[test]
fn cases_are_valid_distinct_and_away_from_defaults() {
    let cases = [
        magenta(),
        magenta_with_uv(),
        warm_white_3200(),
        warm_white_zero_output(),
        uv_only_black(),
    ];
    for (index, case) in cases.iter().enumerate() {
        case.validate().unwrap();
        assert_ne!(case, &ColorIntent::default());
        assert!(cases[index + 1..].iter().all(|other| other != case));
    }
    assert_eq!(warm_white_zero_output().relative_output, 0.0);
    assert_eq!(uv_only_black().base_xyz.y, 0.0);
    assert!(uv_only_black().uv.amount > 0.0);
    assert_eq!(magenta().uv.amount, 0.0);
    assert_ne!(angles(), point_target(Uuid::from_u128(1)));
}
