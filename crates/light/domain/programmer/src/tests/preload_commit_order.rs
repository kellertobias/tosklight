use super::*;
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, DynamicValueTiming};
use uuid::Uuid;

fn lane_on(link: Uuid, size: f32) -> DynamicSemanticValue {
    serde_json::from_value(serde_json::json!({
        "type":"dynamic_on", "instance_link":link, "lane_id":Uuid::from_u128(1),
        "dynamic":{"dynamic_id":null,"last_known_pool_number":1,"embedded_fallback":{"definition":{
            "id":Uuid::from_u128(100),"pool_number":1,"revision":1,"name":"Position",
            "target_binding":{"type":"targetless"},"lanes":[{
                "id":Uuid::from_u128(1),"speed_multiplier":{"numerator":1,"denominator":1},"width":1.0,
                "programming":{"address":{"representation":{"kind":"angles"},"component":{"kind":"pan"}},
                    "configuration":{"mode":"keyframes","configuration":{"points":[
                        {"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":45.0}},"interpolation":"linear"}
                    ],"size":1.0}}}
            }],"phase":{"ordering":{"type":"selection"},"offset_degrees":0,"span_degrees":360,
                "block_size":1,"repeats":1,"wings":false},
            "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
        }}},"overrides":{"size":size,"speed_multiplier":{"numerator":1,"denominator":1},"phase_offset_degrees":0},
        "timing":{}
    })).unwrap()
}

#[test]
fn go_commits_an_older_pending_off_after_a_newer_live_on_and_undo_restores_both() {
    let started = chrono::DateTime::from_timestamp_millis(10_000).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    registry.start(session);
    let set = |value| DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: AttributeKey("position".into()),
        value,
    };
    assert!(registry.apply_dynamic_values(session, &[set(lane_on(link, 1.0))], None));
    assert!(registry.arm_preload(session, true));
    clock.advance_millis(10);
    assert!(registry.apply_dynamic_values(
        session,
        &[set(DynamicSemanticValue::DynamicOff {
            instance_link: link,
            timing: DynamicValueTiming::default(),
        })],
        None
    ));
    assert!(registry.arm_preload(session, false));
    clock.advance_millis(10);
    assert!(registry.apply_dynamic_values(session, &[set(lane_on(link, 0.5))], None));
    let before = registry.get(session).unwrap();
    let pending_order = before.preload_dynamic_pending[0].programmer_order;
    let live_order = before.dynamic_values[0].programmer_order;
    assert!(pending_order < live_order);

    clock.advance_millis(10);
    let committed_at = started + chrono::Duration::milliseconds(30);
    assert!(registry.activate_preload_at(session, committed_at));
    let committed = registry.get(session).unwrap();
    let applied = &committed.preload_dynamic_active[0];
    assert!(applied.programmer_order > live_order);
    assert_eq!(applied.changed_at_millis, 10_030);
    let effective = light_dynamics::merge_dynamic_address_values(
        committed
            .dynamic_values
            .iter()
            .chain(committed.preload_dynamic_active.iter()),
    );
    assert_eq!(effective, vec![applied]);
    assert!(
        matches!(effective[0].value, DynamicSemanticValue::DynamicOff { instance_link, .. }
        if instance_link == link)
    );
    assert_eq!(committed.dynamic_values, before.dynamic_values);

    assert!(registry.undo(session));
    let undone = registry.get(session).unwrap();
    assert_eq!(
        undone.preload_dynamic_pending,
        before.preload_dynamic_pending
    );
    assert_eq!(undone.dynamic_values, before.dynamic_values);
    assert!(registry.redo(session));
    assert_eq!(
        registry.get(session).unwrap().preload_dynamic_active,
        committed.preload_dynamic_active
    );
}

