use super::*;
use light_core::{
    AttributeKey, AttributeValue, ReplacementProfileContext, ReplacementProgramProjection,
};
use std::collections::HashMap;

fn projection(owner: FixtureId) -> ReplacementProgramProjection {
    let profile = ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: Uuid::new_v4(),
    };
    ReplacementProgramProjection {
        source_owner: owner,
        source_profile: profile.clone(),
        source_head_id: Uuid::new_v4(),
        target_profile: profile,
        targets: Vec::new(),
    }
}

#[test]
fn replacement_metadata_publication_is_sparse_session_free_and_does_not_relabel_scalar_edits() {
    let setup = LiveSetup::new(16);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
    let owner = FixtureId::new();
    let other = FixtureId::new();
    registry.set_many(
        session,
        [
            (
                owner,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.5),
            ),
            (
                other,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.7),
            ),
        ],
    );
    let before = registry.get(session).unwrap();
    let original = before
        .values
        .iter()
        .find(|value| value.fixture_id == owner)
        .unwrap();
    let metadata = projection(owner);
    assert!(registry.apply_replacement_migration(
        before.id,
        &[(
            original.programmer_order,
            HashMap::from([(owner, metadata.clone())])
        )]
    ));
    let context = ActionContext::system(setup.context.desk_id, ActionSource::UserInterface);
    let sequence = setup
        .service
        .publish_replacement_migration_values(&context, &before)
        .unwrap();
    assert_eq!(sequence, 1);
    assert_eq!(registry.normal_values_revision(), 1);
    let EventReplay::Events(events) = setup.events.replay(
        0,
        &EventFilter::for_desk(context.desk_id).with_object(EventObject::programming_values()),
    ) else {
        panic!("expected sparse normal event")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::ValuesChanged(change)) = &events[0].payload
    else {
        panic!("expected values event")
    };
    assert_eq!(change.delta.fixture_values.len(), 1);
    let row = &change.delta.fixture_values[0];
    assert_eq!(row.fixture_id, owner);
    assert_eq!(row.programmer_order, original.programmer_order);
    assert_eq!(row.value, original.value);
    assert_eq!(row.replacement_projection.as_ref(), Some(&metadata));
    assert!(change.delta.group_values.is_empty());
    assert!(change.delta.removed_fixture_values.is_empty());
    let unchanged = registry.get(session).unwrap();
    assert_eq!(
        setup
            .service
            .publish_replacement_migration_values(&context, &unchanged),
        None
    );
    assert_eq!(registry.normal_values_revision(), 1);
    assert_eq!(setup.events.latest_sequence(), 1);
    let mut stale = unchanged;
    stale.id = light_core::ProgrammerId::new();
    assert_eq!(
        setup
            .service
            .publish_replacement_migration_values(&context, &stale),
        None
    );
    assert_eq!(setup.events.latest_sequence(), 1);
    assert!(registry.knows_session(session));
}

#[test]
fn replacement_pending_and_active_only_metadata_publish_no_identical_normal_event() {
    for active in [false, true] {
        let setup = LiveSetup::new(16);
        let registry = setup.ports.registry.as_ref().unwrap();
        let session = SessionId(setup.context.session_id.unwrap());
        let owner = FixtureId::new();
        assert!(registry.arm_preload(session, true));
        assert!(registry.apply_preload_values(
            session,
            &[
                light_programmer::PreloadProgrammerValueMutation::SetFixture {
                    fixture_id: owner,
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Normalized(0.5),
                    timing: Default::default(),
                }
            ]
        ));
        if active {
            assert!(registry.activate_preload(session));
        }
        let before = registry.get(session).unwrap();
        let order = if active {
            before.preload_active[0].programmer_order
        } else {
            before.preload_pending[0].programmer_order
        };
        assert!(registry.apply_replacement_migration(
            before.id,
            &[(order, HashMap::from([(owner, projection(owner))]))]
        ));
        let context = ActionContext::system(setup.context.desk_id, ActionSource::UserInterface);
        assert_eq!(
            setup
                .service
                .publish_replacement_migration_values(&context, &before),
            None
        );
        assert_eq!(registry.normal_values_revision(), 0);
        assert_eq!(setup.events.latest_sequence(), 0);
    }
}
