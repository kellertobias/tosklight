//! The first native edit seeds the displayed premaster output once and atomically selects
//! Direct for the whole mixed selection; the output replays it exactly on compatible heads and
//! best effort on the lookalike, and the colour report says which, per head.
use super::*;
use light_wire::v2::native_color::{ColorIntentDirectCompatibility, ColorIntentDirectReplay};

#[tokio::test]
async fn the_first_native_edit_adopts_the_displayed_premaster_output_once() {
    let desk = Desk::new().await;
    desk.program_semantic([1.0, 0.5, 0.25]);
    let pages = desk.pages("").await;
    let shown: Vec<u32> = pages
        .values
        .expect("displayed premaster values")
        .controls
        .iter()
        .map(|value| value.raw)
        .collect();
    let lease = desk.lease().await;
    // The displayed frame stays authoritative even after a newer frame is accepted.
    desk.program_semantic([0.0, 0.0, 1.0]);
    let depth = desk.programmers.undo_depth(desk.session);

    let first = desk
        .apply(
            "native-1",
            desk.native_edit(1, json!({"kind": "relative", "value": 1000}), Some(lease)),
        )
        .await;
    assert_eq!(first["status"], "changed", "{first}");
    assert!(first.get("hold").is_none(), "{first}");
    let mut expected = shown.clone();
    expected[1] += 1000;
    for fixture in desk.fixtures {
        assert_eq!(
            desk.recipe(fixture),
            expected,
            "one shared reference recipe: every participating channel seeded from the shown frame"
        );
    }
    // Later samples of the same gesture edit in place; they never reseed.
    desk.publish();
    desk.apply(
        "native-2",
        desk.native_edit(1, json!({"kind": "relative", "value": 1000}), Some(lease)),
    )
    .await;
    desk.apply(
        "native-3",
        desk.native_edit(3, json!({"kind": "set", "value": u32::MAX}), Some(lease)),
    )
    .await;
    expected[1] += 1000;
    expected[3] = u32::MAX;
    assert_eq!(
        desk.recipe(desk.fixtures[0]),
        expected,
        "full 32-bit width kept"
    );
    desk.apply(
        "finish",
        json!({"type": "finish_gesture", "attribute": "color", "undo_group": "native-turn"}),
    )
    .await;
    assert_eq!(
        desk.programmers.undo_depth(desk.session),
        depth.map(|depth| depth + 1),
        "one Undo group for the gesture"
    );

    // The output replays the reference recipe exactly on the compatible heads and fits the
    // lookalike best effort; the report carries the per-head status passively.
    desk.publish();
    let report = desk.report().await;
    let status = |fixture: FixtureId| {
        report
            .heads
            .iter()
            .find(|head| head.fixture_id == fixture.0)
            .and_then(|head| head.direct.clone())
            .unwrap_or_else(|| panic!("Direct status for {fixture:?}: {report:?}"))
    };
    for fixture in &desk.fixtures[..2] {
        let status = status(*fixture);
        assert_eq!(status.replay, ColorIntentDirectReplay::Exact, "{status:?}");
        assert_eq!(status.compatibility, None);
    }
    let lookalike = status(desk.fixtures[2]);
    assert_eq!(lookalike.replay, ColorIntentDirectReplay::Fallback);
    assert_eq!(
        lookalike.compatibility,
        Some(ColorIntentDirectCompatibility::DifferentSource),
        "identity decides, never names or slots"
    );
    let _ = std::fs::remove_dir_all(&desk.directory);
}

/// G5: an idle head (no Color programmed, so no Color output in the frame) outputs its profile
/// defaults. The pages show exactly those, and the first native edit adopts them once.
#[tokio::test]
async fn the_first_native_edit_on_an_idle_head_adopts_its_profile_defaults() {
    let desk = Desk::new().await;
    desk.publish();
    let defaults: Vec<u32> = path_channels(&desk.profile)
        .iter()
        .map(|channel| channel.default_raw)
        .collect();
    let shown: Vec<u32> = desk
        .pages("")
        .await
        .values
        .expect("an idle head shows its profile defaults")
        .controls
        .iter()
        .map(|value| value.raw)
        .collect();
    assert_eq!(shown, defaults);
    let applied = desk
        .apply(
            "idle",
            desk.native_edit(0, json!({"kind": "relative", "value": 5}), None),
        )
        .await;
    assert_eq!(applied["status"], "changed", "{applied}");
    assert!(applied.get("hold").is_none(), "{applied}");
    let mut expected = defaults;
    expected[0] += 5;
    for fixture in &desk.fixtures[..2] {
        assert_eq!(
            desk.recipe(*fixture),
            expected,
            "seeded once from the defaults"
        );
    }
    let _ = std::fs::remove_dir_all(&desk.directory);
}

#[tokio::test]
async fn native_edits_hold_quietly_with_a_gone_lease() {
    let desk = Desk::new().await;

    // A lease the session never received holds with the re-read reason; latest is not used.
    desk.program_semantic([1.0, 1.0, 1.0]);
    let held = desk
        .apply(
            "gone-lease",
            desk.native_edit(0, json!({"kind": "relative", "value": 5}), Some(999_999)),
        )
        .await;
    assert_eq!(held["hold"], "displayed_source_unavailable");
    // Without a lease (OSC, HTTP integrators) the latest accepted frame is adopted.
    let applied = desk
        .apply(
            "latest",
            desk.native_edit(0, json!({"kind": "relative", "value": 5}), None),
        )
        .await;
    assert_eq!(applied["status"], "changed", "{applied}");
    assert!(matches!(
        desk.value(desk.fixtures[2]),
        Some(AttributeValue::ColorProgram(program)) if matches!(program.as_ref(), ColorProgram::Direct { .. })
    ));
    let _ = std::fs::remove_dir_all(&desk.directory);
}