#[test]
fn go_preserves_cross_lane_order_shared_edits_and_complete_intents() {
    let at = chrono::DateTime::from_timestamp_millis(10_000).unwrap();
    let registry = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(at)));
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let mut state = registry.get(session).unwrap();
    let mut ordinary = state.values[0].clone();
    ordinary.programmer_order = 20;
    let intent = AttributeValue::Position(Arc::new(
        light_core::programming::PositionIntent::angles(540.0, 42.0),
    ));
    let mut position = ordinary.clone();
    position.attribute = AttributeKey("position".into());
    position.value = intent.clone();
    position.programmer_order = 21;
    state.preload_pending = vec![position, ordinary];
    let group = |order| GroupProgrammerValue {
        value: AttributeValue::Normalized(0.75),
        changed_at: at,
        programmer_order: order,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    };
    state.preload_group_pending = HashMap::from([
        (
            "z-last".into(),
            HashMap::from([(AttributeKey::intensity(), group(30))]),
        ),
        (
            "a-first".into(),
            HashMap::from([(AttributeKey::intensity(), group(10))]),
        ),
    ]);
    state.preload_dynamic_pending = Arc::new(vec![DynamicAddressValue {
        fixture_id: fixture,
        attribute: AttributeKey("focus".into()),
        value: DynamicSemanticValue::FixAt {
            value: 0.25,
            timing: Default::default(),
        },
        programmer_order: 21,
        changed_at_millis: 10_000,
    }]);
    state.preload_group_release_pending = vec![GroupReleaseProgrammerValue {
        group_id: "release".into(),
        attribute: AttributeKey("color".into()),
        programmer_order: 40,
        changed_at_millis: 10_000,
    }];
    Arc::make_mut(&mut state.values)[0].programmer_order = 100;
    registry.restore(state);
    let go = at + chrono::Duration::seconds(1);
    assert!(registry.activate_preload_at_with_fade(session, go, 500));
    let committed = registry.get(session).unwrap();
    let ordinary = committed
        .preload_active
        .iter()
        .find(|row| row.attribute.is_intensity())
        .unwrap();
    let position = committed
        .preload_active
        .iter()
        .find(|row| row.attribute.0.as_ref() == "position")
        .unwrap();
    let orders = [
        committed.preload_group_active["a-first"][&AttributeKey::intensity()].programmer_order,
        ordinary.programmer_order,
        position.programmer_order,
        committed.preload_group_active["z-last"][&AttributeKey::intensity()].programmer_order,
    ];
    assert!(orders[0] > 100);
    assert!(orders.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        position.programmer_order,
        committed.preload_dynamic_active[0].programmer_order
    );
    assert_eq!(position.value, intent);
    assert_eq!(position.changed_at, go);
    assert_eq!(position.fade_millis, Some(500));
    assert_eq!(
        committed.preload_dynamic_active[0].changed_at_millis,
        11_000
    );
    assert_eq!(
        committed.preload_group_release_active[0].changed_at_millis,
        10_000
    );
    assert_eq!(
        committed.preload_group_release_active[0].programmer_order,
        40
    );
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    assert!(registry.get(session).unwrap().values[0].programmer_order > orders[3]);
}

