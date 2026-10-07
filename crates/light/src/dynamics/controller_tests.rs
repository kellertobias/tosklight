use super::*;
use crate::ActionSource;
use light_dynamics::DynamicAddressValue;

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::from_u128(101), "pool_number": 1, "revision": 1, "name": "Scope test",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::from_u128(102), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type":"value", "value":0.25}, "interpolation":"linear"},
                {"position": 0.5, "source": {"type":"value", "value":0.75}, "interpolation":"linear"}
            ], "size":1.0},
            "max_min": {"minimum":{"type":"value","value":0.25},
                "maximum":{"type":"value","value":0.75}, "function":"sinus", "size":1.0},
            "middle_amplitude": {"middle":{"type":"current"}, "amplitude":0.25,
                "function":"sinus", "size":1.0},
            "speed_multiplier":{"numerator":1,"denominator":1}, "width":1.0,
            "random_group_id":null
        }],
        "phase":{"ordering":{"type":"selection"}, "offset_degrees":0.0,
            "span_degrees":360.0, "block_size":1, "repeats":1, "wings":false,
            "anchors_degrees":[]},
        "speed":{"type":"fixed", "duration_millis":1000}, "default_activation":"start_now"
    })).unwrap()
}

fn on(link: Uuid, fixture: FixtureId, order: u64, size: f32) -> DynamicAddressValue {
    let definition = definition();
    DynamicAddressValue {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::DynamicOn {
            instance_link: link,
            lane_id: definition.lanes[0].id,
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition),
                },
            },
            overrides: DynamicInstanceOverrides {
                size,
                speed_multiplier: light_dynamics::Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: DynamicValueTiming::default(),
        },
        programmer_order: order,
        changed_at_millis: order * 10,
    }
}

fn off(link: Uuid, fixture: FixtureId, order: u64) -> DynamicAddressValue {
    DynamicAddressValue {
        value: DynamicSemanticValue::DynamicOff {
            instance_link: link,
            timing: Default::default(),
        },
        ..on(link, fixture, order, 1.0)
    }
}

#[derive(Default)]
struct Ports {
    starts: Mutex<Vec<DynamicStartRequest>>,
    updates: Mutex<Vec<Uuid>>,
    offs: Mutex<Vec<Uuid>>,
    events: Mutex<Vec<crate::DynamicRuntimeChange>>,
}

