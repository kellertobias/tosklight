use super::*;

fn source(sequence: u64) -> Arc<super::super::visualization_frame::PublishedVisualizationFrame> {
    Arc::new(
        super::super::visualization_frame::PublishedVisualizationFrame {
            sequence,
            generated_at: std::time::SystemTime::now(),
            scope: VisualizationScope { show_id: None },
            show_revision: sequence,
            options: light_engine::RenderOptions::default(),
            values: light_engine::FrameValues::empty(),
            profile_visualization_values: Arc::new(light_engine::Pooled::default()),
        },
    )
}

fn scope(id: &str) -> VisualizationScope {
    VisualizationScope {
        show_id: Some(uuid::Uuid::parse_str(id).unwrap()),
    }
}

#[test]
fn same_revision_show_switch_requires_structural_invalidation() {
    let first = scope("11111111-1111-4111-8111-111111111111");
    let replacement = scope("22222222-2222-4222-8222-222222222222");

    assert!(!structural_scope_changed(Some((first, 7)), first, 7));
    assert!(structural_scope_changed(Some((first, 7)), replacement, 7));
    assert!(structural_scope_changed(Some((first, 7)), first, 8));
}

#[test]
fn client_messages_tolerate_unknown_fields_through_the_logged_decoder() {
    let message = decode_client_message(
        r#"{
            "type":"subscribe",
            "lanes":["normal"],
            "max_rate_hz":10,
            "future":{"secret":"must-not-be-logged"}
        }"#,
    )
    .unwrap();

    assert_eq!(
        message,
        VisualizationClientMessage::Subscribe {
            lanes: vec![VisualizationLane::Normal],
            max_rate_hz: 10,
            acknowledgements: false,
            include_dynamic_stack: false,
            sparse_dynamic_stack: false,
            batched_messages: false,
        }
    );
}

#[test]
fn visualization_client_messages_have_a_small_transport_bound() {
    assert_eq!(MAX_VISUALIZATION_CLIENT_MESSAGE_BYTES, 16 * 1024);
}

