//! Cold validation of destination Dynamic controller reconciliation.
//!
//! A portable-show edit changes Dynamic definitions, Groups, stage positions and Playback rows at
//! once. Warm reconciliation tolerates transient failures frame by frame and discards them. A cold
//! installation must not: publishing a runtime in which some controller mutation silently failed
//! would leave a partially reconciled Dynamic state behind the operator's back.
//!
//! [`reconcile_cold_dynamic_candidate`] runs the same three reconciliation flows (Programmer, Cue,
//! Playback) against a detached candidate and observes every result the warm path discards. It
//! never samples, ticks, publishes sources or touches Live; the caller forks the candidate
//! (`DynamicRuntime::fork_for_cold_install`), installs the destination definitions and native
//! setup, calls this helper, and discards the candidate on error.

use super::*;

/// Which of the three shared reconciliation flows observed the result.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) enum ColdDynamicReconciliationFlow {
    Programmer,
    Cue,
    Playback,
}

/// The logical controller whose reconciliation produced a failure or a passive requirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdDynamicReconciliationContext {
    pub flow: ColdDynamicReconciliationFlow,
    pub controller_id: Uuid,
    pub source: light_dynamics::DynamicControllerSource,
    /// Page-qualified identity of a Playback row, when the row carries one.
    pub playback_identity: Option<PlaybackIdentity>,
}

impl ColdDynamicReconciliationContext {
    pub(super) fn released(
        flow: ColdDynamicReconciliationFlow,
        controller: &light_dynamics::DynamicController,
    ) -> Self {
        Self {
            flow,
            controller_id: controller.id,
            source: controller.source.clone(),
            playback_identity: None,
        }
    }

    pub(super) fn programmer(
        controller_id: Uuid,
        programmer_id: Uuid,
        instance_link: Uuid,
    ) -> Self {
        Self {
            flow: ColdDynamicReconciliationFlow::Programmer,
            controller_id,
            source: light_dynamics::DynamicControllerSource::Programmer {
                programmer_id,
                instance_link: Some(instance_link),
            },
            playback_identity: None,
        }
    }

    pub(super) fn cue(controller_id: Uuid, cue_list_id: Uuid, instance_link: Uuid) -> Self {
        Self {
            flow: ColdDynamicReconciliationFlow::Cue,
            controller_id,
            source: light_dynamics::DynamicControllerSource::Cue {
                cue_list_id,
                instance_link,
            },
            playback_identity: None,
        }
    }

    pub(super) fn playback(
        controller_id: Uuid,
        playback: &light_playback::ActiveDynamicPlayback,
    ) -> Self {
        Self {
            flow: ColdDynamicReconciliationFlow::Playback,
            controller_id,
            source: dynamic_playback_owner(playback),
            playback_identity: playback.playback_identity,
        }
    }
}

/// The runtime mutation or resolution step that failed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) enum ColdDynamicReconciliationOperation {
    Release,
    CancelRelease,
    Rank,
    Owner,
    LaneSelection,
    Targets,
    Controller,
    Pause,
    OutputGate,
    FallbackDefinition,
    Start,
    GroupResolution,
    PlaybackRow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ColdDynamicReconciliationCause {
    /// A Dynamic runtime mutation rejected the destination state.
    Runtime(light_dynamics::DynamicRuntimeError),
    /// The top-level Group exists but its nested or spatial resolution failed. `resolver` is
    /// the Group resolver's own context, preserved verbatim rather than parsed.
    InvalidGroup { group_id: String, resolver: String },
    /// An enabled Playback row names no destination Playback definition.
    MissingPlaybackDefinition,
    /// An enabled Dynamic Playback row names a destination Playback that is not a Dynamic.
    NonDynamicPlaybackTarget,
    /// An enabled Playback row without page identity has an invalid physical number.
    InvalidPlaybackNumber(String),
    /// Start reported an empty scope although the resolved destination scope has targets.
    UnexpectedEmptyTargets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdDynamicReconciliationFailure {
    pub context: ColdDynamicReconciliationContext,
    pub operation: ColdDynamicReconciliationOperation,
    pub cause: ColdDynamicReconciliationCause,
}

/// Why a controller currently resolves no targets. None of these block installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ColdDynamicScopeReason {
    /// The referenced top-level Group is absent from the destination show.
    MissingGroup { group_id: String },
    /// The referenced Group exists and was deliberately stored empty.
    EmptyGroup { group_id: String },
    /// Frozen or Playback target scope is genuinely empty.
    EmptyScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdDynamicScopeRequirement {
    pub context: ColdDynamicReconciliationContext,
    pub reason: ColdDynamicScopeReason,
}

/// Successful cold reconciliation. The candidate now holds the reconciled controllers; the
/// passive scope requirements describe controllers that are retained or skipped with no targets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct ColdDynamicReconciliation {
    pub scope_requirements: Vec<ColdDynamicScopeRequirement>,
}

