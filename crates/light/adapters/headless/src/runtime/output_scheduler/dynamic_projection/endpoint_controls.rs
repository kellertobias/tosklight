//! Captured output gates are transient. They never edit authored values, retained samples or
//! source occurrences. The typed compositor applies a master to the evaluated endpoint, before
//! activation; scalar Intensity and physical sequence-master handling keep their existing paths.
#![allow(dead_code)]

use super::CapturedDynamicOutputControls;
use light_core::programming::IntentError;
use light_dynamics::{FamilyEndpointOutputControl, FamilySampleRank};

/// Validate once for a captured frame, then borrow its exact controls for every owner group.
/// This adapter does not read the current Playback engine or copy its maps per fixture.
pub(super) struct CapturedFamilyEndpointControls<'a> {
    captured: CapturedDynamicOutputControls<'a>,
}

impl<'a> CapturedFamilyEndpointControls<'a> {
    pub fn new(captured: CapturedDynamicOutputControls<'a>) -> Result<Self, IntentError> {
        if captured
            .playbacks
            .values()
            .any(|control| !control.master.is_finite() || !(0.0..=1.0).contains(&control.master))
        {
            return Err(IntentError(
                "captured Dynamic Playback master must be finite and between zero and one".into(),
            ));
        }
        Ok(Self { captured })
    }

