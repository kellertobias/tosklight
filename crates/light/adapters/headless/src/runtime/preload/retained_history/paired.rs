//! Detached paired Pending attempts. Both branches follow the same immutable captures, but
//! keep independent controls, sampling history, native pins and source catalogues. No Live
//! capture, application lock, publication or cold-install authority is introduced here.
use super::*;

pub(in crate::runtime) struct PairedPendingHistory<T> {
    before: DetachedPendingHistory<()>,
    after: DetachedPendingHistory<()>,
    last_success: Option<PendingPairResult<T>>,
}

pub(in crate::runtime) struct PendingPairResult<T> {
    pub capture: Arc<RetainedInputCapture>,
    pub before_sample: Option<DynamicSampleBoundary>,
    pub after_sample: Option<DynamicSampleBoundary>,
    pub value: T,
}

/// Evaluate BOTH branches on every selected attempt, including when current output does not
/// need Before. The caller must perform each branch's complete whole-runtime sample/reconcile
/// pass (or preserve its marker when genuinely idle), then all fallible composition/encoding,
/// before returning Ok. Instance-only sampling is forbidden, as in PendingAttemptEvaluator.
///
/// Both runtimes are already in distinct output transactions. Do not nest another transaction,
/// cold-install, publish, or mutate external semantic state during fallible work. The supplied
/// origins are COW candidates. External continuity/result owners must stage their own state and
/// change it only in their infallible final commit tail; this coordinator cannot roll back
/// arbitrary callback-owned state. Immutable compilation caches may warm on failure.
///
/// These are trusted internal callback obligations. Committed-marker getters deliberately hide
/// provisional samples inside the journals, so they cannot certify callback sampling coverage.
pub(in crate::runtime) trait PendingPairEvaluator<T> {
    fn evaluate(
        &mut self,
        capture: &RetainedInputCapture,
        before: &mut DynamicRuntime,
        before_origins: &mut DynamicSourceOrigins,
        after: &mut DynamicRuntime,
        after_origins: &mut DynamicSourceOrigins,
    ) -> Result<T, String>;
}

#[derive(Debug)]
pub(in crate::runtime) enum PendingPairStop {
    /// No mutation occurred: at least one prepared branch token is stale/wrong, or captures
    /// are not the exact same Arc sequence in both tokens.
    Rejected(PendingHistoryGap),
    /// This attempt was not sampled or consumed. Earlier successful replay in its other
    /// branch remains accepted. Use positions() to prepare a retry from the actual cursors.
    Replay {
        branch: PreloadBranch,
        gap: PendingHistoryGap,
    },
}

#[derive(Default, Debug)]
pub(in crate::runtime) struct PendingPairWindowOutcome {
    pub consumed_attempts: usize,
    pub successful_attempts: usize,
    pub failed_attempts: Vec<PendingAttemptFailure>,
    pub stopped: Option<PendingPairStop>,
}

/// Opaque pair of independently validated, allocation/revision-bound TL581 plans.
pub(in crate::runtime) struct PreparedPendingPairWindow {
    before: PreparedPendingWindow,
    after: PreparedPendingWindow,
}

impl<T> PairedPendingHistory<T> {
    /// Both seeds must be unpinned episode-start preview forks of the same authoritative
    /// capture, following PendingHistorySeed's caller-proven registry/catalogue preconditions.
    pub(in crate::runtime) fn new(
        before: PendingHistorySeed,
        after: PendingHistorySeed,
    ) -> Result<Self, PendingHistoryGap> {
        if before.key.branch != PreloadBranch::BeforeRelease
            || after.key.branch != PreloadBranch::AfterRelease
            || before.key.activation != after.key.activation
            || before.key.programmer != after.key.programmer
            || before.position != after.position
            || !Arc::ptr_eq(&before.snapshot, &after.snapshot)
            || before.live_sample != after.live_sample
        {
            return Err(PendingHistoryGap::InvalidSeed);
        }
        Ok(Self {
            before: DetachedPendingHistory::new(before)?,
            after: DetachedPendingHistory::new(after)?,
            last_success: None,
        })
    }

    /// Actual accepted progress. A replay stop can leave different cold/control cursors while
    /// input cursors remain equal. Retry with each branch's own remaining histories; never
    /// replay the other branch's already accepted prefix or silently reseed either runtime.
    pub(in crate::runtime) fn positions(&self) -> (PendingHistoryPosition, PendingHistoryPosition) {
        (self.before.position, self.after.position)
    }

    pub(in crate::runtime) fn last_success(&self) -> Option<&PendingPairResult<T>> {
        self.last_success.as_ref()
    }

