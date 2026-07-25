use super::*;

#[test]
fn interaction_routes_are_exactly_desk_and_object_scoped() {
    let setup = LiveSetup::new(8);
    setup.press(CommandKey::Digit(1));

    let object = EventObject::programming_command_line(setup.context.desk_id);
    assert_eq!(object.capability, EventCapability::Desk);
    assert_eq!(
        object.id,
        format!("programming-command-line:{}", setup.context.desk_id)
    );
    let matching = setup.events.replay(0, &setup.command_filter());
    assert!(matches!(matching, EventReplay::Events(events) if events.len() == 1));

    assert_non_matching_routes(&setup);
}

fn assert_non_matching_routes(setup: &LiveSetup) {
    let programmer_scope =
        EventFilter::for_desk(setup.context.desk_id).with_capability(EventCapability::Programmer);
    assert!(matches!(
        setup.events.replay(0, &programmer_scope),
        EventReplay::Events(events) if events.is_empty()
    ));
    assert_wrong_desk_and_object(setup);

    let legacy_object = EventFilter::for_desk(setup.context.desk_id).with_object(EventObject::new(
        EventCapability::Desk,
        format!("programming-interaction:{}", setup.context.desk_id),
    ));
    assert!(matches!(
        setup.events.replay(0, &legacy_object),
        EventReplay::Events(events) if events.is_empty()
    ));
}

fn assert_wrong_desk_and_object(setup: &LiveSetup) {
    let other_desk = Uuid::new_v4();
    let wrong_desk = EventFilter::for_desk(other_desk)
        .with_object(EventObject::programming_command_line(setup.context.desk_id));
    assert!(matches!(
        setup.events.replay(0, &wrong_desk),
        EventReplay::Events(events) if events.is_empty()
    ));
    let wrong_object = EventFilter::for_desk(setup.context.desk_id)
        .with_object(EventObject::programming_command_line(other_desk));
    assert!(matches!(
        setup.events.replay(0, &wrong_object),
        EventReplay::Events(events) if events.is_empty()
    ));
}

#[test]
fn combined_change_is_delivered_once_through_each_exact_route() {
    let bus = EventBus::new(8);
    let desk_id = Uuid::new_v4();
    let context = action_context(desk_id);
    let command = EventObject::programming_command_line(desk_id);
    let selection = EventObject::programming_selection(desk_id);
    let subscriptions = subscriptions(&bus, desk_id, &command, &selection);

    let published = bus.publish(crate::EventDraft::programming_interaction_changed(
        &context,
        both_components(desk_id),
    ));
    assert_eq!(published.object, Some(command));
    assert_eq!(published.related_objects, vec![selection]);
    for subscription in subscriptions {
        assert!(matches!(
            subscription.try_next(),
            Some(SubscriptionDelivery::Event(event)) if event.sequence == published.sequence
        ));
        assert!(subscription.try_next().is_none());
    }
}

fn subscriptions(
    bus: &EventBus,
    desk_id: Uuid,
    command: &EventObject,
    selection: &EventObject,
) -> [crate::EventSubscription; 3] {
    let options = SubscriptionOptions::default();
    [
        bus.subscribe(
            EventFilter::for_desk(desk_id).with_object(command.clone()),
            options.clone(),
        ),
        bus.subscribe(
            EventFilter::for_desk(desk_id).with_object(selection.clone()),
            options.clone(),
        ),
        bus.subscribe(
            EventFilter::for_desk(desk_id)
                .with_object(command.clone())
                .with_object(selection.clone()),
            options,
        ),
    ]
}

#[test]
fn interaction_change_rejects_empty_and_cross_desk_components() {
    let desk_id = Uuid::new_v4();
    assert!(ProgrammingInteractionChange::from_components(desk_id, None, None).is_none());
    let before = ProgrammingInteractionProjection {
        desk_id,
        command_line: Default::default(),
        selection: Default::default(),
    };
    assert!(ProgrammingInteractionChange::between(&before, &before).is_none());
    let mut other_desk = before.clone();
    other_desk.desk_id = Uuid::new_v4();
    assert!(ProgrammingInteractionChange::between(&before, &other_desk).is_none());
}

#[test]
fn sparse_components_are_lossless_when_multiple_views_share_one_subscription() {
    let bus = EventBus::new(8);
    let desk_id = Uuid::new_v4();
    let context = action_context(desk_id);
    let subscription = bus.subscribe(
        EventFilter::for_desk(desk_id)
            .with_object(EventObject::programming_command_line(desk_id))
            .with_object(EventObject::programming_selection(desk_id)),
        SubscriptionOptions {
            capacity: 4,
            ..SubscriptionOptions::default()
        },
    );
    bus.publish(crate::EventDraft::programming_interaction_changed(
        &context,
        both_components(desk_id),
    ));
    bus.publish(crate::EventDraft::programming_interaction_changed(
        &context,
        ProgrammingInteractionChange::from_components(desk_id, Some(Default::default()), None)
            .unwrap(),
    ));

    let deliveries = (0..2).map(|_| subscription.try_next()).collect::<Vec<_>>();
    assert!(
        deliveries
            .iter()
            .all(|delivery| { matches!(delivery, Some(SubscriptionDelivery::Event(_))) })
    );
    let Some(SubscriptionDelivery::Event(first)) = &deliveries[0] else {
        panic!("the combined sparse change should remain queued")
    };
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &first.payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert!(change.selection().is_some());
}

fn action_context(desk_id: Uuid) -> ActionContext {
    ActionContext::operator(
        desk_id,
        UserId::new().0,
        SessionId::new().0,
        ActionSource::UserInterface,
    )
}

fn both_components(desk_id: Uuid) -> ProgrammingInteractionChange {
    ProgrammingInteractionChange::from_components(
        desk_id,
        Some(Default::default()),
        Some(ProgrammerSelection::default()),
    )
    .unwrap()
}
