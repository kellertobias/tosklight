//! Native pages: identity, full-width descriptors, reference choice, replay preview, inertness
//! and the route contract.
use super::*;

#[tokio::test]
async fn native_pages_publish_full_width_descriptors_of_a_clearly_identified_reference() {
    let desk = Desk::new().await;
    desk.program_semantic([1.0, 0.5, 0.0]);
    let pages = desk.pages("").await;
    assert!(pages.semantic);
    assert_eq!(pages.unavailable, None);
    let reference = pages.reference.clone().expect("a verified reference head");
    assert_eq!(
        reference.fixture_id, desk.fixtures[0].0,
        "first verified head"
    );
    assert!(!reference.chosen);
    assert_eq!(reference.fixture_number, Some(1));
    assert_eq!(reference.head_id, desk.profile.modes[0].heads[0].id);
    assert_eq!(
        serde_json::to_value(&reference.identity).unwrap(),
        serde_json::to_value(ToIntentWire::to_intent_wire(&identity(&desk.profile))).unwrap()
    );
    // Four path controls on page 3, identified by channel/function UUIDs at full width.
    assert_eq!(pages.pages.len(), 1);
    assert_eq!(pages.pages[0].number, 3);
    let controls: Vec<_> = pages.pages[0].controls.iter().flatten().collect();
    let channels = path_channels(&desk.profile);
    assert_eq!(controls.len(), 4);
    for (control, channel) in controls.iter().zip(&channels) {
        assert_eq!(control.channel_id, channel.id);
        assert_eq!(control.id, format!("native.{}", channel.id));
        assert_eq!(control.functions[0].function_id, channel.functions[0].id);
    }
    // TL-653: controls carry the attribute registry's operator labels, never raw identifiers.
    let labels: Vec<_> = controls
        .iter()
        .map(|control| control.label.as_str())
        .collect();
    assert_eq!(labels, ["Red", "Green", "Blue", "White"]);
    let maxima: Vec<_> = controls.iter().map(|control| control.raw_max).collect();
    assert_eq!(maxima, [255, 65_535, 16_777_215, u32::MAX]);
    let widths: Vec<_> = controls.iter().map(|control| control.resolution).collect();
    assert_eq!(
        widths,
        [
            wire::NativeColorResolution::Bits8,
            wire::NativeColorResolution::Bits16,
            wire::NativeColorResolution::Bits24,
            wire::NativeColorResolution::Bits32,
        ]
    );
    assert!(
        controls
            .iter()
            .all(|control| control.functions[0].raw_to == control.raw_max)
    );
    assert!(pages.overflow.is_empty());
    // Every full-width value round-trips through the typed snapshot unchanged.
    let echoed: wire::NativeColorPagesSnapshot =
        serde_json::from_value(serde_json::to_value(&pages).unwrap()).unwrap();
    assert_eq!(echoed, pages);

    // Candidates and the replay preview: compatible heads replay exactly, the lookalike
    // (same names, widths and slots) is a best-effort match.
    assert_eq!(
        pages
            .candidates
            .iter()
            .map(|candidate| candidate.fixture_id)
            .collect::<Vec<_>>(),
        desk.fixtures.iter().map(|id| id.0).collect::<Vec<_>>()
    );
    let replay: Vec<_> = pages.fixtures.iter().map(|f| f.replay).collect();
    assert_eq!(
        replay,
        [
            wire::NativeColorReplayPreview::Exact,
            wire::NativeColorReplayPreview::Exact,
            wire::NativeColorReplayPreview::Fallback,
        ],
        "never matched by names or slots"
    );
    // Current values of the reference head come from the accepted frame, read-only.
    let values = pages.values.expect("the reference head outputs Color");
    assert_eq!(values.controls.len(), 4);
    assert_eq!(values.controls[0].channel_id, channels[0].id);

    // An explicit reference head is honoured; one outside the selection falls back passively.
    let chosen = desk
        .pages(&format!("&reference={}", desk.fixtures[2].0))
        .await;
    let reference = chosen.reference.unwrap();
    assert!(reference.chosen);
    assert_eq!(reference.fixture_id, desk.fixtures[2].0);
    assert_eq!(
        chosen.fixtures.iter().map(|f| f.replay).collect::<Vec<_>>(),
        [
            wire::NativeColorReplayPreview::Fallback,
            wire::NativeColorReplayPreview::Fallback,
            wire::NativeColorReplayPreview::Exact,
        ]
    );
    let stranger = desk
        .pages(&format!("&reference={}", FixtureId::new().0))
        .await;
    assert!(!stranger.reference.unwrap().chosen);
    let _ = std::fs::remove_dir_all(&desk.directory);
}

