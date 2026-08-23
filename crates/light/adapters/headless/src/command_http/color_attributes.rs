use std::collections::HashMap;

use light_core::FixtureId;

use super::super::AppState;

/// The six operator-facing color channel attributes a color-range fan-out can address,
/// paired with which RGB component (direct or inverted) resolves them.
pub(super) const COLOR_CHANNELS: [(&str, usize, bool); 6] = [
    ("color.red", 0, false),
    ("color.green", 1, false),
    ("color.blue", 2, false),
    ("color.cyan", 0, true),
    ("color.magenta", 1, true),
    ("color.yellow", 2, true),
];

/// How one selectable identity accepts a whole-color picker value. Profile color systems consume
/// canonical XYZ; legacy definitions without authored systems retain their direct RGB/CMY fan-out.
#[derive(Clone, Debug)]
pub(crate) enum ColorTarget {
    Canonical,
    Direct(Vec<&'static str>),
}

/// Selectable identity → the whole-color target its heads expose. A plain fixture id resolves
/// through its shared heads; a logical-head id through exactly that head.
pub(crate) type ColorAttributeIndex = HashMap<FixtureId, ColorTarget>;

pub(crate) fn color_attribute_index(state: &AppState) -> ColorAttributeIndex {
    let snapshot = state.output.snapshot();
    let mut index = ColorAttributeIndex::new();
    for fixture in snapshot.fixtures.iter() {
        index.insert(
            fixture.fixture_id,
            color_target(&fixture.definition, |head| head.shared),
        );
        for logical in &fixture.logical_heads {
            index.insert(
                logical.fixture_id,
                color_target(&fixture.definition, |head| head.index == logical.head_index),
            );
        }
    }
    index
}

fn color_target(
    definition: &light_fixture::FixtureDefinition,
    head_filter: impl Fn(&light_fixture::LogicalHead) -> bool,
) -> ColorTarget {
    let selected_heads = definition
        .heads
        .iter()
        .filter(|head| head_filter(head))
        .collect::<Vec<_>>();
    let has_authored_system = definition
        .profile_snapshot
        .as_deref()
        .and_then(|profile| {
            let mode_id = definition.mode_id?;
            profile.modes.iter().find(|mode| mode.id == mode_id)
        })
        .is_some_and(|mode| {
            selected_heads.iter().any(|head| {
                mode.heads
                    .get(head.index as usize)
                    .is_some_and(|profile_head| {
                        mode.color_systems
                            .iter()
                            .any(|system| system.head_id == profile_head.id)
                    })
            })
        });
    if has_authored_system {
        return ColorTarget::Canonical;
    }
    ColorTarget::Direct(
        COLOR_CHANNELS
            .iter()
            .filter(|(attribute, _, _)| {
                selected_heads.iter().any(|head| {
                    head.parameters
                        .iter()
                        .any(|parameter| &*parameter.attribute.0 == *attribute)
                })
            })
            .map(|(attribute, _, _)| *attribute)
            .collect(),
    )
}
