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

/// Selectable identity → the color channel attributes its heads expose. A plain fixture id
/// resolves through its shared heads; a logical-head id through exactly that head — the same
/// rule the operator color dialog applied while this resolution still lived client-side.
pub(crate) type ColorAttributeIndex = HashMap<FixtureId, Vec<&'static str>>;

pub(crate) fn color_attribute_index(state: &AppState) -> ColorAttributeIndex {
    let snapshot = state.engine.snapshot();
    let mut index = ColorAttributeIndex::new();
    for fixture in snapshot.fixtures.iter() {
        index.insert(
            fixture.fixture_id,
            supported_channels(&fixture.definition, |head| head.shared),
        );
        for logical in &fixture.logical_heads {
            index.insert(
                logical.fixture_id,
                supported_channels(&fixture.definition, |head| head.index == logical.head_index),
            );
        }
    }
    index
}

fn supported_channels(
    definition: &light_fixture::FixtureDefinition,
    head_filter: impl Fn(&light_fixture::LogicalHead) -> bool,
) -> Vec<&'static str> {
    COLOR_CHANNELS
        .iter()
        .filter(|(attribute, _, _)| {
            definition
                .heads
                .iter()
                .filter(|head| head_filter(head))
                .any(|head| {
                    head.parameters
                        .iter()
                        .any(|parameter| parameter.attribute.0 == *attribute)
                })
        })
        .map(|(attribute, _, _)| *attribute)
        .collect()
}