#[tokio::test]
async fn reading_and_navigating_native_pages_never_changes_the_programmer() {
    let desk = Desk::new().await;
    desk.program_semantic([0.2, 0.4, 0.6]);
    let before = (
        desk.programmers.normal_values_revision(),
        desk.programmers.undo_depth(desk.session),
        desk.value(desk.fixtures[0]),
    );
    for query in [
        String::new(),
        format!("&reference={}", desk.fixtures[1].0),
        format!(
            "&reference={}&head={}",
            desk.fixtures[2].0, desk.lookalike.modes[0].heads[0].id
        ),
    ] {
        desk.pages(&query).await;
    }
    desk.report().await;
    assert_eq!(
        (
            desk.programmers.normal_values_revision(),
            desk.programmers.undo_depth(desk.session),
            desk.value(desk.fixtures[0]),
        ),
        before,
        "pages, reference choice and reports are inert: no request, history or intent change"
    );
    let _ = std::fs::remove_dir_all(&desk.directory);
}

#[tokio::test]
async fn the_native_pages_route_is_authenticated_show_guarded_and_quiet_without_a_verified_head() {
    let desk = Desk::new().await;
    let uri = format!(
        "/api/v2/programming/color/native-pages?fixture_ids={}&future=1",
        desk.fixtures[0].0
    );
    let anonymous = desk
        .app
        .clone()
        .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let other_show = desk
        .app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header(header::AUTHORIZATION, format!("Bearer {}", desk.token))
                .header("x-tosk-show", uuid::Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other_show.status(), StatusCode::CONFLICT);
    for bad in [
        "/api/v2/programming/color/native-pages?fixture_ids=nope",
        "/api/v2/programming/color/native-pages?reference=nope",
        &format!(
            "/api/v2/programming/color/native-pages?fixture_ids={}&head={}",
            desk.fixtures[0].0,
            uuid::Uuid::new_v4()
        ),
    ] {
        assert_eq!(
            desk.get(bad).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    // No verified head: quiet, typed, not an error. No accepted frame: no values.
    let unknown: wire::NativeColorPagesSnapshot = json_body(
        desk.get(&format!(
            "/api/v2/programming/color/native-pages?fixture_ids={}",
            FixtureId::new().0
        ))
        .await,
    )
    .await;
    assert_eq!(
        unknown.unavailable,
        Some(wire::NativeColorPagesUnavailable::NoVerifiedHead)
    );
    assert!(unknown.pages.is_empty() && unknown.reference.is_none());
    let idle = desk.pages("").await;
    assert!(idle.reference.is_some() && idle.values.is_none());
    let _ = std::fs::remove_dir_all(&desk.directory);
}

#[test]
fn native_control_labels_come_from_the_attribute_registry() {
    use light_core::{AttributeKey, CustomAttributeDescriptor};
    let key = |id: &str| AttributeKey(id.into());
    let custom = [CustomAttributeDescriptor {
        id: key("fixture.color_scene"),
        label: "Color Scene".into(),
        value_type: light_core::AttributeValueType::Continuous,
        display_unit: None,
        physical_unit: None,
        normalized_bounds: None,
        domain_bounds: None,
        cyclic: false,
        recordable: true,
        lifecycle: Default::default(),
    }];
    let label = |own: &str, canonical: &str| {
        crate::runtime::native_color_pages::control_label(&custom, &key(own), &key(canonical))
    };
    assert_eq!(label("color.red", "color.red"), "Red");
    assert_eq!(label("color.wheel.1", "color.wheel.1"), "Color Wheel 1");
    assert_eq!(
        label("color.temperature", "color.temperature"),
        "Color Temperature"
    );
    // A manufacturer attribute takes its canonical attribute's label.
    assert_eq!(label("vendor.red_led", "color.red"), "Red");
    // A configured custom attribute takes its configured label; an unknown one keeps its id.
    assert_eq!(
        label("fixture.color_scene", "fixture.color_scene"),
        "Color Scene"
    );
    assert_eq!(label("vendor.mystery", "vendor.mystery"), "vendor.mystery");
}
