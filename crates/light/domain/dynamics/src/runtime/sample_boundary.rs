//! Identity of the history that a later transport control can retain. A timestamp is not an
//! identity: two accepted samples at the same clock time can read different Current values.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicSampleScope {
    /// Every instance was considered successfully. Idle/passive lanes may keep older history.
    WholeRuntime,
    /// This does not certify other instances and cannot be promoted to a whole-frame anchor.
    Instance(Uuid),
}

/// An opaque, process-local history boundary. This is neither a persisted checkpoint nor proof
/// that a physical output frame was published. The producer associates an accepted whole-runtime
/// boundary with its immutable input capture before releasing the runtime lock. A replay branch
/// must evaluate those inputs itself; copying Live's held expressions would erase pending intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicSampleBoundary {
    identity: Uuid,
    sampled_at_millis: u64,
    scope: DynamicSampleScope,
}

impl DynamicSampleBoundary {
    pub fn sampled_at_millis(self) -> u64 {
        self.sampled_at_millis
    }

    pub fn scope(self) -> DynamicSampleScope {
        self.scope
    }
}

impl DynamicRuntime {
    /// The most recent successful sample outside a still-provisional output transaction.
    /// `None` means no verifiable boundary is available (new/restored runtime or an uncommitted
    /// failed sample outside a transaction). Consumers must not infer an anchor from wall time.
    pub fn committed_sample_boundary(&self) -> Option<DynamicSampleBoundary> {
        self.output_frame_undo
            .as_ref()
            .and_then(|undo| undo.sample_boundary)
            .unwrap_or(self.sample_boundary)
    }

    /// Invalidate first: a fallible nontransactional sampler may have changed only part of the
    /// history before failing. Transaction rollback restores the previous marker with history.
    pub(super) fn begin_sample_boundary(&mut self) {
        if let Some(undo) = &mut self.output_frame_undo {
            undo.sample_boundary.get_or_insert(self.sample_boundary);
        }
        self.sample_boundary = None;
    }

    pub(super) fn finish_sample_boundary(&mut self, at: u64, scope: DynamicSampleScope) {
        self.sample_boundary = Some(DynamicSampleBoundary {
            identity: Uuid::new_v4(),
            sampled_at_millis: at,
            scope,
        });
    }
}
