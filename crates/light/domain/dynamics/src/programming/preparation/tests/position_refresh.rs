//! TL-842 P1: a controller of plain authored Pan/Tilt leaves, prepared over retained samples,
//! rewrites last frame's compiled Position forest in place. Every frame must equal a fresh
//! preparation, both composed and in every compiled part, across value, rank, activation,
//! occurrence and shape changes, a rejected value, and a Current axis that cannot refresh.
use super::*;
use crate::programming::expression_angles::POSITION_REFRESHES;

fn occurrence(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}

/// One controller on one target. Frame 4 rejects its tilt (not finite), frame 5
/// reads Current tilt, frame 7 drops the tilt lane.
fn frame(index: usize) -> Vec<DynamicRuntimeSample> {
    let mut pan = sample(
        30,
        angle(ProgrammingComponent::Pan, 10. + 7. * index as f32),
    );
    let tilt_value = if index == 4 {
        f32::NAN
    } else {
        20. + 3. * index as f32
    };
    let mut tilt = sample(31, angle(ProgrammingComponent::Tilt, tilt_value));
    if index == 5 {
        tilt = sample(31, current(ProgrammingComponent::Tilt));
    }
    if index == 3 {
        let E::Programming {
            occurrence: slot, ..
        } = &mut pan.expression
        else {
            unreachable!()
        };
        *slot = Some(occurrence(77));
    }
    for lane in [&mut pan, &mut tilt] {
        lane.controller_id = Uuid::from_u128(90);
        lane.priority += (index % 3) as i16;
        lane.activation_mix = 1.0 - index as f32 * 0.05;
    }
    if index == 7 {
        vec![pan]
    } else {
        vec![pan, tilt]
    }
}

/// The composed snapshot and every compiled part of the prepared Position sample.
fn prepared(
    input: &[DynamicRuntimeSample],
    scratch: &mut DynamicFamilyPreparationScratch,
) -> Result<(String, Option<String>), String> {
    let sources = Sources {
        pan: Some(30.),
        tilt: Some(40.),
        ..Default::default()
    };
    let prepared = prepare_dynamic_family_samples(input, &sources, None, scratch)
        .map_err(|error| format!("{error:?}"))?;
    let compiled = prepared
        .families
        .iter()
        .filter(|group| group.owner == ProgrammingOwner::Position)
        .flat_map(|group| &group.samples)
        .find_map(|sample| match sample {
            FamilyCompositionSample::CoupledExpression {
                expression,
                rank,
                activation_mix,
            } => Some(format!(
                "{rank:?} {}\n{}",
                activation_mix.to_bits(),
                expression.describe_plain_whole()
            )),
            _ => None,
        });
    Ok((preparation_snapshot(prepared), compiled))
}

#[test]
fn plain_position_controller_refreshes_in_place_exactly_like_a_fresh_compile() {
    let mut retained = DynamicFamilyPreparationScratch::default();
    let before = POSITION_REFRESHES.with(std::cell::Cell::get);
    for index in 0..9 {
        let input = frame(index);
        let expected = prepared(&input, &mut DynamicFamilyPreparationScratch::default());
        let actual = prepared(&input, &mut retained);
        assert_eq!(actual, expected, "frame {index}");
        assert_eq!(actual.is_err(), index == 4, "frame {index}");
    }
    // Frames 1, 2 and 3 rewrite in place. 4 is rejected and 5 reads Current, so both compile
    // afresh; 5 still compiles to the same one-leaf shape, so 6 rewrites it. 7 has one lane
    // after two and 8 two after one: both compile afresh.
    let refreshed = POSITION_REFRESHES.with(std::cell::Cell::get) - before;
    assert_eq!(refreshed, 4);
}