/// Rejected cold reconciliation. Every observed failure is listed in flow order; the candidate
/// is partially reconciled and must be discarded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdDynamicReconciliationError {
    pub failures: Vec<ColdDynamicReconciliationFailure>,
    pub scope_requirements: Vec<ColdDynamicScopeRequirement>,
}

impl std::fmt::Display for ColdDynamicReconciliationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Dynamic controller reconciliation rejected {} mutation(s)",
            self.failures.len()
        )?;
        if let Some(first) = self.failures.first() {
            write!(
                formatter,
                "; first: {:?} {:?} of controller {} ({:?})",
                first.context.flow, first.operation, first.context.controller_id, first.cause
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ColdDynamicReconciliationError {}

/// Finalized destination inputs, all captured at one instant.
pub(in crate::runtime) struct ColdDynamicReconciliationInputs<'a> {
    /// Captured application time. A retry after a failed candidate must reuse the same value so
    /// new controllers keep their authored activation time.
    pub captured_at_millis: u64,
    /// Destination Engine snapshot: Dynamic definitions, Groups, stage positions and Playbacks.
    pub snapshot: &'a light_engine::EngineSnapshot,
    pub programmer_values: &'a [(Uuid, i16, light_dynamics::DynamicAddressValue)],
    pub extra_programmer_values: &'a [(Uuid, i16, light_dynamics::DynamicAddressValue)],
    pub cue_values: &'a [light_playback::ActiveCueDynamicValue],
    pub playbacks: &'a [light_playback::ActiveDynamicPlayback],
    pub playback_paused: bool,
}

/// Reconcile a detached Dynamic candidate against finalized destination rows and report every
/// failure the warm path would discard.
///
/// Runs the shared Programmer, Cue and Playback flows in the same order as warm action
/// reconciliation, reusing their release, cancel-release, rank, lane, target, controller, pause,
/// output-gate, fallback and start transitions. Absent top-level Groups, deliberately empty Groups
/// and genuinely empty scopes are passive and returned as requirements; start is still invoked so
/// invalid controller values are rejected. Native capability absence compiles to suspended lanes
/// and is therefore never a failure here.
///
/// `candidate` must be a fork (`fork_for_cold_install`) whose destination definitions and native
/// setup are already installed. This helper does not sample, tick, publish or touch Live. On
/// error the candidate is partially reconciled and must be discarded; retry with a fresh fork
/// and the same `captured_at_millis`.
pub(in crate::runtime) fn reconcile_cold_dynamic_candidate(
    candidate: &mut light_dynamics::DynamicRuntime,
    inputs: ColdDynamicReconciliationInputs<'_>,
) -> Result<ColdDynamicReconciliation, ColdDynamicReconciliationError> {
    let mut outcome = ColdOutcome::default();
    let mut observer = ReconciliationObserver {
        cold: Some(&mut outcome),
    };
    let now_millis = inputs.captured_at_millis;
    candidate
        .apply_recorded_control(light_dynamics::TimedDynamicControl {
            at_millis: now_millis,
            control: light_dynamics::DynamicControl::GlobalPause(inputs.playback_paused),
        })
        .expect("detached cold reconciliation is outside sampling");
    reconcile_programmer_dynamics_observed(
        candidate,
        now_millis,
        inputs.snapshot,
        inputs.programmer_values,
        inputs.extra_programmer_values,
        None,
        &mut observer,
    );
    reconcile_cue_dynamics_observed(
        candidate,
        now_millis,
        inputs.snapshot,
        inputs.cue_values,
        None,
        &mut observer,
    );
    reconcile_dynamic_playbacks_observed(
        candidate,
        now_millis,
        inputs.snapshot,
        inputs.playbacks,
        None,
        &mut observer,
    );
    if outcome.failures.is_empty() {
        Ok(ColdDynamicReconciliation {
            scope_requirements: outcome.scope_requirements,
        })
    } else {
        Err(ColdDynamicReconciliationError {
            failures: outcome.failures,
            scope_requirements: outcome.scope_requirements,
        })
    }
}

#[derive(Default)]
pub(super) struct ColdOutcome {
    failures: Vec<ColdDynamicReconciliationFailure>,
    scope_requirements: Vec<ColdDynamicScopeRequirement>,
}

/// Observes reconciliation results. Warm reconciliation discards them exactly as before; cold
/// reconciliation records failures and passive scope requirements.
pub(super) struct ReconciliationObserver<'r> {
    cold: Option<&'r mut ColdOutcome>,
}