#[test]
fn negotiated_lane_batch_uses_one_websocket_text_frame() {
    let message = || {
        VisualizationOutgoingMessage::Server(VisualizationServerMessage::Error {
            code: "test".into(),
            message: "message".into(),
        })
    };
    let batched = serialize_outgoing_messages(vec![message(), message()], true).unwrap();
    assert_eq!(batched.len(), 1);
    let Message::Text(payload) = &batched[0] else {
        panic!("lane batch must be text");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(payload)
            .unwrap()
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let legacy = serialize_outgoing_messages(vec![message(), message()], false).unwrap();
    assert_eq!(legacy.len(), 2);
}

#[tokio::test]
async fn stalled_client_queue_retains_only_the_latest_complete_batch() {
    let queue = LatestBatch::default();
    assert_eq!(
        queue
            .replace(
                vec![VisualizationOutgoingMessage::Raw(Message::Text(
                    "stale".into(),
                ))],
                vec![VisualizationOutgoingMessage::Raw(Message::Text(
                    "first replacement".into(),
                ))],
            )
            .await,
        Some(false)
    );
    assert_eq!(
        queue
            .replace(
                vec![VisualizationOutgoingMessage::Raw(Message::Text(
                    "incoherent delta".into(),
                ))],
                vec![VisualizationOutgoingMessage::Raw(Message::Text(
                    "latest snapshot".into(),
                ))],
            )
            .await,
        Some(true)
    );

    let batch = queue.next().await.unwrap();
    assert_eq!(
        batch,
        vec![VisualizationOutgoingMessage::Raw(Message::Text(
            "latest snapshot".into()
        ))]
    );
    assert!(queue.pending.lock().await.is_none());
}

#[test]
fn negotiated_acknowledgements_hold_publication_until_the_batch_is_processed() {
    let mut publication = ClientPublicationState::new();
    publication.set_acknowledgements(true);
    publication
        .unacknowledged_sequences
        .extend(1..=MAX_UNACKNOWLEDGED_PUBLICATIONS as u64);

    assert!(!publication.can_publish());
    publication.acknowledge(1);
    assert!(publication.can_publish());
    assert_eq!(
        publication.unacknowledged_sequences,
        VecDeque::from_iter(2..=MAX_UNACKNOWLEDGED_PUBLICATIONS as u64)
    );

    publication.set_acknowledgements(false);
    assert!(publication.can_publish());
    assert!(publication.unacknowledged_sequences.is_empty());
}

#[tokio::test(start_paused = true)]
async fn ten_hz_notification_retains_the_client_send_cap() {
    let mut throttle = ClientRateThrottle::new();
    throttle.set_rate(10);
    let first = throttle.observe(source(1), Instant::now()).unwrap();
    assert_eq!(first.sequence, 1);
    throttle.mark_sent(Instant::now());

    tokio::time::advance(Duration::from_millis(1)).await;
    assert!(throttle.observe(source(2), Instant::now()).is_none());
    tokio::time::advance(Duration::from_millis(99)).await;
    let second = throttle.take_due(Instant::now()).unwrap();
    assert_eq!(second.sequence, 2);
}

#[tokio::test(start_paused = true)]
async fn lower_rate_throttle_does_not_accumulate_projection_delay() {
    let mut throttle = ClientRateThrottle::new();
    throttle.set_rate(5);
    assert_eq!(
        throttle
            .observe(source(1), Instant::now())
            .unwrap()
            .sequence,
        1
    );
    throttle.mark_sent(Instant::now());

    tokio::time::advance(Duration::from_millis(210)).await;
    assert_eq!(
        throttle
            .observe(source(2), Instant::now())
            .unwrap()
            .sequence,
        2
    );
    throttle.mark_sent(Instant::now());

    tokio::time::advance(Duration::from_millis(189)).await;
    assert!(throttle.observe(source(3), Instant::now()).is_none());
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 3);
}

#[tokio::test(start_paused = true)]
async fn lower_rate_throttle_keeps_the_newest_notified_source_until_due() {
    let mut throttle = ClientRateThrottle::new();
    throttle.set_rate(2);
    assert_eq!(
        throttle
            .observe(source(1), Instant::now())
            .unwrap()
            .sequence,
        1
    );
    throttle.mark_sent(Instant::now());

    tokio::time::advance(Duration::from_millis(100)).await;
    assert!(throttle.observe(source(2), Instant::now()).is_none());
    tokio::time::advance(Duration::from_millis(100)).await;
    assert!(throttle.observe(source(3), Instant::now()).is_none());
    tokio::time::advance(Duration::from_millis(299)).await;
    assert!(throttle.take_due(Instant::now()).is_none());
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 3);
}

#[tokio::test(start_paused = true)]
async fn non_divisor_rate_never_exceeds_the_requested_maximum() {
    let mut throttle = ClientRateThrottle::new();
    throttle.set_rate(3);
    assert_eq!(
        throttle
            .observe(source(1), Instant::now())
            .unwrap()
            .sequence,
        1
    );
    throttle.mark_sent(Instant::now());

    tokio::time::advance(Duration::from_millis(333)).await;
    assert!(throttle.observe(source(2), Instant::now()).is_none());
    tokio::time::advance(Duration::from_micros(334)).await;
    assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 2);
}

#[test]
fn stage_stream_keeps_only_attributes_consumed_by_the_renderer() {
    for attribute in [
        "intensity",
        "pan",
        "tilt",
        "zoom",
        "focus",
        "beam.zoom",
        "beam.focus",
        "gobo",
        "color",
        "color.red",
        "color.green",
        "color.blue",
    ] {
        assert!(stage_visualization_attribute(attribute), "{attribute}");
    }
    for attribute in ["shutter", "strobe", "media.playback", "control.reset"] {
        assert!(!stage_visualization_attribute(attribute), "{attribute}");
    }
}
