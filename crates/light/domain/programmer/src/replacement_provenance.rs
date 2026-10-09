use crate::ProgrammerRegistry;
use light_core::{ReplacementProjectionMap, SessionId};
use std::{collections::HashSet, sync::Arc};

impl ProgrammerRegistry {
    /// Preflight physical routing rebasing while the caller owns the global mutation boundary.
    pub fn validate_replacement_migration_with_history(
        &self,
        programmer_id: light_core::ProgrammerId,
        live: &[(u64, ReplacementProjectionMap)],
        undo: &[Vec<(u64, ReplacementProjectionMap)>],
        redo: &[Vec<(u64, ReplacementProjectionMap)>],
    ) -> Result<(), String> {
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let states = self.state.read();
        let state = states
            .as_ref()
            .filter(|state| state.id == programmer_id)
            .ok_or("replacement Programmer identity changed")?;
        validate_history(state, live, undo, redo)
    }

    /// Rebase retained physical routing without adding, removing or reordering operator history.
    /// Accepted preflight remains authoritative only under the same caller-held mutation boundary.
    pub fn apply_replacement_migration_with_history(
        &self,
        programmer_id: light_core::ProgrammerId,
        live: &[(u64, ReplacementProjectionMap)],
        undo: &[Vec<(u64, ReplacementProjectionMap)>],
        redo: &[Vec<(u64, ReplacementProjectionMap)>],
    ) -> Result<bool, String> {
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let state = states
            .as_mut()
            .filter(|state| state.id == programmer_id)
            .ok_or("replacement Programmer identity changed")?;
        validate_history(state, live, undo, redo)?;
        let mut candidate = state.clone();
        let old_metadata = Arc::clone(&state.replacement_provenance);
        let mut changed = merge_origins(&mut candidate.replacement_provenance, live);
        for (history, deltas) in [(&mut candidate.undo, undo), (&mut candidate.redo, redo)] {
            for (snapshot, origins) in history.iter_mut().zip(deltas) {
                let mut metadata = Arc::clone(&snapshot.replacement_provenance);
                if merge_origins(&mut metadata, origins) {
                    Arc::make_mut(snapshot).replacement_provenance = metadata;
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(false);
        }
        let order_changed =
            |order: u64| old_metadata.get(&order) != candidate.replacement_provenance.get(&order);
        let normal_changed = candidate
            .values
            .iter()
            .chain(candidate.preload_active.iter())
            .any(|value| order_changed(value.programmer_order))
            || candidate
                .group_values
                .values()
                .chain(candidate.preload_group_active.values())
                .flat_map(|values| values.values())
                .any(|value| order_changed(value.programmer_order));
        let pending_changed = candidate
            .preload_pending
            .iter()
            .any(|value| order_changed(value.programmer_order))
            || candidate
                .preload_group_pending
                .values()
                .flat_map(|values| values.values())
                .any(|value| order_changed(value.programmer_order));
        *state = candidate;
        drop(states);
        if normal_changed {
            self.mark_normal_values_changed();
        }
        if pending_changed {
            self.mark_preload_values_changed();
        }
        Ok(true)
    }

    /// Attach a captured recall envelope only to still-live exact edit identities.
    pub fn attach_replacement_provenance(
        &self,
        session: SessionId,
        origins: &[(u64, ReplacementProjectionMap)],
        preload: bool,
        checkpoint: bool,
    ) -> bool {
        if !self.knows_session(session) {
            return false;
        }
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        let (values, groups) = if preload {
            (
                state.preload_pending.as_slice(),
                &state.preload_group_pending,
            )
        } else {
            (state.values.as_slice(), state.group_values.as_ref())
        };
        let live = values
            .iter()
            .map(|value| value.programmer_order)
            .chain(
                groups
                    .values()
                    .flat_map(|values| values.values())
                    .map(|value| value.programmer_order),
            )
            .collect::<HashSet<_>>();
        let updates = origins
            .iter()
            .filter(|(order, map)| {
                *order != 0
                    && live.contains(order)
                    && if map.is_empty() {
                        state.replacement_provenance.contains_key(order)
                    } else {
                        state.replacement_provenance.get(order) != Some(map)
                    }
            })
            .cloned()
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return false;
        }
        if checkpoint {
            state.checkpoint();
        }
        let metadata = Arc::make_mut(&mut state.replacement_provenance);
        for (order, map) in updates {
            if map.is_empty() {
                metadata.remove(&order);
            } else {
                metadata.insert(order, map);
            }
        }
        drop(states);
        if preload {
            self.mark_preload_values_changed();
        } else {
            self.mark_normal_values_changed();
        }
        true
    }

    /// Install post-commit replacement envelopes onto the exact still-current desk Programmer.
    /// This does not create a session or rewrite values and includes already-active Preload.
    pub fn apply_replacement_migration(
        &self,
        programmer_id: light_core::ProgrammerId,
        origins: &[(u64, ReplacementProjectionMap)],
    ) -> bool {
        if origins.iter().any(|(_, map)| {
            map.iter().any(|(owner, projection)| {
                *owner != projection.source_owner || projection.validate().is_err()
            })
        }) {
            return false;
        }
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut().filter(|state| state.id == programmer_id) else {
            return false;
        };
        let direct = state
            .values
            .iter()
            .chain(state.preload_pending.iter())
            .chain(state.preload_active.iter())
            .map(|value| (value.programmer_order, value.fixture_id))
            .collect::<std::collections::HashMap<_, _>>();
        if origins.iter().any(|(order, map)| {
            direct
                .get(order)
                .is_some_and(|owner| map.keys().any(|source| source != owner))
        }) {
            return false;
        }
        let normal = state
            .values
            .iter()
            .chain(state.preload_active.iter())
            .map(|value| value.programmer_order)
            .chain(
                state
                    .group_values
                    .values()
                    .chain(state.preload_group_active.values())
                    .flat_map(|values| values.values())
                    .map(|value| value.programmer_order),
            )
            .collect::<HashSet<_>>();
        let pending = state
            .preload_pending
            .iter()
            .map(|value| value.programmer_order)
            .chain(
                state
                    .preload_group_pending
                    .values()
                    .flat_map(|values| values.values())
                    .map(|value| value.programmer_order),
            )
            .collect::<HashSet<_>>();
        let updates = origins
            .iter()
            .filter(|(order, map)| {
                *order != 0
                    && (normal.contains(order) || pending.contains(order))
                    && map.iter().any(|(owner, projection)| {
                        state
                            .replacement_provenance
                            .get(order)
                            .and_then(|existing| existing.get(owner))
                            != Some(projection)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return false;
        }
        let changed_normal = updates.iter().any(|(order, _)| normal.contains(order));
        let changed_pending = updates.iter().any(|(order, _)| pending.contains(order));
        state.checkpoint();
        let metadata = Arc::make_mut(&mut state.replacement_provenance);
        for (order, map) in updates {
            metadata.entry(order).or_default().extend(map);
        }
        drop(states);
        if changed_normal {
            self.mark_normal_values_changed();
        }
        if changed_pending {
            self.mark_preload_values_changed();
        }
        true
    }
}

fn validate_history(
    state: &crate::ProgrammerState,
    live: &[(u64, ReplacementProjectionMap)],
    undo: &[Vec<(u64, ReplacementProjectionMap)>],
    redo: &[Vec<(u64, ReplacementProjectionMap)>],
) -> Result<(), String> {
    if state.undo.len() != undo.len() || state.redo.len() != redo.len() {
        return Err("replacement Programmer history changed; capture it again".into());
    }
    validate_snapshot(&state.snapshot(), live)?;
    for (snapshot, origins) in state
        .undo
        .iter()
        .zip(undo)
        .chain(state.redo.iter().zip(redo))
    {
        validate_snapshot(snapshot, origins)?;
    }
    Ok(())
}

fn validate_snapshot(
    snapshot: &crate::ProgrammerSnapshot,
    origins: &[(u64, ReplacementProjectionMap)],
) -> Result<(), String> {
    let direct = snapshot
        .values
        .iter()
        .chain(snapshot.preload_pending.iter())
        .chain(snapshot.preload_active.iter())
        .map(|value| (value.programmer_order, value.fixture_id))
        .collect::<std::collections::HashMap<_, _>>();
    let group_orders = snapshot
        .group_values
        .values()
        .chain(snapshot.preload_group_pending.values())
        .chain(snapshot.preload_group_active.values())
        .flat_map(|values| values.values())
        .map(|value| value.programmer_order)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut counts = std::collections::HashMap::<u64, usize>::new();
    for order in snapshot
        .values
        .iter()
        .chain(snapshot.preload_pending.iter())
        .chain(snapshot.preload_active.iter())
        .map(|value| value.programmer_order)
        .chain(
            snapshot
                .group_values
                .values()
                .chain(snapshot.preload_group_pending.values())
                .chain(snapshot.preload_group_active.values())
                .flat_map(|values| values.values())
                .map(|value| value.programmer_order),
        )
    {
        *counts.entry(order).or_default() += 1;
    }
    for (order, map) in origins {
        if *order == 0
            || !seen.insert(*order)
            || counts.get(order) != Some(&1)
            || (!direct.contains_key(order) && !group_orders.contains(order))
        {
            return Err("replacement source edit order is stale or ambiguous".into());
        }
        for (owner, projection) in map {
            projection.validate().map_err(|error| error.to_string())?;
            if *owner != projection.source_owner
                || direct.get(order).is_some_and(|fixture| fixture != owner)
            {
                return Err(
                    "replacement envelope does not belong to its captured source address".into(),
                );
            }
        }
    }
    Ok(())
}

fn merge_origins(
    metadata: &mut Arc<std::collections::HashMap<u64, ReplacementProjectionMap>>,
    origins: &[(u64, ReplacementProjectionMap)],
) -> bool {
    let mut changed = false;
    for (order, map) in origins {
        if map.iter().any(|(owner, projection)| {
            metadata.get(order).and_then(|existing| existing.get(owner)) != Some(projection)
        }) {
            Arc::make_mut(metadata)
                .entry(*order)
                .or_default()
                .extend(map.clone());
            changed = true;
        }
    }
    changed
}
