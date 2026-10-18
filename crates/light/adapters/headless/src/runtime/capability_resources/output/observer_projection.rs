//! Fresh observational Dynamic projection; Live remains owned by the output scheduler.
use super::observer_projection_diagnostics::{ProjectionObservation, Stage};
use super::*;

impl OutputResource {
    pub(in crate::runtime) fn visualization_dynamic_projection(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        _projected: bool,
    ) -> (
        light_engine::ResolvedValues,
        light_dynamics::DynamicRuntimeSnapshot,
        Vec<light_dynamics::DynamicRuntimeSample>,
    ) {
        // Visualization is observational. Sampling can retire completed release
        // transitions, so always operate on a clone and leave the authoritative
        // output scheduler responsible for mutating and publishing runtime state.
        let mut observation = ProjectionObservation::begin();
        let snapshot = self.engine.snapshot();
        let mut visualization_runtime =
            light_dynamics::DynamicRuntime::with_programming_contract_support(
                self.engine.supported_programming_contract(),
            );
        observation
            .measure(Stage::InstallDefinitions, || {
                visualization_runtime.install_definitions(snapshot.dynamics.iter().cloned())
            })
            .expect("Engine snapshot contains validated Dynamic definitions");
        // The portable capture owns its state. Private restoration reads only that capture
        // and this observer's definitions; release Live before doing the cold validation.
        let portable = observation.measure(Stage::LiveCapture, || self.dynamics.lock().snapshot());
        #[cfg(test)]
        early_unlock_tests::after_capture(self, &portable);
        observation
            .measure(Stage::RestoreSnapshot, || {
                visualization_runtime.restore_snapshot(portable)
            })
            .expect("live Dynamic runtime snapshot remains restorable");
        let visualization_runtime = Mutex::new(visualization_runtime);
        let (sampled, runtime_snapshot, runtime_samples) =
            observation.measure(Stage::DynamicProjection, || {
                output_scheduler::observer_dynamic_projection(
                    &self.engine,
                    &visualization_runtime,
                    &self.speed_groups,
                    &self.rate,
                    extra_programmer_values,
                )
            });
        let resolved = observation.measure(Stage::ResolvedValues, || {
            self.engine
                .resolved_values_with_contribution_batches(&sampled)
        });
        // Preserve the original return-expression drop order: batches, then private runtime.
        observation.measure(Stage::Teardown, || {
            drop(sampled);
            drop(visualization_runtime);
        });
        (resolved, runtime_snapshot, runtime_samples)
    }
}

#[cfg(test)]
mod early_unlock_tests;
