//! TL-657: each readout owner carries the Color its frame resolves, the colour a first semantic
//! Color edit starts from while the Programmer holds none.
use super::*;
use light_core::programming::{ColorIntent, ColorProgram};
use light_engine::{FrameValues, ResolvedChangedAt, ResolvedValues};

fn owner(id: FixtureId) -> PositionCommandedOwnerReadout {
    PositionCommandedOwnerReadout {
        owner: id,
        commands: None,
    }
}

#[test]
fn readout_owner_carries_the_frame_color_and_none_without_one() {
    let coloured = FixtureId::new();
    let colourless = FixtureId::new();
    let mut intent = ColorIntent::default();
    intent.recipe.rgb = [1.0, 0.0, 0.25];
    let red = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
    let mut values = ResolvedValues::default();
    values.insert((coloured, ProgrammingOwner::Color.key()), red.clone());
    let frame = FrameValues::from_maps(values, ResolvedChangedAt::default());
    let owners = requested_owners(
        vec![owner(coloured), owner(colourless), owner(coloured)],
        vec![None, None, None],
        &frame,
    );
    assert_eq!(owners[0].color, Some(red.clone()));
    assert_eq!(owners[1].color, None);
    assert_eq!(owners[2].color, Some(red));
    assert!(owners.iter().all(|owner| owner.requested.is_none()));
}