    /// Validate both complete bounded intervals before mutation. Separate replay slices allow
    /// rebuilding after a partial replay stop; inputs must always be the same retained objects.
    pub(in crate::runtime) fn prepare_window(
        &self,
        inputs: &[Arc<RetainedInputCapture>],
        before_cold: &[Arc<ColdGenerationEvent>],
        before_controls: &DynamicControlBatch,
        after_cold: &[Arc<ColdGenerationEvent>],
        after_controls: &DynamicControlBatch,
        limits: PendingHistoryLimits,
    ) -> Result<PreparedPendingPairWindow, PendingHistoryGap> {
        Ok(PreparedPendingPairWindow {
            before: self.before.prepare_window(
                self.before.key,
                inputs,
                before_cold,
                before_controls,
                limits,
            )?,
            after: self.after.prepare_window(
                self.after.key,
                inputs,
                after_cold,
                after_controls,
                limits,
            )?,
        })
    }

    pub(in crate::runtime) fn consume_window(
        &mut self,
        window: PreparedPendingPairWindow,
        evaluator: &mut impl PendingPairEvaluator<T>,
    ) -> PendingPairWindowOutcome {
        let mut outcome = PendingPairWindowOutcome::default();
        if !Arc::ptr_eq(&self.before.identity, &window.before.identity)
            || self.before.revision != window.before.revision
            || !Arc::ptr_eq(&self.after.identity, &window.after.identity)
            || self.after.revision != window.after.revision
        {
            outcome.stopped = Some(PendingPairStop::Rejected(PendingHistoryGap::StaleWindow));
            return outcome;
        }
        if window.before.attempts.len() != window.after.attempts.len()
            || window
                .before
                .attempts
                .iter()
                .zip(&window.after.attempts)
                .any(|(before, after)| !Arc::ptr_eq(&before.capture, &after.capture))
        {
            outcome.stopped = Some(PendingPairStop::Rejected(PendingHistoryGap::InputInterval));
            return outcome;
        }
        // Both checks precede even replay; invalidate all sibling plans before any callback.
        self.before.revision += 1;
        self.after.revision += 1;
        for (before_attempt, after_attempt) in window
            .before
            .attempts
            .into_iter()
            .zip(window.after.attempts)
        {
            if let Err(gap) = self.before.replay_interval(&before_attempt.replay) {
                outcome.stopped = Some(PendingPairStop::Replay {
                    branch: PreloadBranch::BeforeRelease,
                    gap,
                });
                break;
            }
            if let Err(gap) = self.after.replay_interval(&after_attempt.replay) {
                outcome.stopped = Some(PendingPairStop::Replay {
                    branch: PreloadBranch::AfterRelease,
                    gap,
                });
                break;
            }
            let capture = before_attempt.capture;
            let mut before_origins = self.before.origins.clone();
            let mut after_origins = self.after.origins.clone();
            let before = &mut self.before;
            let after = &mut self.after;
            let evaluated = catch_unwind(AssertUnwindSafe(|| {
                before.runtime.with_output_frame_transaction(
                    &mut before.transaction,
                    |before_runtime| {
                        // Distinct runtime journals are nested so Err/unwind restores both. After
                        // the inner callback returns Ok, only infallible journal commit tails run.
                        after.runtime.with_output_frame_transaction(
                            &mut after.transaction,
                            |after_runtime| {
                                evaluator.evaluate(
                                    &capture,
                                    before_runtime,
                                    &mut before_origins,
                                    after_runtime,
                                    &mut after_origins,
                                )
                            },
                        )
                    },
                )
            }));
            // Every evaluated input is consumed, including an error or panic. Its new Live
            // identity maps to each branch's OWN retained marker after transaction rollback.
            for branch in [&mut self.before, &mut self.after] {
                branch.position.inputs = capture.to;
                if let Some(sample) = capture.live_sample {
                    branch.live_sample = Some(sample);
                }
            }
            outcome.consumed_attempts += 1;
            match evaluated {
                Ok(Ok(value)) => {
                    self.before.origins = before_origins;
                    self.after.origins = after_origins;
                    self.last_success = Some(PendingPairResult {
                        before_sample: self.before.runtime.committed_sample_boundary(),
                        after_sample: self.after.runtime.committed_sample_boundary(),
                        capture,
                        value,
                    });
                    outcome.successful_attempts += 1;
                }
                failed => outcome.failed_attempts.push(PendingAttemptFailure {
                    input: capture.to,
                    detail: match failed {
                        Ok(Err(detail)) => detail,
                        Err(_) => {
                            "Pending pair evaluator panicked; both samples were rolled back".into()
                        }
                        Ok(Ok(_)) => unreachable!(),
                    },
                }),
            }
        }
        outcome
    }
}

#[cfg(test)]
#[path = "paired/tests.rs"]
mod tests;
