//! Owned show activation: request cancellation may cancel before admission, never its commit tail.
use super::*;
use std::sync::atomic::{AtomicU8, Ordering};

const PENDING: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTING: u8 = 2;
const FINISHED: u8 = 3;

pub(in crate::runtime) enum ActivationCompletion {
    Open {
        previous: Option<ShowEntry>,
    },
    CleanDefault {
        previous: Option<ShowEntry>,
    },
    Rollback {
        previous: Option<ShowEntry>,
    },
    RevisionCopy {
        previous: Option<ShowEntry>,
        source: RevisionCopySource,
    },
    Mvr {
        imported: usize,
        unresolved: usize,
    },
}

impl ActivationCompletion {
    fn previous_id(&self, destination: light_core::ShowId) -> Option<light_core::ShowId> {
        match self {
            Self::Open { previous }
            | Self::CleanDefault { previous }
            | Self::Rollback { previous } => previous.as_ref().map(|show| show.id),
            Self::RevisionCopy { previous, .. } => previous
                .as_ref()
                .map(|show| show.id)
                .filter(|id| *id != destination),
            Self::Mvr { .. } => None,
        }
    }
    fn publish(self, state: &AppState, show: &ShowEntry, transition: &Transition) {
        match self {
            Self::Open { previous } => emit(
                state,
                "show_opened",
                serde_json::json!({"show":show,"transition":transition,"previous_show":previous}),
            ),
            Self::CleanDefault { previous } => emit(
                state,
                "show_opened",
                serde_json::json!({"show":show,"transition":transition,"previous_show":previous,"source":"built_in_default"}),
            ),
            Self::Rollback { .. } => emit(
                state,
                "show_rolled_back",
                serde_json::json!({"show":show,"transition":transition}),
            ),
            Self::RevisionCopy { source, .. } => emit(
                state,
                "show_opened",
                serde_json::json!({"show":show,"revision_copy":source,"transition":transition}),
            ),
            Self::Mvr {
                imported,
                unresolved,
            } => emit(
                state,
                "mvr_imported",
                serde_json::json!({"show":show,"fixtures":imported,"unresolved":unresolved,"scenery":0}),
            ),
        };
    }
}

// Field drop order removes the overlay before releasing activation/workflow exclusion.
struct ActivationResources {
    lease: capability_resources::OutputTransitionLease,
    activation: Option<capability_resources::ActiveShowPermit>,
    _show_change: tokio::sync::OwnedMutexGuard<()>,
}

struct Admission {
    phase: AtomicU8,
    cancelled: tokio::sync::Notify,
}

// Only the v2 library lifecycle boundary sets this scope. Read it before spawning the
// activation worker: Tokio task locals intentionally do not inherit across spawned tasks.
tokio::task_local! { static REQUEST_ADMISSION: Arc<Admission>; }

/// Keep lifecycle replay completion alive after an admitted request disconnects. The work
/// owns the library replay gate and inserts the outcome before releasing it. Cancellation
/// still reaches the exact activation CAS, including while the action imports/prepares a file.
pub(super) async fn await_owned_activation_action<T: Send + 'static>(
    work: impl std::future::Future<Output = Result<T, ApiError>> + Send + 'static,
) -> Result<T, ApiError> {
    let admission = Arc::new(Admission {
        phase: AtomicU8::new(PENDING),
        cancelled: Default::default(),
    });
    let request = CancelOnRequestDrop(admission.clone());
    let worker = tokio::spawn(REQUEST_ADMISSION.scope(admission, work));
    let result = worker
        .await
        .map_err(|error| ApiError::internal(format!("show lifecycle task failed: {error}")))?;
    drop(request);
    result
}

pub(super) fn check_activation_request_cancelled() -> Result<(), ApiError> {
    if REQUEST_ADMISSION
        .try_with(|admission| admission.phase.load(Ordering::Acquire) == CANCELLED)
        .unwrap_or(false)
    {
        return Err(cancelled());
    }
    Ok(())
}

/// Cancellation signals admission only. The owned task retains all resources until its
/// blocking worker has actually returned, including when that worker was queued at cancellation.
struct CancelOnRequestDrop(Arc<Admission>);
impl Drop for CancelOnRequestDrop {
    fn drop(&mut self) {
        if self
            .0
            .phase
            .compare_exchange(PENDING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.0.cancelled.notify_one();
        }
    }
}

fn cancelled() -> ApiError {
    ApiError::conflict("show activation was cancelled before commit admission")
}

impl Admission {
    async fn sleep(&self, duration: Duration) -> Result<(), ApiError> {
        if self.phase.load(Ordering::Acquire) == CANCELLED {
            return Err(cancelled());
        }
        tokio::select! {
            _ = tokio::time::sleep(duration) => Ok(()),
            _ = self.cancelled.notified() => Err(cancelled()),
        }
    }
}