impl DynamicsPorts for Ports {
    fn authorize(&self, _: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }
    fn snapshot(&self) -> Arc<EngineSnapshot> {
        Arc::new(EngineSnapshot {
            dynamics: Arc::new(vec![definition()]),
            ..Default::default()
        })
    }
    fn now_millis(&self) -> u64 {
        100
    }
    fn runtime_controller_is_completed(&self, _: Uuid) -> bool {
        false
    }
    fn runtime_controller_instance(&self, _: Uuid) -> Option<Uuid> {
        Some(Uuid::from_u128(999))
    }
    fn reconcile_programmer_runtime(&self) {}
    fn start_runtime(&self, request: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError> {
        self.starts.lock().push(request);
        Ok(Uuid::from_u128(999))
    }
    fn off_runtime_controller(
        &self,
        id: Uuid,
        _: u64,
        _: u64,
        _: u64,
    ) -> Result<(Uuid, bool), DynamicRuntimeError> {
        self.offs.lock().push(id);
        Ok((Uuid::from_u128(999), true))
    }
    fn update_runtime_controller(
        &self,
        id: Uuid,
        _: Option<f32>,
        _: Option<f32>,
        _: Option<f32>,
    ) -> Result<(), DynamicRuntimeError> {
        self.updates.lock().push(id);
        Ok(())
    }
    fn publish_runtime_change(&self, _: &ActionContext, change: crate::DynamicRuntimeChange) {
        self.events.lock().push(change);
    }
}

fn desk() -> (ProgrammerRegistry, SessionId, ActionContext) {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    let context = ActionContext::operator(Uuid::new_v4(), session.0, ActionSource::Keyboard);
    (registry, session, context)
}

#[test]
fn effective_controller_scope_includes_committed_preload_and_only_armed_pending() {
    let (registry, session, _) = desk();
    let mut state = registry.get(session).unwrap();
    let link = Uuid::from_u128(200);
    let fixture = FixtureId::new();
    state.preload_dynamic_active = Arc::new(vec![on(link, fixture, 1, 0.5)]);
    state.preload_dynamic_pending = Arc::new(vec![off(link, fixture, 2)]);
    let first = resolve_programmer_dynamic_controller(&state, link).unwrap();
    assert_eq!(first.targets, vec![fixture]);
    assert_eq!(
        first.controller_id,
        light_dynamics::programmer_dynamic_controller_id(state.id, link)
    );
    assert_eq!(
        resolve_programmer_dynamic_controller(&state, first.controller_id),
        Some(first.clone())
    );
    state.blind = true;
    state.preload_capture_programmer = false;
    assert_eq!(
        effective_programmer_dynamic_controllers(&state),
        vec![first.clone()]
    );
    state.preload_capture_programmer = true;
    assert!(effective_programmer_dynamic_controllers(&state).is_empty());
    state.blind = false;
    state.id = light_core::ProgrammerId::new();
    assert!(resolve_programmer_dynamic_controller(&state, first.controller_id).is_none());
    assert_ne!(
        resolve_programmer_dynamic_controller(&state, link)
            .unwrap()
            .controller_id,
        first.controller_id
    );
}

#[test]
fn newer_normal_on_supersedes_committed_off_without_renaming_the_link() {
    let (registry, session, _) = desk();
    let mut state = registry.get(session).unwrap();
    let link = Uuid::from_u128(200);
    let fixture = FixtureId::new();
    state.dynamic_values = Arc::new(vec![on(link, fixture, 3, 0.7)]);
    state.preload_dynamic_active = Arc::new(vec![off(link, fixture, 2)]);
    let values = effective_programmer_dynamic_values(&state);
    assert_eq!(values, vec![&state.dynamic_values[0]]);
    assert_eq!(
        effective_programmer_dynamic_controllers(&state)[0].authored_link,
        link
    );
}

#[test]
fn controller_reference_comes_from_the_newest_surviving_on_row() {
    let (registry, session, _) = desk();
    let mut state = registry.get(session).unwrap();
    let link = Uuid::from_u128(200);
    let first = FixtureId::new();
    let second = FixtureId::new();
    let mut newest = on(link, second, 3, 0.7);
    let next_dynamic = Uuid::from_u128(300);
    if let DynamicSemanticValue::DynamicOn { dynamic, .. } = &mut newest.value {
        dynamic.dynamic_id = Some(next_dynamic);
    }
    state.dynamic_values = Arc::new(vec![on(link, first, 1, 0.5)]);
    state.preload_dynamic_active = Arc::new(vec![newest]);
    let controller = resolve_programmer_dynamic_controller(&state, link).unwrap();
    assert_eq!(controller.dynamic_id, Some(next_dynamic));
    assert_eq!(controller.targets, vec![first, second]);
    assert_eq!(effective_programmer_dynamic_values(&state).len(), 2);
}

#[test]
fn start_stores_raw_link_but_runtime_and_events_use_scoped_identity() {
    for preload in [false, true] {
        let (registry, session, context) = desk();
        if preload {
            assert!(registry.arm_preload(session, true));
        }
        let service = DynamicsService::new(registry.clone());
        let ports = Ports::default();
        let outcome = service
            .start(
                &context,
                DynamicStartCommand {
                    dynamic_id: definition().id,
                    targets: vec![FixtureId::new()],
                    overrides: DynamicInstanceOverrides {
                        size: 1.0,
                        speed_multiplier: light_dynamics::Rational::ONE,
                        phase_offset_degrees: 0.0,
                    },
                    timing: Default::default(),
                    undo_group: None,
                },
                &ports,
            )
            .unwrap();
        let state = registry.get(session).unwrap();
        let values = effective_programmer_dynamic_values(&state);
        let link = values[0].value.track_key().instance_link.unwrap();
        assert_ne!(link, outcome.controller_id);
        assert_eq!(
            outcome.controller_id,
            light_dynamics::programmer_dynamic_controller_id(state.id, link)
        );
        assert!(
            ports
                .events
                .lock()
                .iter()
                .all(|event| event.controller_id == Some(outcome.controller_id))
        );
        if preload {
            assert!(ports.starts.lock().is_empty());
            assert_eq!(outcome.runtime_instance_id, outcome.controller_id);
        } else {
            let starts = ports.starts.lock();
            assert_eq!(starts[0].controller.id, outcome.controller_id);
            assert!(matches!(starts[0].controller.source,
                DynamicControllerSource::Programmer { programmer_id, instance_link: Some(raw) }
                    if programmer_id == state.id.0 && raw == link));
        }
    }
}

#[test]
fn committed_preload_controller_accepts_raw_update_and_scoped_off() {
    let (registry, session, context) = desk();
    let link = Uuid::from_u128(200);
    let fixture = FixtureId::new();
    let mut state = registry.get(session).unwrap();
    state.preload_dynamic_active = Arc::new(vec![on(link, fixture, 1, 0.5)]);
    let scoped = light_dynamics::programmer_dynamic_controller_id(state.id, link);
    registry.restore(state);
    let service = DynamicsService::new(registry.clone());
    let ports = Ports::default();
    service
        .update_controller(
            &context,
            DynamicControllerUpdate {
                controller_id: link,
                size: Some(0.7),
                speed_multiplier: None,
                phase_offset_degrees: None,
                undo_group: None,
            },
            &ports,
        )
        .unwrap();
    assert_eq!(*ports.updates.lock(), vec![scoped]);
    let state = registry.get(session).unwrap();
    assert!(matches!(&state.dynamic_values[0].value,
        DynamicSemanticValue::DynamicOn { instance_link, overrides, .. }
            if *instance_link == link && overrides.size == 0.7));
    let result = service
        .off(
            &context,
            DynamicOffCommand {
                controller_id: scoped,
                timing: Default::default(),
            },
            &ports,
        )
        .unwrap();
    assert_eq!(result.controller_id, scoped);
    assert!(
        ports.offs.lock().is_empty(),
        "Off must first author the edit; the captured output frame owns runtime reconciliation"
    );
    assert!(
        matches!(registry.get(session).unwrap().dynamic_values[0].value,
        DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == link)
    );
}

#[test]
fn pending_off_resolves_existing_link_without_mutating_live_runtime() {
    let (registry, session, context) = desk();
    let link = Uuid::from_u128(200);
    let fixture = FixtureId::new();
    let mut state = registry.get(session).unwrap();
    state.dynamic_values = Arc::new(vec![on(link, fixture, 1, 0.5)]);
    let scoped = light_dynamics::programmer_dynamic_controller_id(state.id, link);
    registry.restore(state);
    assert!(registry.arm_preload(session, true));
    let service = DynamicsService::new(registry.clone());
    let ports = Ports::default();
    let result = service
        .off(
            &context,
            DynamicOffCommand {
                controller_id: scoped,
                timing: Default::default(),
            },
            &ports,
        )
        .unwrap();
    assert_eq!(result.controller_id, scoped);
    assert_eq!(result.runtime_instance_id, scoped);
    assert!(ports.offs.lock().is_empty());
    let state = registry.get(session).unwrap();
    assert!(matches!(
        state.dynamic_values[0].value,
        DynamicSemanticValue::DynamicOn { .. }
    ));
    assert!(matches!(state.preload_dynamic_pending[0].value,
        DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == link));
    assert!(effective_programmer_dynamic_controllers(&state).is_empty());
}

#[test]
fn targetless_start_with_nothing_selected_never_falls_back_to_every_fixture() {
    let (registry, session, context) = desk();
    let service = DynamicsService::new(registry.clone());
    let ports = Ports::default();
    let command = DynamicStartCommand {
        dynamic_id: definition().id,
        targets: Vec::new(),
        overrides: DynamicInstanceOverrides {
            size: 1.0,
            speed_multiplier: light_dynamics::Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: Default::default(),
        undo_group: None,
    };
    // The test show has no fixture with an Intensity channel: nothing is capable.
    let error = service
        .start(&context, command.clone(), &ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(ports.starts.lock().is_empty());
    assert!(registry.selection(session).unwrap().selected.is_empty());
    assert!(
        service
            .off_matching(&context, command, &ports)
            .unwrap()
            .is_none()
    );
}