impl ReconciliationObserver<'static> {
    pub(super) fn warm() -> Self {
        Self { cold: None }
    }
}

impl ReconciliationObserver<'_> {
    pub(super) fn is_cold(&self) -> bool {
        self.cold.is_some()
    }

    pub(super) fn observe<T>(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        operation: ColdDynamicReconciliationOperation,
        result: Result<T, light_dynamics::DynamicRuntimeError>,
    ) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.fail(
                    context,
                    operation,
                    ColdDynamicReconciliationCause::Runtime(error),
                );
                None
            }
        }
    }

    pub(super) fn fail(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        operation: ColdDynamicReconciliationOperation,
        cause: ColdDynamicReconciliationCause,
    ) {
        if let Some(outcome) = self.cold.as_deref_mut() {
            outcome.failures.push(ColdDynamicReconciliationFailure {
                context: context.clone(),
                operation,
                cause,
            });
        }
    }

    fn passive(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        reason: ColdDynamicScopeReason,
    ) {
        if let Some(outcome) = self.cold.as_deref_mut() {
            outcome
                .scope_requirements
                .push(ColdDynamicScopeRequirement {
                    context: context.clone(),
                    reason,
                });
        }
    }

    /// Warm reconciliation treats an unresolvable Group as no targets; cold reconciliation
    /// distinguishes absent, deliberately empty and invalid existing Groups.
    pub(super) fn group_scope(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        group_id: &str,
        scope: DynamicGroupScope,
    ) -> (
        Vec<FixtureId>,
        Option<light_dynamics::SpatialSelectionMapping>,
    ) {
        match scope {
            DynamicGroupScope::Resolved { targets, mapping } => {
                if targets.is_empty() {
                    self.passive(
                        context,
                        ColdDynamicScopeReason::EmptyGroup {
                            group_id: group_id.to_owned(),
                        },
                    );
                }
                (targets, mapping)
            }
            DynamicGroupScope::Missing => {
                self.passive(
                    context,
                    ColdDynamicScopeReason::MissingGroup {
                        group_id: group_id.to_owned(),
                    },
                );
                Default::default()
            }
            DynamicGroupScope::Invalid(resolver) => {
                self.fail(
                    context,
                    ColdDynamicReconciliationOperation::GroupResolution,
                    ColdDynamicReconciliationCause::InvalidGroup {
                        group_id: group_id.to_owned(),
                        resolver,
                    },
                );
                Default::default()
            }
        }
    }

    pub(super) fn explicit_scope(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        targets: Vec<FixtureId>,
    ) -> Vec<FixtureId> {
        if targets.is_empty() {
            self.passive(context, ColdDynamicScopeReason::EmptyScope);
        }
        targets
    }

    /// Start is always invoked so controller values are validated. `EmptyTargets` is passive only
    /// when the resolved destination scope is genuinely empty; its requirement is already recorded.
    pub(super) fn started(
        &mut self,
        context: &ColdDynamicReconciliationContext,
        scope_was_empty: bool,
        result: Result<Uuid, light_dynamics::DynamicRuntimeError>,
    ) -> Option<Uuid> {
        match result {
            Err(light_dynamics::DynamicRuntimeError::EmptyTargets) if scope_was_empty => None,
            Err(light_dynamics::DynamicRuntimeError::EmptyTargets) => {
                self.fail(
                    context,
                    ColdDynamicReconciliationOperation::Start,
                    ColdDynamicReconciliationCause::UnexpectedEmptyTargets,
                );
                None
            }
            result => self.observe(context, ColdDynamicReconciliationOperation::Start, result),
        }
    }
}

#[cfg(test)]
mod tests;