struct ActivationDestination {
    entry: ShowEntry,
    runtime: PersistedOutputRuntime,
    completion: ActivationCompletion,
    prepared: capability_resources::PreparedOutputActivation,
    document: light_show::PortableShowDocument,
    configuration: PreparedShowConfiguration,
    media: std::collections::HashSet<light_core::FixtureId>,
}

/// Preparation has no live effects. The caller transfers its already-owned workflow guard;
/// after spawning it retains only a response receiver and a cancellation signal.
#[allow(clippy::too_many_arguments)]
pub(super) async fn activate_prepared_show(
    state: &AppState,
    prepared: PreparedShowActivation<PreparedOutputSnapshot>,
    context: &light_application::ActionContext,
    transition: &Transition,
    duration: Option<u64>,
    entry: ShowEntry,
    runtime: PersistedOutputRuntime,
    completion: ActivationCompletion,
    show_change: tokio::sync::OwnedMutexGuard<()>,
) -> Result<ShowEntry, ApiError> {
    check_activation_request_cancelled()?;
    runtime
        .validate_for_support(state.output.supported_programming_contract())
        .map_err(|error| ApiError::internal(error.to_string()))?;
    // Legacy normalization must happen inside the same retained Programmer capture as final
    // owner preparation. Startup's independently-normalizing wrapper is not appropriate here.
    let checkpoint = runtime
        .dynamic_source_checkpoint()
        .map_err(|error| ApiError::internal(error.to_string()))?
        .unwrap_or(dynamic_source_origins::DynamicRuntimeSourceCheckpoint {
            runtime: Default::default(),
            origins: None,
        });
    let (prepared, document, configuration) = prepared.into_parts();
    if document.id() != entry.id {
        return Err(ApiError::conflict(
            "prepared activation belongs to another show",
        ));
    }
    let media = prepared
        .snapshot()
        .fixtures
        .iter()
        .filter(|fixture| fixture.direct_control.is_some())
        .map(|fixture| fixture.fixture_id)
        .collect();
    let prepared = state
        .output
        .prepare_destination_activation_with_controls(
            prepared,
            checkpoint,
            &runtime.dynamic_playbacks,
            runtime.dynamics_paused_at,
            &runtime.group_masters,
        )
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let destination = ActivationDestination {
        entry,
        runtime,
        completion,
        prepared,
        document,
        configuration,
        media,
    };
    let lease = match transition {
        Transition::HoldCurrent => state.output.begin_transition_hold(),
        Transition::SafeBlackout => state.output.begin_transition_blackout(),
        Transition::TimedFade => state.output.begin_transition_fade(),
    };
    let resources = ActivationResources {
        lease,
        activation: None,
        _show_change: show_change,
    };
    let admission = REQUEST_ADMISSION.try_with(Arc::clone).unwrap_or_else(|_| {
        Arc::new(Admission {
            phase: AtomicU8::new(PENDING),
            cancelled: Default::default(),
        })
    });
    let request = CancelOnRequestDrop(admission.clone());
    let state = state.clone();
    let context = context.clone();
    let transition = transition.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let result = run_activation_workflow(
            state,
            context,
            transition,
            duration,
            destination,
            resources,
            admission.clone(),
        )
        .await;
        admission.phase.store(FINISHED, Ordering::Release);
        let _ = send.send(result);
    });
    let result = receive
        .await
        .map_err(|_| ApiError::internal("owned show activation task failed"))?;
    drop(request);
    result
}

