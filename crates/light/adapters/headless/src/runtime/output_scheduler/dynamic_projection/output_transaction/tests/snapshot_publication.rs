use super::*;
use crate::runtime::dynamic_snapshot_publication::DynamicSnapshotPublication;
use std::cell::Cell;

struct Harness {
    engine: Engine,
    clock: Arc<light_core::ManualClock>,
    dynamics: Mutex<DynamicRuntime>,
    publication: DynamicSnapshotPublication,
    origins: SharedDynamicSourceOrigins,
    groups: Mutex<[light_control::speed::SpeedGroupController; 5]>,
    cache: ProgrammerReconciliationCache,
    rate: AtomicU16,
    finishes: Cell<usize>,
}

#[test]
fn mismatched_retention_token_does_not_interrupt_live_output_or_record_wrong_inputs() {
    use crate::runtime::dynamic_snapshot_publication::RetainedFrameCapture;
    let harness = Harness::new();
    harness
        .publication
        .begin_retained_history(
            &mut harness.dynamics.lock(),
            &harness.engine.snapshot(),
            std::num::NonZeroUsize::new(4).unwrap(),
        )
        .unwrap();
    let cursor = harness.publication.input_capture_cursor().unwrap();
    let other = RetainedFrameCapture::select(
        harness.engine.prepare_output_frame(Default::default()),
        &harness.publication,
        std::time::Instant::now(),
    );
    let frame = harness.engine.prepare_output_frame(Default::default());
    let output = dynamic_output_frame(
        &harness.engine,
        &frame,
        other.retained(),
        &[],
        &harness.dynamics,
        &harness.publication,
        &harness.origins,
        &harness.groups,
        &harness.rate,
        &harness.cache,
        &LiveFamilyAdapters::default(),
        legacy(|batches| harness.engine.render_prepared(&frame, batches)),
    )
    .unwrap();
    assert!(output.sample_boundary.is_some());
    assert!(
        harness
            .publication
            .input_captures_since(cursor)
            .unwrap()
            .is_empty()
    );
}

impl Harness {
    fn new() -> Self {
        let clock = Arc::new(light_core::ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = light_core::SessionId::new();
        programmers.start(session);
        let definition = definition();
        let engine = Engine::new(programmers.clone());
        let group_target = light_playback::PlaybackTarget::Group {
            group_id: "front".into(),
            initial_master: Some(1.0),
        };
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                playbacks: vec![light_playback::PlaybackDefinition {
                    number: 1,
                    name: "Front master".into(),
                    buttons: light_playback::PlaybackDefinition::default_buttons(&group_target),
                    button_count: 3,
                    fader: light_playback::PlaybackDefinition::default_fader(&group_target),
                    has_fader: true,
                    footprint: light_playback::PlaybackFootprint::Normal,
                    go_activates: true,
                    auto_off: false,
                    xfade_millis: 0,
                    color: "#20c997".into(),
                    flash_release: light_playback::FlashReleaseMode::ReleaseAll,
                    protect_from_swap: false,
                    presentation_icon: None,
                    presentation_image: None,
                    target: group_target,
                }]
                .into(),
                groups: vec![light_programmer::GroupDefinition {
                    id: "front".into(),
                    ..Default::default()
                }]
                .into(),
                ..Default::default()
            })
            .unwrap();
        assert!(programmers.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: FixtureId::new(),
                attribute: AttributeKey::intensity(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: Uuid::new_v4(),
                    lane_id: definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: 1,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition.clone()),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.0,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.0,
                    },
                    timing: Default::default(),
                },
            }],
            None,
        ));
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition]).unwrap();
        let publication = DynamicSnapshotPublication::new(engine.snapshot());
        Self {
            engine,
            clock,
            dynamics: Mutex::new(runtime),
            publication,
            origins: SharedDynamicSourceOrigins::default(),
            groups: Mutex::new(std::array::from_fn(|_| {
                light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
            })),
            cache: ProgrammerReconciliationCache::default(),
            rate: AtomicU16::new(40),
            finishes: Cell::new(0),
        }
    }

    fn render(
        &self,
        frame: &light_engine::PreparedOutputFrame,
    ) -> Result<CommittedDynamicOutput<()>, EngineError> {
        dynamic_output_frame(
            &self.engine,
            frame,
            None,
            &[],
            &self.dynamics,
            &self.publication,
            &self.origins,
            &self.groups,
            &self.rate,
            &self.cache,
            &LiveFamilyAdapters::default(),
            legacy(|batches| {
                self.finishes.set(self.finishes.get() + 1);
                self.engine.render_prepared(frame, batches).map(|_| ())
            }),
        )
    }

    fn assert_rejected_without_dynamic_effects(&self, frame: &light_engine::PreparedOutputFrame) {
        let runtime = self.dynamics.lock().snapshot();
        let origins = self.origins.load_full();
        let reconciliation = self
            .cache
            .changed(frame.dynamic_programmer_values(), &frame.snapshot());
        let sources_changed = self.cache.sources_changed(&origins);
        let finishes = self.finishes.get();
        assert!(matches!(
            self.render(frame),
            Err(EngineError::StalePreparedFrame)
        ));
        assert_eq!(self.dynamics.lock().snapshot(), runtime);
        assert!(Arc::ptr_eq(&origins, &self.origins.load_full()));
        assert_eq!(
            self.cache
                .changed(frame.dynamic_programmer_values(), &frame.snapshot()),
            reconciliation
        );
        assert_eq!(self.cache.sources_changed(&origins), sources_changed);
        assert_eq!(self.finishes.get(), finishes);
    }

    fn publish_current_registry(&self) {
        let snapshot = self.engine.snapshot();
        let mut runtime = self.dynamics.lock();
        runtime
            .install_definitions(snapshot.dynamics.iter().cloned())
            .unwrap();
        self.publication.installed(snapshot);
    }
}

