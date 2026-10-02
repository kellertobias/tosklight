//! Stage one keeps legacy scalar output while typed Fixed rows enter family composition once.
//! The compiler's captured row identities select exclusions; values, ranks and source evidence
//! are never reconstructed or removed from the typed masks.
#![allow(dead_code)]

use super::*;
use fixed_masks::PreparedFixedMask;
use light_dynamics::{FamilyFixedSampleSource, FamilySampleIdentity};
use rustc_hash::FxHashSet;

/// Reuse one bounded identity set per independently evaluated branch. No captured input array
/// is cloned, and membership checks never scan the masks again for each fixture or source row.
#[derive(Default)]
pub(super) struct HybridScalarProjectionScratch {
    excluded: FxHashSet<(FamilyFixedSampleSource, usize)>,
}

/// `fixed` must be the successfully validated compilation of these same captured input arrays.
/// Exclude every typed row, including delayed, disabled and unavailable masks: participation
/// belongs to family assembly. Legacy runtime fragments (including Focus) still pass through;
/// same-owner legacy/typed composition remains a separate production integration gate.
pub(super) fn project_hybrid_scalar_samples(
    inputs: &CapturedDynamicInputs<'_>,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: CapturedDynamicOutputControls<'_>,
    sources: &impl DynamicTickSource,
    fixed: &[PreparedFixedMask],
    scratch: &mut HybridScalarProjectionScratch,
) -> Vec<ContributionBatch> {
    scratch.excluded.clear();
    scratch.excluded.extend(fixed.iter().map(|row| {
        let FamilySampleIdentity::Fixed { source, row_index } = row.rank.identity else {
            unreachable!("validated Fixed compilation has captured row identities")
        };
        (source, row_index)
    }));
    let candidates = collect_dynamic_candidates_with_fixed_rows(
        inputs.addresser,
        inputs.programmer_values,
        inputs.cue_values,
        inputs.extra_programmer_values,
        samples,
        controls.playbacks,
        controls.cues,
        sources,
        u64::try_from(inputs.now.timestamp_millis()).unwrap_or_default(),
        |source, index| !scratch.excluded.contains(&(source, index)),
    );
    if candidates.is_empty() {
        Vec::new()
    } else {
        vec![dynamic_contribution_batch(candidates, sources, inputs.now)]
    }
}

#[cfg(test)]
mod tests;