#[test]
fn go_release_cutoffs_preserve_intervening_live_fixture_and_group_edits() {
    for legacy in [false, true] {
        let at = chrono::DateTime::from_timestamp_millis(10_000).unwrap();
        let clock = Arc::new(ManualClock::new(at));
        let registry = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let fixture = FixtureId::new();
        let attribute = AttributeKey::intensity();
        registry.start(session);
        registry.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.5),
        );
        registry.set_group(
            session,
            "front".into(),
            attribute.clone(),
            AttributeValue::Normalized(0.5),
        );
        let mut older = registry.get(session).unwrap();
        if legacy {
            Arc::make_mut(&mut older.values)[0].programmer_order = 0;
            Arc::make_mut(&mut older.group_values)
                .get_mut("front")
                .unwrap()
                .get_mut(&attribute)
                .unwrap()
                .programmer_order = 0;
            registry.restore(older.clone());
            clock.advance_millis(10);
        }
        registry.arm_preload(session, true);
        registry.apply_release_values(
            session,
            &[ReleaseProgrammerFixtureValue {
                fixture_id: fixture,
                attribute: attribute.clone(),
            }],
            &[ReleaseProgrammerGroupValue {
                group_id: "front".into(),
                attribute: attribute.clone(),
            }],
        );
        if legacy {
            let mut released = registry.get(session).unwrap();
            Arc::make_mut(&mut released.preload_dynamic_pending)[0].programmer_order = 0;
            released.preload_group_release_pending[0].programmer_order = 0;
            registry.restore(released);
            clock.advance_millis(10);
        }
        let prepared = registry.get(session).unwrap();
        registry.arm_preload(session, false);
        registry.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.7),
        );
        registry.set_group(
            session,
            "front".into(),
            attribute.clone(),
            AttributeValue::Normalized(0.7),
        );
        let mut newer = registry.get(session).unwrap();
        if legacy {
            Arc::make_mut(&mut newer.values)[0].programmer_order = 0;
            Arc::make_mut(&mut newer.group_values)
                .get_mut("front")
                .unwrap()
                .get_mut(&attribute)
                .unwrap()
                .programmer_order = 0;
            registry.restore(newer.clone());
        }
        clock.advance_millis(5_000);
        registry.activate_preload(session);
        let committed = registry.get(session).unwrap();
        assert_eq!(
            committed.preload_dynamic_active,
            prepared.preload_dynamic_pending
        );
        assert_eq!(
            committed.preload_group_release_active.as_ref(),
            &prepared.preload_group_release_pending
        );
        assert_eq!(committed.values, newer.values);
        assert_eq!(committed.group_values, newer.group_values);
        let fixture_release = &committed.preload_dynamic_active[0];
        let group_release = &committed.preload_group_release_active[0];
        for (millis, order, older_time, older_order, newer_time, newer_order) in [
            (
                fixture_release.changed_at_millis,
                fixture_release.programmer_order,
                older.values[0].changed_at,
                older.values[0].programmer_order,
                newer.values[0].changed_at,
                newer.values[0].programmer_order,
            ),
            (
                group_release.changed_at_millis,
                group_release.programmer_order,
                older.group_values["front"][&attribute].changed_at,
                older.group_values["front"][&attribute].programmer_order,
                newer.group_values["front"][&attribute].changed_at,
                newer.group_values["front"][&attribute].programmer_order,
            ),
        ] {
            let cutoff = light_core::ProgrammerEditStamp {
                changed_at: chrono::DateTime::from_timestamp_millis(millis as i64).unwrap(),
                programmer_order: order,
            };
            assert!(
                cutoff.supersedes(older_time, older_order),
                "the old source stays suppressed"
            );
            assert!(
                !cutoff.supersedes(newer_time, newer_order),
                "a prepared Release must not cut off the intervening Live edit"
            );
        }
        registry.release_fixture_attribute(session, fixture, &attribute);
        registry.release_group_attribute(session, "front", &attribute);
        let cleared = registry.get(session).unwrap();
        assert_eq!(
            cleared.preload_dynamic_active,
            committed.preload_dynamic_active
        );
        assert_eq!(
            cleared.preload_group_release_active,
            committed.preload_group_release_active
        );
    }
}

#[test]
fn typed_color_release_keeps_original_component_cutoff_after_go() {
    use light_core::programming::{
        ColorComponent, ColorIntent, ColorProgram, ProgrammingComponent,
    };
    let at = chrono::DateTime::from_timestamp_millis(10_000).unwrap();
    let registry = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(at)));
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let color = AttributeKey("color".into());
    let component = Some(ProgrammingComponent::Color(ColorComponent::Uv));
    let hold = |uv| {
        let mut intent = ColorIntent::default();
        intent.uv.amount = uv;
        let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
        let mut address = light_dynamics::DynamicValueAddress::whole_family(
            light_core::programming::ProgrammingOwner::Color,
            &family,
        )
        .unwrap();
        address.component = component;
        DynamicSemanticValue::ProgrammingFixAt {
            mask: light_dynamics::ProgrammingFamilyFixAt { address, family },
            timing: Default::default(),
        }
    };
    let edit = |value| DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: color.clone(),
        value,
    };
    registry.start(session);
    registry.apply_dynamic_values(session, &[edit(hold(0.5))], None);
    let older = registry.get(session).unwrap().dynamic_values[0].clone();
    registry.arm_preload(session, true);
    registry.apply_dynamic_values(
        session,
        &[edit(DynamicSemanticValue::ProgrammingRelease { component })],
        None,
    );
    let release = registry.get(session).unwrap().preload_dynamic_pending[0].clone();
    registry.arm_preload(session, false);
    registry.apply_dynamic_values(session, &[edit(hold(0.7))], None);
    let newer = registry.get(session).unwrap().dynamic_values[0].clone();
    registry.activate_preload_at(session, at + chrono::Duration::seconds(5));
    let committed = registry.get(session).unwrap();
    assert_eq!(committed.preload_dynamic_active[0], release);
    assert_eq!(
        light_dynamics::merge_dynamic_address_values([&older, &release, &newer]),
        vec![&newer]
    );
    assert_eq!(
        light_dynamics::merge_dynamic_address_values([&older, &release]),
        vec![&release]
    );
}