async fn run_activation_workflow(
    state: AppState,
    context: light_application::ActionContext,
    transition: Transition,
    duration: Option<u64>,
    destination: ActivationDestination,
    mut resources: ActivationResources,
    admission: Arc<Admission>,
) -> Result<ShowEntry, ApiError> {
    let frame = Duration::from_millis(25);
    let fade_sleep =
        Duration::from_millis((duration.unwrap_or(1000).clamp(100, 30000) / 40).max(1));
    match transition {
        Transition::HoldCurrent => {}
        Transition::SafeBlackout => admission.sleep(frame * 2).await?,
        Transition::TimedFade => {
            for step in 1..=20 {
                resources.lease.set_fade_gain(1.0 - step as f32 / 20.);
                admission.sleep(fade_sleep).await?;
            }
        }
    }
    if admission.phase.load(Ordering::Acquire) == CANCELLED {
        return Err(cancelled());
    }
    #[cfg(test)]
    {
        let probe_state = state.active_show.clone();
        tokio::task::spawn_blocking(move || probe_state.pause_activation_before_commit_if_armed())
            .await
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    resources.activation = Some(tokio::select! {
        permit = state.active_show.acquire() => permit,
        _ = admission.cancelled.notified() => return Err(cancelled()),
    });
    let worker_transition = transition.clone();
    let (entry, resources) = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        let _completed = CompletedProbe(state.active_show.clone());
        #[cfg(test)]
        state
            .active_show
            .pause_activation_before_admission_if_armed();
        admission
            .phase
            .compare_exchange(PENDING, COMMITTING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| cancelled())?;
        #[cfg(test)]
        state
            .active_show
            .pause_activation_after_admission_if_armed();
        let ActivationDestination {
            entry,
            runtime,
            completion,
            prepared,
            document,
            configuration,
            media,
        } = destination;
        let previous_id = completion.previous_id(entry.id);
        let receipt = state
            .output
            .engine()
            .with_retained_dynamic_programmer_source(|_| {
                let receipt = state
                    .programming
                    .run_selection_refresh(&context, || {
                        let receipt = state
                            .output
                            .install_destination_activation_with_commit(
                                &state.playback.render_capability(),
                                prepared,
                                || {
                                    state
                                        .installation
                                        .record_show_activation(entry.id, previous_id)
                                        .map_err(|error| {
                                            light_core::programming::IntentError(error.to_string())
                                        })
                                },
                            )
                            .map_err(|error| ApiError::internal(error.to_string()))?;
                        state
                            .programming
                            .clear_pending_command_choices_except_context(None);
                        Ok::<_, ApiError>(receipt)
                    })
                    .output?;
                // An Err above leaves gesture/Align, Highlight, media and health untouched.
                // Finish Align after selection publication; finish_alignment publishes its own event.
                state.programming.run_value_gesture_boundary(&context, || {
                    state.highlight.clear_all();
                    state.output.clear_highlighted_fixtures();
                    state.output.reset_show_output_health();
                    state.media.retain_fixtures(&media);
                    invalidate_active_show_document(&state);
                    state.active_show.replace_current(Some(receipt.clone()));
                    state.active_show.document_cache().replace(Some(document));
                    configuration.install_memory_only(&state);
                    state.active_show.set_error(None);
                    // Playback, Group Masters and pause were prepared in the same private generation.
                    // This merge runs after publication/Playback/Dynamics guards have been released.
                    resources.lease.restore_destination_control(&runtime);
                    state.output.clear_runtime_replay();
                });
                Ok::<_, ApiError>(receipt)
            })?;
        state.extensions.refresh_feedback_snapshots();
        // TL-548 C4: every activation, including a same-show reload, is a new Pending nonce.
        state
            .programming
            .pending_episodes()
            .trigger(output_scheduler::PendingTrigger::Reload);
        completion.publish(&state, &receipt, &worker_transition);
        drop(resources.activation.take());
        Ok::<_, ApiError>((receipt, resources))
    })
    .await
    .map_err(|error| ApiError::internal(format!("show activation worker failed: {error}")))??;
    // The request can disappear after admission without skipping completion or fade cleanup.
    // Rendering is no longer excluded while the incoming show fades up.
    match transition {
        Transition::TimedFade => {
            for step in 1..=20 {
                resources.lease.set_fade_gain(step as f32 / 20.);
                tokio::time::sleep(fade_sleep).await;
            }
        }
        _ => tokio::time::sleep(frame).await,
    }
    drop(resources);
    Ok(entry)
}

/// Removes a destination the caller created for this activation (clean default, revision copy)
/// when activation returned an error, matching the callers' preparation-failure cleanup.
/// Reacquires show-change so no other activation of the copy can be in flight, and keeps a copy
/// that is live or durably active, which only a committed activation can make it.
pub(super) async fn discard_unactivated_destination(state: &AppState, entry: &ShowEntry) {
    let _show_change = state.active_show.acquire_show_change().await;
    let live = state
        .active_show
        .current()
        .is_some_and(|show| show.id == entry.id);
    if live {
        return;
    }
    let durable = match state.installation.active_show() {
        Ok(active) => active.is_some_and(|show| show.id == entry.id),
        Err(error) => {
            tracing::warn!(%error, show_id = %entry.id.0, "Preserving activation copy: active identity could not be read");
            return;
        }
    };
    if durable {
        return;
    }
    if let Err(error) = state.installation.remove_show(entry.id) {
        tracing::warn!(%error, show_id = %entry.id.0, "Preserving activation copy: library entry could not be removed");
        return;
    }
    let _ = std::fs::remove_file(&entry.path);
}

#[cfg(test)]
struct CompletedProbe(capability_resources::ActiveShowResource);
#[cfg(test)]
impl Drop for CompletedProbe {
    fn drop(&mut self) {
        self.0.pause_activation_completed_if_armed();
    }
}
