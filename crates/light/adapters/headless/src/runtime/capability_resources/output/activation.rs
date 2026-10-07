//! Detached destination activation. Production transition wiring is deliberately separate.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceBinding;
use light_application::PlaybackOperation;
use light_programmer::ActiveDynamicSessionSource;

/// A single-use destination pair, reconciled against exact retained Programmer authority.
/// No guard is retained; outgoing frames may run while this token is held.
#[must_use]
pub(in crate::runtime) struct PreparedOutputActivation {
    owner: Arc<Engine>,
    publication: Arc<DynamicSnapshotPublication>,
    dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    base_snapshot: Arc<EngineSnapshot>,
    authority: Option<ActiveDynamicSessionSource>,
    engine: light_engine::FinalizedEngineSnapshot,
    restored: output_scheduler::RestoredDynamicCandidate,
}

fn same_authority(
    expected: &Option<ActiveDynamicSessionSource>,
    actual: &Option<ActiveDynamicSessionSource>,
) -> bool {
    match (expected, actual) {
        (None, None) => true,
        (Some((a, ap, an, aa)), Some((b, bp, bn, ba))) => {
            a == b && ap == bp && Arc::ptr_eq(an, bn) && Arc::ptr_eq(aa, ba)
        }
        _ => false,
    }
}

/// Instance links of every retained Programmer row, captured under the activation authority.
fn retained_programmer_links(
    authority: &Option<ActiveDynamicSessionSource>,
) -> Vec<(light_core::ProgrammerId, Uuid)> {
    authority
        .as_ref()
        .into_iter()
        .flat_map(|(owner, _, normal, active)| {
            normal.iter().chain(active.iter()).filter_map(move |row| {
                row.value
                    .track_key()
                    .instance_link
                    .map(|link| (light_core::ProgrammerId(*owner), link))
            })
        })
        .collect::<Vec<_>>()
}

/// Rebinds restored Playback controllers to their finalized destination owner and priority.
fn rebind_restored_playback_controllers(
    engine: &light_engine::FinalizedEngineSnapshot,
    snapshot: &EngineSnapshot,
    checkpoint: &mut DynamicRuntimeSourceCheckpoint,
) -> Result<(), IntentError> {
    let playback_owners = engine
        .dynamic_playbacks()
        .iter()
        .map(|active| {
            let target = active
                .dynamic_id
                .expect("finalized restored Dynamic Playback has a stable target identity");
            // Use the assignment selected by destination restoration, including
            // the virtual page; another assignment of the same target may differ.
            let playback = match active.playback_identity {
                Some(light_playback::PlaybackIdentity::Virtual(address)) => snapshot
                    .playback_pages
                    .iter()
                    .find(|page| page.number == address.page())
                    .and_then(|page| page.virtual_playbacks.get(&address.number().get())),
                _ => snapshot
                    .playbacks
                    .iter()
                    .find(|playback| playback.number == active.playback_number),
            }
            .ok_or_else(|| {
                IntentError("finalized Dynamic Playback assignment is missing".into())
            })?;
            let light_playback::PlaybackTarget::Dynamic { assignment } = &playback.target else {
                return Err(IntentError(
                    "finalized Playback assignment is not Dynamic".into(),
                ));
            };
            Ok((
                light_playback::dynamic_playback_controller_id(target),
                (
                    output_scheduler::dynamic_playback_owner(active),
                    assignment.priority,
                ),
            ))
        })
        .collect::<Result<HashMap<_, _>, IntentError>>()?;
    for controller in checkpoint
        .runtime
        .instances
        .iter_mut()
        .flat_map(|instance| &mut instance.controllers)
    {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Playback { .. }
        ) {
            if let Some((owner, priority)) = playback_owners.get(&controller.id) {
                controller.source = owner.clone();
                controller.priority = *priority;
            }
        }
    }
    Ok(())
}

/// Drops authored source-origin bindings whose instance controller did not survive restore.
fn retain_active_origin_bindings(restored: &mut output_scheduler::RestoredDynamicCandidate) {
    let projection = restored.runtime.output_projection_snapshot();
    let active = projection
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .controllers
                .iter()
                .map(move |controller| (instance.id, controller.id))
        })
        .collect::<std::collections::HashSet<_>>();
    restored
        .origins
        .retain_bindings(|record| match record.binding {
            DynamicSourceBinding::Authored {
                instance_id,
                controller_id,
                ..
            } => active.contains(&(instance_id, controller_id)),
            DynamicSourceBinding::Fixed { .. } | DynamicSourceBinding::StaticBaseline { .. } => {
                true
            }
        });
}