    pub fn control_for(&self, rank: FamilySampleRank) -> FamilyEndpointOutputControl {
        let Some(identity) = rank.dynamic_identity() else {
            // Fixed Cue/Programmer masks have authored timing, but no standalone Dynamic master.
            return FamilyEndpointOutputControl::Unchanged;
        };
        if self
            .captured
            .cues
            .get(&identity.controller_id)
            .is_some_and(|cue| !cue.enabled)
        {
            return FamilyEndpointOutputControl::Suppressed;
        }
        let Some(control) = self.captured.playbacks.get(&identity.controller_id) else {
            // A Cue sequence master does not fade a non-intensity Color/Position/Focus owner
            // toward zero. Its physical brightness ownership is handled independently.
            return FamilyEndpointOutputControl::Unchanged;
        };
        if control.crossfade_non_intensity {
            FamilyEndpointOutputControl::CrossfadeCurrent {
                mix: control.master,
            }
        } else if control.master == 0.0 {
            FamilyEndpointOutputControl::Suppressed
        } else {
            FamilyEndpointOutputControl::Unchanged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::output_scheduler::dynamic_projection::{
        CueDynamicOutputControl, DynamicPlaybackControl,
    };
    use light_core::{AttributeValue, FixtureId, programming::*};
    use light_dynamics::*;
    use std::{collections::HashMap, sync::Arc};
    use uuid::Uuid;

    struct Current(FixtureId);
    impl DynamicValueSourceResolver for Current {
        fn current(
            &self,
            target: FixtureId,
            address: &DynamicValueAddress,
        ) -> Option<DynamicValue> {
            assert_eq!(target, self.0);
            assert_eq!(address.owner(), ProgrammingOwner::Focus);
            Some(if address.component.is_some() {
                DynamicValue::Scalar(0.2)
            } else {
                DynamicValue::Family(AttributeValue::Normalized(0.2))
            })
        }
        fn preset(
            &self,
            _: &DynamicPresetSourceBinding,
            _: Uuid,
            _: FixtureId,
        ) -> Option<DynamicValue> {
            panic!("output masters cannot read presets")
        }
    }
    struct NoConversion;
    impl WholeFamilyExpressionFrameResolver for NoConversion {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            panic!("normalized Focus requires no representation conversion")
        }
    }

    fn rank(order: u128) -> FamilySampleRank {
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(100 + order),
                controller_id: Uuid::from_u128(200 + order),
                lane_id: Uuid::from_u128(300 + order),
            },
        }
    }
    fn focus(value: f32, order: u128, mix: f32) -> FamilyCompositionSample {
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Focus,
                        component: None,
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Family(AttributeValue::Normalized(value)),
            rank(order),
            mix,
        )
        .unwrap()
        .into()
    }
    fn playback(master: f32, crossfade_non_intensity: bool) -> DynamicPlaybackControl {
        DynamicPlaybackControl {
            identity: light_playback::PlaybackIdentity::physical(1).unwrap(),
            master,
            crossfade_non_intensity,
            auto_off_full_control: false,
            temporary_only: false,
        }
    }
    fn compose(
        samples: &[FamilyCompositionSample],
        playbacks: &HashMap<Uuid, DynamicPlaybackControl>,
        cues: &HashMap<Uuid, CueDynamicOutputControl>,
    ) -> f32 {
        let controls =
            CapturedFamilyEndpointControls::new(CapturedDynamicOutputControls { playbacks, cues })
                .unwrap();
        let control = |rank| controls.control_for(rank);
        let current = Current(FixtureId::new());
        let context = FamilyCompositionContext {
            endpoint_output: Some(FamilyEndpointOutputContext {
                control: &control,
                target: current.0,
                current: &current,
                native_models: None,
            }),
            ..Default::default()
        };
        compose_retained_dynamic_family(
            ProgrammingOwner::Focus,
            &AttributeValue::Normalized(0.2),
            samples,
            &context,
            &NoConversion,
            &mut RetainedFamilyCompositionScratch::default(),
        )
        .unwrap()
        .normalized()
        .unwrap()
    }

    #[test]
    fn captured_master_controls_endpoint_before_activation_over_a_different_dynamic_underlay() {
        // Reuse the same retained samples as a paused controller would. Only output controls
        // change; a mistaken activation*=master implementation yields different results.
        let samples = vec![focus(1.0, 1, 1.0), focus(0.8, 2, 0.5)];
        let controller = rank(2).dynamic_identity().unwrap().controller_id;
        for (master, crossfade, expected) in [
            (0.5, true, 0.75),
            (0.0, true, 0.6),
            (1.0, true, 0.9),
            (0.0, false, 1.0),
            (0.5, false, 0.9),
        ] {
            let result = compose(
                &samples,
                &HashMap::from([(controller, playback(master, crossfade))]),
                &HashMap::new(),
            );
            assert!(
                (result - expected).abs() < 0.00001,
                "master={master}, crossfade={crossfade}: {result}"
            );
        }
    }

    #[test]
    fn cue_output_gate_and_fixed_masks_preserve_their_distinct_ownership() {
        let mut samples = vec![focus(1.0, 1, 1.0), focus(0.8, 2, 0.5)];
        let controller = rank(2).dynamic_identity().unwrap().controller_id;
        let mut cues = HashMap::from([(
            controller,
            CueDynamicOutputControl {
                enabled: false,
                sequence_master: 0.5,
            },
        )]);
        assert_eq!(compose(&samples, &HashMap::new(), &cues), 1.0);
        cues.get_mut(&controller).unwrap().enabled = true;
        cues.get_mut(&controller).unwrap().sequence_master = 0.0;
        assert!((compose(&samples, &HashMap::new(), &cues) - 0.9).abs() < 0.00001);

        let fixed = ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Focus,
            None,
            AttributeValue::Normalized(0.4),
        )
        .unwrap();
        samples.push(
            fixed
                .compile(
                    None,
                    &FamilyEditContext::default(),
                    FamilySampleRank {
                        identity: FamilySampleIdentity::Fixed {
                            source: FamilyFixedSampleSource::Cue,
                            row_index: 0,
                        },
                        ..rank(3)
                    },
                    1.0,
                )
                .unwrap()
                .into(),
        );
        assert_eq!(
            compose(
                &samples,
                &HashMap::from([(controller, playback(0.0, false))]),
                &cues
            ),
            0.4
        );
    }

    #[test]
    fn invalid_captured_master_is_rejected_even_when_crossfade_is_disabled() {
        for master in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            let playbacks = HashMap::from([(Uuid::from_u128(202), playback(master, false))]);
            assert!(
                CapturedFamilyEndpointControls::new(CapturedDynamicOutputControls {
                    playbacks: &playbacks,
                    cues: &HashMap::new(),
                })
                .is_err()
            );
        }
    }
}
