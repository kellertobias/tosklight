//! Three-way merge of whole shows (TL-858).

use super::json::BodyMerger;
use super::model::{MergeInputs, MergeKindSummary, MergeOutcome, MergeResolutions};
use super::renumber;
use std::collections::{BTreeMap, BTreeSet};

/// Merges the offline copy (`theirs`) into the show (`mine`) against their common `base`.
///
/// An object only one side changed takes that side; an object both sides changed merges
/// structurally; what still differs is a conflict, decided by `resolutions` or reported. The
/// result is deterministic, so a preview and the apply that follows it agree.
pub fn merge_shows(inputs: &MergeInputs, resolutions: &MergeResolutions) -> MergeOutcome {
    let mut theirs = inputs.theirs.clone();
    let renumbers = renumber::separate_collisions(&inputs.base, &inputs.mine, &mut theirs);
    let keys = inputs
        .base
        .keys()
        .chain(inputs.mine.keys())
        .chain(theirs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut outcome = MergeOutcome::default();
    let mut summary = BTreeMap::<String, MergeKindSummary>::new();
    for key in keys {
        let (base, mine, theirs) = (
            inputs.base.get(&key),
            inputs.mine.get(&key),
            theirs.get(&key),
        );
        let (merged, mut conflicts) = {
            let mut merger = BodyMerger::new(&key, resolutions);
            let merged = merger.merge(base, mine, theirs);
            (merged, std::mem::take(&mut merger.conflicts))
        };
        let entry = summary
            .entry(key.kind().to_owned())
            .or_insert_with(|| MergeKindSummary {
                kind: key.kind().to_owned(),
                ..MergeKindSummary::default()
            });
        entry.conflicts += conflicts.len();
        let both_changed = mine.is_some() && theirs.is_some() && base != mine && base != theirs;
        if both_changed && mine != theirs && conflicts.is_empty() {
            outcome.auto_merged += 1;
        }
        if merged.as_ref() != mine {
            match (mine, &merged) {
                (None, _) => entry.added += 1,
                (_, None) => entry.deleted += 1,
                _ => entry.changed += 1,
            }
            outcome.writes.insert(key, merged);
        }
        outcome.conflicts.append(&mut conflicts);
    }
    outcome.summary = summary
        .into_values()
        .filter(|kind| kind.added + kind.changed + kind.deleted + kind.conflicts > 0)
        .collect();
    let mut merged_macros = inputs.mine.clone();
    for (key, body) in &outcome.writes {
        match body {
            Some(body) => merged_macros.insert(key.clone(), body.clone()),
            None => merged_macros.remove(key),
        };
    }
    outcome.macro_warnings = renumber::macro_warnings(&merged_macros, &renumbers);
    outcome.renumbers = renumbers;
    outcome
}
