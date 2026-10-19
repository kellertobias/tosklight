//! TL-842 C4: a preparation scratch that keeps its groups between frames must produce exactly
//! what a fresh scratch produces, whether or not a consumer drained the groups in between.
use super::*;

/// Frames that revalue, re-rank, remove and re-add targets, add a target that sorts first,
/// and change Position Current.
fn frame(index: usize) -> Vec<DynamicRuntimeSample> {
    let targets: &[u128] = match index {
        0 => &[3, 1, 2],
        1 => &[2],
        2 => &[2, 1, 3, 0],
        3 => &[],
        _ => &[1, 3, 2],
    };
    let mut inputs = Vec::new();
    for &target in targets {
        for (lane, component) in [ColorComponent::Red, ColorComponent::Green]
            .into_iter()
            .enumerate()
        {
            if index == 2 && target == 3 && lane == 1 {
                continue; // a shorter group than last frame
            }
            let value = if index == 5 && target == 3 && lane == 0 {
                1.5 // outside the slot domain: rejected in place exactly like a fresh build
            } else {
                0.1 + (index % 5) as f32 * 0.15 + target as f32 * 0.01
            };
            let mut input = sample(10 + lane as u128, color(component, value));
            input.target = FixtureId(Uuid::from_u128(100 + target));
            input.priority += (index % 2) as i16;
            input.activation_mix = 1.0 - index as f32 * 0.1;
            inputs.push(input);
        }
    }
    if index != 3 {
        let mut pan = sample(30, angle(ProgrammingComponent::Pan, 20. * index as f32));
        pan.controller_id = Uuid::from_u128(90);
        let mut tilt = sample(31, current(ProgrammingComponent::Tilt));
        tilt.controller_id = pan.controller_id;
        inputs.extend([pan, tilt]);
    }
    inputs
}

#[test]
fn retained_layout_matches_fresh_preparation_over_changing_frames() {
    for chunks in [0, 1, 2, 4] {
        for stop in [false, true] {
            for drained in [false, true] {
                let mut retained = DynamicFamilyPreparationScratch::default();
                let refreshes = super::super::plain_scalar::tests::REFRESHES.with(|n| n.get());
                for index in 0..8 {
                    let input = frame(index);
                    let expected = prepare_observed(
                        &input,
                        &[],
                        &mut DynamicFamilyPreparationScratch::default(),
                        chunks,
                        stop,
                    );
                    let actual = prepare_observed(&input, &[], &mut retained, chunks, stop);
                    assert_eq!(
                        actual.0, expected.0,
                        "frame {index}, chunks {chunks}, stop {stop}, drained {drained}"
                    );
                    assert_eq!(
                        retained.family_indices.len(),
                        retained.families.len(),
                        "the index must name exactly the retained groups"
                    );
                    for (index, group) in retained.families.iter().enumerate() {
                        assert_eq!(retained.family_indices[&(group.target, group.owner)], index);
                    }
                    if drained {
                        for group in retained.prepared_for_drain().families {
                            group.samples.clear();
                        }
                    }
                    assert_eq!(actual.0.is_err(), index == 5);
                }
                // TL-842 C6: serial output over retained samples rewrites them in place; a
                // drained group has no sample to rewrite.
                let refreshed =
                    super::super::plain_scalar::tests::REFRESHES.with(|n| n.get()) - refreshes;
                if drained {
                    assert_eq!(refreshed, 0, "chunks {chunks}");
                } else if chunks == 0 {
                    assert!(refreshed > 0, "stop {stop}");
                }
            }
        }
    }
}