#[test]
fn legacy_pending_order_uses_exact_timestamp_before_go_unifies_the_commit_time() {
    let at = chrono::DateTime::from_timestamp_millis(1_000).unwrap();
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let mut state = registry.get(session).unwrap();
    let mut first = state.values[0].clone();
    first.programmer_order = 0;
    first.changed_at = at + chrono::Duration::nanoseconds(1);
    let mut second = first.clone();
    second.fixture_id = FixtureId::new();
    second.changed_at = at + chrono::Duration::nanoseconds(2);
    state.preload_pending = vec![second, first];
    state.preload_dynamic_pending = Arc::new(vec![DynamicAddressValue {
        fixture_id: fixture,
        attribute: AttributeKey("focus".into()),
        value: DynamicSemanticValue::FixAt {
            value: 0.25,
            timing: Default::default(),
        },
        programmer_order: 0,
        changed_at_millis: 1_000,
    }]);
    registry.restore(state);
    assert!(registry.activate_preload_at(session, at + chrono::Duration::seconds(1)));
    let committed = registry.get(session).unwrap();
    assert!(
        committed.preload_dynamic_active[0].programmer_order
            < committed.preload_active[1].programmer_order
    );
    assert!(
        committed.preload_active[1].programmer_order < committed.preload_active[0].programmer_order
    );
}

fn commit_mixed_orders(stamps: &[(u64, i64)]) -> Vec<u64> {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    registry.set(
        session,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let mut state = registry.get(session).unwrap();
    let template = state.values[0].clone();
    let fixtures = stamps.iter().map(|_| FixtureId::new()).collect::<Vec<_>>();
    state.preload_pending = stamps
        .iter()
        .zip(&fixtures)
        .map(|(&(order, millis), fixture)| {
            let mut value = template.clone();
            value.fixture_id = *fixture;
            value.programmer_order = order;
            value.changed_at = chrono::DateTime::from_timestamp_millis(millis).unwrap();
            value
        })
        .collect();
    registry.restore(state);
    assert!(registry.activate_preload_at(
        session,
        chrono::DateTime::from_timestamp_millis(1_000).unwrap()
    ));
    let committed = registry.get(session).unwrap();
    fixtures
        .iter()
        .map(|fixture| {
            committed
                .preload_active
                .iter()
                .find(|row| row.fixture_id == *fixture)
                .unwrap()
                .programmer_order
        })
        .collect()
}

#[test]
fn mixed_pending_orders_keep_acyclic_legacy_timestamp_precedence() {
    // Legacy at t=200 lies between the two ordered edits; it must not move before order 1.
    let orders = commit_mixed_orders(&[(2, 300), (0, 200), (1, 100)]);
    assert!(orders[2] < orders[1]);
    assert!(orders[1] < orders[0]);
}

#[test]
fn cyclic_mixed_pending_orders_normalize_only_the_cycle_deterministically() {
    // order 2 supersedes order 1, legacy@200 supersedes order2@100, and order1@300
    // supersedes legacy@200. There is no scalar ordering preserving all three relations.
    // The documented cycle rule uses timestamp, while outside predecessors/successors remain.
    let orders = commit_mixed_orders(&[(1, 300), (2, 100), (0, 200), (0, 50), (3, 400)]);
    assert!(orders[3] < orders[1]);
    assert!(orders[1] < orders[2]);
    assert!(orders[2] < orders[0]);
    assert!(orders[0] < orders[4]);
    let repeated = commit_mixed_orders(&[(1, 300), (2, 100), (0, 200), (0, 50), (3, 400)]);
    assert_eq!(orders, repeated);
}