impl OutputResource {
    /// Normalize and prepare a raw checkpoint against finalized
    /// destination Playback owners. Release policy and saved-row normalization happen before
    /// strict reconciliation; no outgoing Playback or Dynamic state is sampled or installed.
    ///
    /// Retained authority intentionally includes disconnected Programmer state. Pending Preload
    /// rows and connection status are not owners under this policy. Exact normal/active Arc
    /// identities, Programmer identity and priority are captured under the actual Engine's gate.
    /// A later commit rejects changed authority; equal row values in new Arcs are still stale.
    #[cfg(test)]
    pub(in crate::runtime) fn prepare_destination_activation(
        &self,
        prepared: PreparedOutputSnapshot,
        checkpoint: DynamicRuntimeSourceCheckpoint,
        saved_playbacks: &[light_playback::ActiveDynamicPlayback],
        paused_since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<PreparedOutputActivation, IntentError> {
        self.prepare_destination_activation_with_controls(
            prepared,
            checkpoint,
            saved_playbacks,
            paused_since,
            &HashMap::new(),
        )
    }

    pub(in crate::runtime) fn prepare_destination_activation_with_controls(
        &self,
        prepared: PreparedOutputSnapshot,
        mut checkpoint: DynamicRuntimeSourceCheckpoint,
        saved_playbacks: &[light_playback::ActiveDynamicPlayback],
        paused_since: Option<chrono::DateTime<chrono::Utc>>,
        group_masters: &HashMap<String, f32>,
    ) -> Result<PreparedOutputActivation, IntentError> {
        self.engine
            .with_retained_dynamic_programmer_source(|authority| {
                let base_snapshot = self.engine.snapshot();
                let PreparedOutputSnapshot {
                    engine, restored, ..
                } = prepared;
                if restored.is_some() {
                    return Err(IntentError(
                        "destination activation requires a raw checkpoint".into(),
                    ));
                }
                // Normalization must use this same authority capture. An earlier missing link
                // followed by Undo could otherwise discard a legacy owner's held/Random history.
                let links = retained_programmer_links(&authority);
                light_dynamics::normalize_legacy_programmer_controller_ids(
                    &mut checkpoint.runtime,
                    &links,
                )
                .map_err(|error| IntentError(error.to_string()))?;
                let mut engine = self
                    .engine
                    .finalize_snapshot_playback_restoring_dynamics(
                        engine,
                        saved_playbacks,
                        paused_since,
                    )
                    .map_err(|error| IntentError(error.to_string()))?;
                // Group Master levels and physical fader pickup belong to the same private
                // generation as restored Playback, before any owner capture or publication.
                engine
                    .prepare_group_masters(group_masters)
                    .map_err(|error| IntentError(error.to_string()))?;
                let snapshot = engine.snapshot_arc();
                // Restore normalizes each retained Playback row to a destination assignment
                // and supplies its stable target ID. Existing controllers survive by that ID;
                // their operational address and priority follow the finalized owner before restore.
                // Historical source-origin records remain the original immutable evidence.
                rebind_restored_playback_controllers(&engine, &snapshot, &mut checkpoint)?;
                let mut restored = output_scheduler::prepare_restored_dynamic_candidate(
                    &snapshot,
                    &self.dynamics.lock(),
                    checkpoint,
                )
                .map_err(|error| IntentError(error.to_string()))?;
                let (programmer_values, _) =
                    self.engine.captured_dynamic_programmer_values_from_sources(
                        authority.clone().into_iter().collect(),
                    );
                restored.runtime.set_definitions_pinned(false);
                output_scheduler::reconcile_cold_dynamic_candidate(
                    &mut restored.runtime,
                    output_scheduler::ColdDynamicReconciliationInputs {
                        captured_at_millis: u64::try_from(engine.sampled_at().timestamp_millis())
                            .unwrap_or_default(),
                        snapshot: &snapshot,
                        programmer_values: &programmer_values,
                        extra_programmer_values: &[],
                        cue_values: engine.cue_dynamic_values(),
                        playbacks: engine.dynamic_playbacks(),
                        playback_paused: engine.playback_dynamics_paused(),
                    },
                )
                .map_err(|error| IntentError(error.to_string()))?;
                // Reconciliation may start/retarget owners after the initial checkpoint preflight.
                output_scheduler::materialize_pending_preset_dependencies(
                    &snapshot,
                    &mut restored.runtime,
                )
                .map_err(|error| IntentError(error.to_string()))?;
                retain_active_origin_bindings(&mut restored);
                Ok(PreparedOutputActivation {
                    owner: self.engine.clone(),
                    publication: self.dynamic_snapshot.clone(),
                    dynamics: self.dynamics.clone(),
                    base_snapshot,
                    authority,
                    engine,
                    restored,
                })
            })
    }

    /// Caller owns active-show exclusion and supplies this desk's Playback capability.
    /// This synchronous seam acquires the actual Programmer
    /// gate, then publication -> ordered Playback -> Dynamics, and retains them through install.
    /// It must not be entered from an already ordered Playback operation. No guard crosses await.
    ///
    /// A token belongs to one Engine and one outgoing snapshot identity. Snapshot replacement
    /// invalidates it; ordinary outgoing controls/frames do not, because activation deliberately
    /// replaces their state with the saved destination. All rejection precedes watermark/origin
    /// changes. The final tail has no fallible validation, Playback restore or callback.
    ///
    /// Transition effects, Highlight, media, selection refresh policy, persistence and health
    /// remain the subsequent production wiring's responsibility. This does not roll those back.
    pub(in crate::runtime) fn install_destination_activation(
        &self,
        playback: &PlaybackRenderCapability,
        prepared: PreparedOutputActivation,
    ) -> Result<(), IntentError> {
        self.install_destination_activation_with_commit(playback, prepared, || Ok(()))
    }

    /// Runs atomic durable activation bookkeeping after all authority checks, before runtime
    /// publication. The callback must only commit prepared metadata: it must not read or mutate
    /// Programmer, Playback, Dynamics, output controls or activation state, nor acquire their
    /// locks. It must roll back its own storage transaction on Err. No callback runs after the
    /// runtime swap, and no fallible operation follows a successful callback.
    ///
    /// The caller must already own commit admission and workflow exclusion independently of
    /// request cancellation. This synchronous helper cannot establish that async ownership.
    pub(in crate::runtime) fn install_destination_activation_with_commit<T>(
        &self,
        playback: &PlaybackRenderCapability,
        prepared: PreparedOutputActivation,
        commit_metadata: impl FnOnce() -> Result<T, IntentError>,
    ) -> Result<T, IntentError> {
        self.engine
            .with_retained_dynamic_programmer_source(|authority| {
                if !Arc::ptr_eq(&prepared.owner, &self.engine)
                    || !Arc::ptr_eq(&prepared.publication, &self.dynamic_snapshot)
                    || !Arc::ptr_eq(&prepared.dynamics, &self.dynamics)
                {
                    return Err(IntentError(
                        "destination activation belongs to another Engine".into(),
                    ));
                }
                if !same_authority(&prepared.authority, &authority) {
                    return Err(IntentError(
                        "destination activation Programmer authority is stale".into(),
                    ));
                }
                let _publication = self.dynamic_snapshot.begin_install();
                output_scheduler::ordered_output_operation(playback, || {
                    PlaybackOperation::new(
                        self.install_destination_activation_ordered(prepared, commit_metadata),
                    )
                })
            })
    }

    fn install_destination_activation_ordered<T>(
        &self,
        mut prepared: PreparedOutputActivation,
        commit_metadata: impl FnOnce() -> Result<T, IntentError>,
    ) -> Result<T, IntentError> {
        // Take Playback data before Dynamics, as in existing cold finalization.
        let current_snapshot = self.engine.snapshot();
        if !Arc::ptr_eq(&prepared.base_snapshot, &current_snapshot) {
            return Err(IntentError(
                "destination activation Engine snapshot is stale".into(),
            ));
        }
        let watermark = self
            .engine
            .playback_source_occurrence_watermark()
            .max(prepared.restored.playback_source_occurrence_watermark);
        let mut live = self.dynamics.lock();
        if !self.dynamic_snapshot.matches(&current_snapshot) {
            return Err(IntentError(
                "destination activation Dynamic registry is stale".into(),
            ));
        }
        let snapshot = prepared.engine.snapshot_arc();
        let origins = Arc::new(prepared.restored.origins);
        // Last returned-error boundary. All authority and registry checks precede the durable
        // transaction; every operation after it succeeds is an owned in-memory publication.
        let committed = commit_metadata()?;
        // Reserve on the PRIVATE destination Playback. Reserving only the outgoing Engine
        // would lose the reservation when the exact finalized generation is swapped in.
        prepared
            .engine
            .reserve_playback_source_occurrence_watermark(watermark);
        self.engine.install_finalized_snapshot(prepared.engine);
        *live = prepared.restored.runtime;
        self.dynamic_source_origins.store(origins);
        self.dynamic_snapshot.installed(snapshot);
        drop(live);
        // TL-548 C3: a new show starts with new Live family lanes (taken after `dynamics`).
        self.family_adapters.reset();
        Ok(committed)
    }
}

#[cfg(test)]
mod tests;
