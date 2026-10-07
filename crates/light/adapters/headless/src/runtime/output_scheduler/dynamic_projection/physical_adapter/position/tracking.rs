//! Accepted lane-local geometry dependencies. This is a rejection guard for reuse, never proof
//! that semantic/native inputs or a fitted result are reusable. No output is authored here.
use super::{FixtureId, HybridFrameContext, TransitionError, invalid};
use light_engine::{CapturedFrameToken, PreparedFrameGeometry};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct TrackingOwner {
    pub target: FixtureId,
    pub root: FixtureId,
    pub destinations: Vec<FixtureId>,
    /// Original references, including currently missing Points and both retained branches.
    pub points: Vec<FixtureId>,
    /// Explicit descriptor references for every destination, including missing references.
    pub mount_references: Vec<(FixtureId, Option<FixtureId>)>,
    pub incomplete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct TrackingEdge {
    pub root: FixtureId,
    pub destination: FixtureId,
    pub target: FixtureId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct TrackingInstance {
    pub root: FixtureId,
    pub destination: FixtureId,
}

#[derive(Debug)]
pub(in crate::runtime) struct TrackingRegistry {
    owners: Arc<[TrackingOwner]>,
    instances: Arc<[TrackingInstance]>,
    inverse: BTreeMap<Uuid, Arc<[TrackingEdge]>>,
}

impl TrackingRegistry {
    pub fn owners(&self) -> &[TrackingOwner] {
        &self.owners
    }
    fn owner(&self, target: FixtureId) -> Option<&TrackingOwner> {
        self.owners
            .binary_search_by_key(&target.0, |owner| owner.target.0)
            .ok()
            .map(|index| &self.owners[index])
    }
    pub fn instances(&self) -> &[TrackingInstance] {
        &self.instances
    }
    pub fn contains_instance(&self, root: FixtureId, destination: FixtureId) -> bool {
        self.instances
            .binary_search_by_key(&(root.0, destination.0), |row| {
                (row.root.0, row.destination.0)
            })
            .is_ok()
    }
    pub fn dependents(&self, point: FixtureId) -> &[TrackingEdge] {
        self.inverse
            .get(&point.0)
            .map_or(&[], |edges| edges.as_ref())
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct TrackingSnapshot {
    token: CapturedFrameToken,
    geometry: PreparedFrameGeometry,
    registry: Arc<TrackingRegistry>,
    changed_points: Arc<[FixtureId]>,
    dirty: Arc<[TrackingInstance]>,
}

impl TrackingSnapshot {
    pub fn token(&self) -> &CapturedFrameToken {
        &self.token
    }
    pub fn geometry(&self) -> &PreparedFrameGeometry {
        &self.geometry
    }
    pub fn registry(&self) -> &Arc<TrackingRegistry> {
        &self.registry
    }
    pub fn changed_points(&self) -> &[FixtureId] {
        &self.changed_points
    }
    pub fn dirty_instances(&self) -> &[TrackingInstance] {
        &self.dirty
    }
    pub fn dirty_instance(&self, root: FixtureId, destination: FixtureId) -> bool {
        self.dirty
            .binary_search_by_key(&(root.0, destination.0), |row| {
                (row.root.0, row.destination.0)
            })
            .is_ok()
    }
}

struct StagedTracking {
    token: CapturedFrameToken,
    snapshot: Option<Arc<TrackingSnapshot>>,
}

#[derive(Default)]
pub(in crate::runtime) struct TrackingState {
    accepted: Option<Arc<TrackingSnapshot>>,
    staged: Option<StagedTracking>,
    changed_points: Vec<FixtureId>,
}

impl TrackingState {
    pub fn begin(&mut self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.staged = None;
        if self.accepted.as_ref().is_some_and(|accepted| {
            accepted.token == *token || token.sampled_at() < accepted.token.sampled_at()
        }) {
            return Err(invalid(
                "Position tracking frame was accepted or is older than accepted state",
            ));
        }
        self.staged = Some(StagedTracking {
            token: token.clone(),
            snapshot: None,
        });
        Ok(())
    }

    pub fn abandon(&mut self) {
        self.staged = None;
    }

    /// Replace only this attempt's candidate. Acceptance remains the caller's finalizer tail.
    pub fn prepare(
        &mut self,
        frame: HybridFrameContext<'_>,
        owners: &[TrackingOwner],
    ) -> Result<(), TransitionError> {
        if !self
            .staged
            .as_ref()
            .is_some_and(|staged| staged.token == *frame.token)
            || !frame.token.same_capture(&frame.capture.frame_token())
            || frame.token.generation() != frame.capture.generation()
            || !frame.token.matches_geometry(frame.geometry)
            || !frame.token.matches_static_frame(frame.scalar)
        {
            return Err(invalid(
                "Position tracking dependencies use a foreign capture, geometry or staged token",
            ));
        }
        let canonical = canonical_owners(owners)?;
        let registry = self
            .accepted
            .as_ref()
            .filter(|accepted| accepted.registry.owners.as_ref() == canonical.as_slice())
            .map(|accepted| Arc::clone(&accepted.registry))
            .unwrap_or_else(|| Arc::new(build_registry(canonical)));
        let registry_changed = self
            .accepted
            .as_ref()
            .is_none_or(|accepted| !Arc::ptr_eq(&registry, &accepted.registry));
        self.changed_points.clear();
        if let Some(accepted) = &self.accepted {
            frame
                .geometry
                .changed_points_into(&accepted.geometry, &mut self.changed_points);
        } else {
            self.changed_points
                .extend(frame.geometry.points().iter().map(|point| point.fixture_id));
        }
        let mut dirty = Vec::new();
        let cold_change = self
            .accepted
            .as_ref()
            .is_none_or(|accepted| accepted.token.generation() != frame.token.generation());
        for owner in registry.owners() {
            if cold_change
                || (owner.incomplete && !self.changed_points.is_empty())
                || (registry_changed
                    && self
                        .accepted
                        .as_ref()
                        .and_then(|accepted| accepted.registry.owner(owner.target))
                        != Some(owner))
            {
                mark_owner(&mut dirty, owner);
            }
        }
        // Retire stale cohort cache entries for removals/rebindings too. A changed peer makes
        // its complete root/instance dirty, so no other logical peer may reuse an old fit.
        if let Some(accepted) = self.accepted.as_ref().filter(|_| registry_changed) {
            for old in accepted.registry.owners() {
                if registry.owner(old.target) != Some(old) {
                    mark_owner(&mut dirty, old);
                }
            }
        }
        for point in &self.changed_points {
            for edge in registry.dependents(*point) {
                dirty.push(TrackingInstance {
                    root: edge.root,
                    destination: edge.destination,
                });
            }
        }
        if let Some(accepted) = &self.accepted {
            for owner in registry.owners() {
                for &destination in &owner.destinations {
                    if frame.geometry.mounts().mount(destination.0)
                        != accepted.geometry.mounts().mount(destination.0)
                    {
                        dirty.push(TrackingInstance {
                            root: owner.root,
                            destination,
                        });
                    }
                }
            }
        }
        dirty.sort_by_key(|row| (row.root.0, row.destination.0));
        dirty.dedup();
        let snapshot = Arc::new(TrackingSnapshot {
            token: frame.token.clone(),
            geometry: frame.geometry.clone(),
            registry,
            changed_points: self.changed_points.clone().into(),
            dirty: dirty.into(),
        });
        self.staged
            .as_mut()
            .expect("validated staged token")
            .snapshot = Some(snapshot);
        Ok(())
    }

    pub fn verify(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        let Some(staged) = self.staged.as_ref().filter(|staged| staged.token == *token) else {
            return Err(invalid(
                "Position tracking verification uses a foreign staged token",
            ));
        };
        let Some(snapshot) = &staged.snapshot else {
            return Err(invalid("Position tracking census was not prepared"));
        };
        if snapshot.token != *token || !token.matches_geometry(&snapshot.geometry) {
            return Err(invalid(
                "Position tracking snapshot does not match its finalizer",
            ));
        }
        Ok(())
    }

    /// Infallible exact-token move after verification and the engine finalizer. No observer,
    /// source lookup, allocation or external publication belongs in this commit tail.
    pub fn accept(&mut self, token: &CapturedFrameToken) -> bool {
        if !self.staged.as_ref().is_some_and(|staged| {
            staged.token == *token
                && staged.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.token == *token && token.matches_geometry(&snapshot.geometry)
                })
        }) {
            return false;
        }
        self.accepted = self.staged.take().expect("verified staged token").snapshot;
        true
    }

    pub fn snapshot(&self) -> Option<Arc<TrackingSnapshot>> {
        self.accepted.as_ref().map(Arc::clone)
    }

    pub fn staged_snapshot(&self) -> Option<Arc<TrackingSnapshot>> {
        self.staged
            .as_ref()
            .and_then(|staged| staged.snapshot.as_ref())
            .map(Arc::clone)
    }

    /// The active attempt is used while fitting; after acceptance this reads its diagnostics.
    /// Without a prepared attempt it is conservative, including unregistered destinations.
    pub fn dirty_instance(&self, root: FixtureId, destination: FixtureId) -> bool {
        let snapshot = match &self.staged {
            Some(staged) => staged.snapshot.as_ref(),
            None => self.accepted.as_ref(),
        };
        snapshot.is_none_or(|snapshot| {
            !snapshot.registry.contains_instance(root, destination)
                || snapshot.dirty_instance(root, destination)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeKey, AttributeValue, SessionId};
    use light_dynamics::DynamicRuntime;
    use light_engine::{Engine, EngineSnapshot};
    use light_fixture::{ChannelFunction, FixtureProfile, PatchedFixture};
    use light_programmer::ProgrammerRegistry;
    use serde_json::json;

    fn fixture(id: FixtureId, point: bool) -> PatchedFixture {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Tracking dependency test".into();
        if point {
            let mode = &mut profile.modes[0];
            let head = mode.heads[0].id;
            mode.splits[0].footprint = 1;
            mode.channels.push(serde_json::from_value(json!({
                "id":Uuid::new_v4(), "head_id":head, "split":1,
                "fixture_attribute":"point.position.x", "attribute":"point.position.x",
                "resolution":"u8", "secondary_slots":[], "default_raw":128,
                "highlight_raw":255,
                "functions":[ChannelFunction::continuous("X", AttributeKey("point.position.x".into()),255)]
            })).unwrap());
        }
        let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
        serde_json::from_value(json!({"fixture_id":id,"definition":definition})).unwrap()
    }

    fn engine(fixtures: Vec<PatchedFixture>) -> (Engine, ProgrammerRegistry, SessionId) {
        let programmers = ProgrammerRegistry::default();
        let session = SessionId::new();
        programmers.start(session);
        for fixture in &fixtures {
            if fixture
                .definition
                .heads
                .iter()
                .flat_map(|head| &head.parameters)
                .any(|parameter| parameter.attribute.0.as_ref() == "point.position.x")
            {
                programmers.set(
                    session,
                    fixture.fixture_id,
                    AttributeKey("point.position.x".into()),
                    AttributeValue::Normalized(0.5),
                );
            }
        }
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                ..Default::default()
            })
            .unwrap();
        (engine, programmers, session)
    }

    fn owner(target: FixtureId, root: FixtureId, points: Vec<FixtureId>) -> TrackingOwner {
        TrackingOwner {
            target,
            root,
            destinations: vec![root],
            points,
            mount_references: vec![(root, None)],
            incomplete: false,
        }
    }

    fn prepare(
        engine: &Engine,
        state: &mut TrackingState,
        owners: &[TrackingOwner],
    ) -> Arc<TrackingSnapshot> {
        let capture = engine.prepare_output_frame(Default::default());
        let token = capture.frame_token();
        let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
        let geometry = engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        state.begin(&token).unwrap();
        state
            .prepare(
                HybridFrameContext {
                    capture: &capture,
                    geometry: &geometry,
                    native_models: models.as_ref(),
                    token: &token,
                    scalar: &scalar,
                },
                owners,
            )
            .unwrap();
        state.verify(&token).unwrap();
        state.staged_snapshot().unwrap()
    }

    #[test]
    fn staged_tracking_reuses_registry_but_commits_only_matching_success() {
        let root = FixtureId::new();
        let (engine, _, _) = engine(vec![fixture(root, false)]);
        let mut state = TrackingState::default();
        let owners = [owner(root, root, vec![])];
        let first = prepare(&engine, &mut state, &owners);
        assert!(state.snapshot().is_none());
        assert!(first.dirty_instance(root, root));
        assert_eq!(
            first.registry().instances(),
            &[TrackingInstance {
                root,
                destination: root
            }]
        );
        assert!(first.registry().contains_instance(root, root));
        assert!(!first.registry().contains_instance(FixtureId::new(), root));
        assert!(!first.registry().contains_instance(root, FixtureId::new()));
        assert!(state.accept(first.token()));
        let second = prepare(&engine, &mut state, &owners);
        assert!(Arc::ptr_eq(first.registry(), second.registry()));
        assert!(!second.dirty_instance(root, root));
        assert!(!state.dirty_instance(root, root));
        assert!(state.dirty_instance(FixtureId::new(), root));
        assert!(state.dirty_instance(root, FixtureId::new()));
        assert!(!state.accept(first.token()));
        assert!(Arc::ptr_eq(&state.snapshot().unwrap(), &first));
        state.abandon();
        assert!(Arc::ptr_eq(&state.snapshot().unwrap(), &first));
        assert!(state.dirty_instance(FixtureId::new(), FixtureId::new()));
        let retry = prepare(&engine, &mut state, &owners);
        assert!(state.accept(retry.token()));
        assert!(Arc::ptr_eq(&state.snapshot().unwrap(), &retry));
        assert!(state.begin(retry.token()).is_err());
        assert!(state.staged_snapshot().is_none());
        let removed = prepare(&engine, &mut state, &[]);
        assert!(removed.registry().instances().is_empty());
        assert!(
            removed.dirty_instance(root, root),
            "removal retires the previously indexed instance"
        );
        assert!(
            state.dirty_instance(root, root),
            "unknown membership remains conservative"
        );
        assert!(state.accept(removed.token()));
        let restored = prepare(&engine, &mut state, &owners);
        assert!(restored.registry().contains_instance(root, root));
        assert!(
            restored.dirty_instance(root, root),
            "a newly indexed owner requires a fresh solve"
        );
    }

    #[test]
    fn actual_aim_pose_dirties_shared_mechanics_and_incomplete_census_not_unrelated_owners() {
        let aim = FixtureId::new();
        let other_point = FixtureId::new();
        let root = FixtureId::new();
        let unrelated = FixtureId::new();
        let incomplete = FixtureId::new();
        let peer = FixtureId::new();
        let (engine, programmers, session) = engine(vec![
            fixture(aim, true),
            fixture(other_point, true),
            fixture(root, false),
            fixture(unrelated, false),
            fixture(incomplete, false),
        ]);
        let mut partial = owner(incomplete, incomplete, vec![]);
        partial.incomplete = true;
        let owners = [
            owner(root, root, vec![aim]),
            owner(peer, root, vec![]),
            owner(unrelated, unrelated, vec![other_point]),
            partial,
        ];
        let mut state = TrackingState::default();
        let first = prepare(&engine, &mut state, &owners);
        assert_eq!(first.registry().owners().len(), 4);
        let mut instances = [root, unrelated, incomplete].map(|root| TrackingInstance {
            root,
            destination: root,
        });
        instances.sort_by_key(|row| (row.root.0, row.destination.0));
        assert_eq!(
            first.registry().instances(),
            &instances,
            "logical peers share one indexed physical instance"
        );
        state.accept(first.token());
        let unchanged = prepare(&engine, &mut state, &owners);
        assert!(unchanged.dirty_instances().is_empty());
        assert!(!state.dirty_instance(root, root));
        assert!(
            state.dirty_instance(root, peer),
            "a logical peer ID is not a physical destination"
        );
        state.accept(unchanged.token());
        programmers.set(
            session,
            aim,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.55),
        );
        let moved = prepare(&engine, &mut state, &owners);
        assert_eq!(moved.changed_points(), &[aim]);
        assert!(moved.dirty_instance(root, root));
        assert!(
            state.dirty_instance(root, root),
            "all logical peers sharing this physical instance are dirty"
        );
        assert!(moved.dirty_instance(incomplete, incomplete));
        assert!(!moved.dirty_instance(unrelated, unrelated));
        assert_eq!(
            moved.registry().dependents(aim),
            &[TrackingEdge {
                root,
                destination: root,
                target: root
            }]
        );
        assert_ne!(first.geometry().point(aim), moved.geometry().point(aim));
        assert_eq!(
            first.geometry().mounts().mount(root.0),
            moved.geometry().mounts().mount(root.0)
        );
        state.abandon();
        let retry = prepare(&engine, &mut state, &owners);
        assert_eq!(
            retry.changed_points(),
            &[aim],
            "failed candidate must not erase the Point delta"
        );
    }

    #[test]
    fn missing_refs_survive_and_cold_generation_is_conservative() {
        let root = FixtureId::new();
        let missing = FixtureId::new();
        let (engine, _, _) = engine(vec![fixture(root, false)]);
        let mut tracked = owner(root, root, vec![missing]);
        tracked.mount_references = vec![(root, Some(missing))];
        let mut state = TrackingState::default();
        let first = prepare(&engine, &mut state, &[tracked.clone()]);
        state.accept(first.token());
        assert_eq!(
            first.registry().dependents(missing).len(),
            1,
            "aim/mount duplicates are one edge"
        );
        let mut replacement = engine.snapshot().as_ref().clone();
        let mut fixtures = replacement.fixtures.as_ref().clone();
        fixtures.push(fixture(missing, true));
        replacement.fixtures = fixtures.into();
        engine.replace_snapshot(replacement).unwrap();
        let restored = prepare(&engine, &mut state, &[tracked]);
        assert!(restored.changed_points().contains(&missing));
        assert!(restored.dirty_instance(root, root));
        assert!(Arc::ptr_eq(first.registry(), restored.registry()));
        assert!(first.geometry().point(missing).is_none());
        assert!(restored.geometry().point(missing).is_some());
    }
}

fn mark_owner(dirty: &mut Vec<TrackingInstance>, owner: &TrackingOwner) {
    dirty.extend(
        owner
            .destinations
            .iter()
            .map(|&destination| TrackingInstance {
                root: owner.root,
                destination,
            }),
    );
}

fn canonical_owners(owners: &[TrackingOwner]) -> Result<Vec<TrackingOwner>, TransitionError> {
    let mut canonical = owners.to_vec();
    for owner in &mut canonical {
        if owner.target.0.is_nil()
            || owner.root.0.is_nil()
            || owner.destinations.is_empty()
            || owner.destinations.iter().any(|id| id.0.is_nil())
            || owner.points.iter().any(|id| id.0.is_nil())
            || owner.mount_references.iter().any(|(id, reference)| {
                id.0.is_nil() || reference.is_some_and(|point| point.0.is_nil())
            })
        {
            return Err(invalid(
                "Position tracking owners require non-nil owners, destinations and references",
            ));
        }
        owner.destinations.sort_by_key(|id| id.0);
        owner.destinations.dedup();
        owner.points.sort_by_key(|id| id.0);
        owner.points.dedup();
        owner.mount_references.sort_by_key(|(id, _)| id.0);
        if owner.mount_references.len() != owner.destinations.len()
            || owner
                .mount_references
                .iter()
                .zip(&owner.destinations)
                .any(|((mount, _), destination)| mount != destination)
        {
            return Err(invalid(
                "Position tracking requires one mount reference per destination",
            ));
        }
    }
    canonical.sort_by_key(|owner| owner.target.0);
    if canonical
        .windows(2)
        .any(|pair| pair[0].target == pair[1].target)
    {
        return Err(invalid(
            "Position tracking census contains a duplicate logical owner",
        ));
    }
    Ok(canonical)
}

fn build_registry(owners: Vec<TrackingOwner>) -> TrackingRegistry {
    let mut instances = Vec::new();
    let mut inverse = BTreeMap::<Uuid, Vec<TrackingEdge>>::new();
    for owner in &owners {
        for &destination in &owner.destinations {
            instances.push(TrackingInstance {
                root: owner.root,
                destination,
            });
            let edge = TrackingEdge {
                root: owner.root,
                destination,
                target: owner.target,
            };
            for point in owner.points.iter().copied().chain(
                owner
                    .mount_references
                    .iter()
                    .filter(|(mount, _)| *mount == destination)
                    .filter_map(|(_, reference)| *reference),
            ) {
                inverse.entry(point.0).or_default().push(edge);
            }
        }
    }
    instances.sort_by_key(|row| (row.root.0, row.destination.0));
    instances.dedup();
    TrackingRegistry {
        owners: owners.into(),
        instances: instances.into(),
        inverse: inverse
            .into_iter()
            .map(|(point, mut edges)| {
                edges.sort_by_key(|edge| (edge.root.0, edge.destination.0, edge.target.0));
                edges.dedup();
                (point, edges.into())
            })
            .collect(),
    }
}