#[test]
fn newer_capture_waits_for_its_registry_even_when_revision_and_definitions_are_unchanged() {
    for change_group_membership in [false, true] {
        let harness = Harness::new();
        let before = harness.engine.snapshot();
        let mut replacement = (*before).clone();
        if change_group_membership {
            Arc::make_mut(&mut replacement.groups)[0].fixtures = vec![FixtureId::new()];
        }
        harness.engine.replace_snapshot(replacement).unwrap();
        harness.clock.advance_millis(125);
        let frame = harness.engine.prepare_output_frame(Default::default());
        assert_eq!(before.revision, frame.snapshot().revision);
        assert!(Arc::ptr_eq(&before.dynamics, &frame.snapshot().dynamics));
        assert!(!Arc::ptr_eq(&before, &frame.snapshot()));
        assert!(harness.dynamics.lock().snapshot().instances.is_empty());

        harness.assert_rejected_without_dynamic_effects(&frame);
        harness.publish_current_registry();
        let completed = harness.render(&frame).unwrap();
        assert_eq!(completed.runtime.instances.len(), 1);
        assert_eq!(completed.samples.len(), 1);
        assert!(!completed.events.is_empty());
        assert!(
            !harness
                .cache
                .changed(frame.dynamic_programmer_values(), &frame.snapshot())
        );
        let mut occurrences = HashSet::new();
        completed.samples[0]
            .expression
            .visit_source_occurrences(&mut |id| {
                occurrences.insert(id);
            })
            .unwrap();
        assert_eq!(occurrences.len(), 1);
        assert!(
            completed
                .origins
                .get(*occurrences.iter().next().unwrap())
                .is_some()
        );
    }
}

#[test]
fn old_capture_cannot_advance_new_registry_although_engine_can_render_its_old_generation() {
    let harness = Harness::new();
    let initial = harness.engine.prepare_output_frame(Default::default());
    let started = harness.render(&initial).unwrap();
    let instance = started.runtime.instances[0].id;
    harness.clock.advance_millis(125);
    let old = harness.engine.prepare_output_frame(Default::default());
    harness
        .engine
        .replace_snapshot((*harness.engine.snapshot()).clone())
        .unwrap();
    harness.publish_current_registry();

    harness.assert_rejected_without_dynamic_effects(&old);
    // The registry guard adds a stronger joint-source boundary than Engine's intentional
    // support for retained captures. Continuity alone does not reject this old generation.
    harness.engine.render_prepared(&old, &[]).unwrap();
    let current = harness.engine.prepare_output_frame(Default::default());
    let completed = harness.render(&current).unwrap();
    assert_eq!(completed.runtime.instances[0].id, instance);
    assert_eq!(completed.samples.len(), 1);
    assert_eq!(started.samples[0].legacy().unwrap().value, 0.0);
    assert_eq!(completed.samples[0].legacy().unwrap().value, 0.25);
}

#[test]
fn group_master_generation_change_keeps_the_matching_registry_eligible() {
    let harness = Harness::new();
    let before = harness.engine.prepare_output_frame(Default::default());
    let started = harness.render(&before).unwrap();
    assert!(harness.engine.set_group_master("front", 0.5).unwrap());
    harness.clock.advance_millis(125);
    let after = harness.engine.prepare_output_frame(Default::default());
    assert_ne!(before.generation(), after.generation());
    assert!(Arc::ptr_eq(&before.snapshot(), &after.snapshot()));
    assert!(harness.publication.matches(&after.snapshot()));
    let completed = harness.render(&after).unwrap();
    assert_eq!(
        completed.runtime.instances[0].id,
        started.runtime.instances[0].id
    );
    assert_eq!(completed.samples.len(), 1);
    assert_eq!(harness.finishes.get(), 2);
}

#[test]
fn cold_controller_reconciliation_defers_an_unpublished_registry_without_losing_authored_rows() {
    let harness = Harness::new();
    let before = harness.dynamics.lock().snapshot();
    let origins = harness.origins.load_full();
    harness
        .engine
        .replace_snapshot((*harness.engine.snapshot()).clone())
        .unwrap();
    crate::runtime::output_scheduler::reconcile_dynamic_controllers(
        &harness.engine,
        &harness.dynamics,
        &harness.publication,
    );
    assert_eq!(harness.dynamics.lock().snapshot(), before);
    assert!(Arc::ptr_eq(&origins, &harness.origins.load_full()));
    harness.publish_current_registry();
    crate::runtime::output_scheduler::reconcile_dynamic_controllers(
        &harness.engine,
        &harness.dynamics,
        &harness.publication,
    );
    let installed = harness.dynamics.lock().snapshot();
    assert_eq!(installed.instances.len(), 1);
    assert!(installed.instances[0].last_sample_values.is_empty());
}
