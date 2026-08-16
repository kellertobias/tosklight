use super::draw::splash_state;
use super::*;

#[test]
fn demo_mode_resolves_the_same_portable_show_as_the_desk() {
    let resolved = canonical_demo_show_path().expect("canonical demo show");
    let desk = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/demo.show");
    assert_eq!(
        std::fs::read(resolved).expect("resolved demo bytes"),
        std::fs::read(desk).expect("Desk demo bytes")
    );
}

#[test]
fn media_workers_exist_only_for_the_normal_standalone_product() {
    let standalone = Application::new(Options::default());
    assert!(standalone.media_workers.is_some());

    let helper = Application::new(Options {
        helper: true,
        ..Options::default()
    });
    assert!(helper.media_workers.is_none());

    let embedded = Application::new(Options {
        embed: true,
        ..Options::default()
    });
    assert!(embedded.media_workers.is_none());
}

#[test]
fn renderer_adopts_settings_delivered_by_the_connected_editor() {
    struct SettingsProvider(Option<viz_scene::RendererSettingsUpdate>);
    impl viz_scene::SceneProvider for SettingsProvider {
        fn capabilities(&self) -> viz_scene::ProviderCapabilities {
            viz_scene::ProviderCapabilities {
                kind: ProviderKind::PlanningSoftware,
                available: true,
                unavailable_reason: None,
                default_host: "127.0.0.1".into(),
                default_port: 5310,
                uses_network_input: true,
            }
        }
        fn poll(&mut self) -> Vec<viz_scene::ProviderEvent> {
            self.0
                .take()
                .map(viz_scene::ProviderEvent::RendererSettings)
                .into_iter()
                .collect()
        }
        fn request_resync(&mut self) {}
        fn shutdown(&mut self) {}
    }

    let mut application = Application::new(Options::default());
    let settings = viz_scene::RendererSettings {
        quality: Some("draft".into()),
        fog: 0.02,
        ..application.preferences.renderer_settings()
    };
    let mut session = Session::new(
        Box::new(SettingsProvider(Some(viz_scene::RendererSettingsUpdate {
            revision: 2,
            source: "editor".into(),
            changed: vec!["quality".into(), "fog".into()],
            settings,
        }))),
        ProviderKind::PlanningSoftware,
        Instant::now(),
    );
    session.pump(Instant::now());
    application.adopt_connected_renderer_settings(&mut session);

    assert_eq!(
        application.preferences.quality_override,
        Some(viz_scene::RenderQuality::Draft)
    );
    assert_eq!(application.preferences.atmosphere.amount, 0.02);
}

/// An empty window has to say which kind of empty it is, and a window with a rig in it must
/// never be covered by a splash — the picture is the point.
#[test]
fn the_splash_only_stands_in_for_a_picture_that_does_not_exist() {
    let endpoint = || "http://127.0.0.1:5310".to_owned();
    assert_eq!(
        splash_state(
            &ConnectionState::WaitingForShow {
                endpoint: endpoint()
            },
            true
        ),
        Some(ui::SplashState::NoShow)
    );
    assert!(matches!(
        splash_state(
            &ConnectionState::LoadingScene {
                endpoint: endpoint()
            },
            true
        ),
        Some(ui::SplashState::Loading(_))
    ));
    assert!(matches!(
        splash_state(
            &ConnectionState::Failed {
                boundary: "server readiness".into(),
                detail: "refused".into(),
            },
            true
        ),
        Some(ui::SplashState::Failed(_))
    ));
    // A show that is loaded and simply has no fixtures is the status bar's business.
    assert_eq!(
        splash_state(
            &ConnectionState::Connected {
                endpoint: endpoint(),
                revision: 2
            },
            true
        ),
        None
    );
    // And a rig on stage is never covered, whatever the connection is doing.
    for state in [
        ConnectionState::LoadingScene {
            endpoint: endpoint(),
        },
        ConnectionState::Failed {
            boundary: "desk".into(),
            detail: "gone".into(),
        },
        ConnectionState::WaitingForShow {
            endpoint: endpoint(),
        },
    ] {
        assert_eq!(splash_state(&state, false), None);
    }
}

/// What a right-button drag is for: standing where the camera stands and looking around.
#[test]
fn a_right_drag_turns_the_camera_in_the_perspective_views() {
    for mode in [ViewMode::Full3d, ViewMode::Simple3d, ViewMode::Lines3d] {
        assert!(!Application::right_drag_pans(mode, ModifiersState::empty()));
    }
}

/// A plan view has no heading to turn, so the same drag keeps moving the picture there.
#[test]
fn a_right_drag_pans_the_plan_views_and_any_view_with_shift() {
    for mode in [
        ViewMode::TopDown,
        ViewMode::FrontToBack,
        ViewMode::BackToFront,
        ViewMode::LeftToRight,
        ViewMode::RightToLeft,
    ] {
        assert!(Application::right_drag_pans(mode, ModifiersState::empty()));
    }
    assert!(Application::right_drag_pans(
        ViewMode::Full3d,
        ModifiersState::SHIFT
    ));
}

/// The drag has to turn the camera by an amount a hand can produce: a window-wide drag is useful,
/// while still leaving ample travel for precise framing.
#[test]
fn the_turn_a_full_drag_makes_is_usable() {
    let across_a_window = 1400.0 * LOOK_RADIANS_PER_UNIT;
    assert!(across_a_window > 1.0, "{across_a_window} is too slow");
    assert!(
        across_a_window < std::f32::consts::FRAC_PI_2,
        "{across_a_window} is too fast"
    );
}

#[test]
fn a_small_pointer_motion_stays_precise() {
    let turn = 4.0 * LOOK_RADIANS_PER_UNIT;
    assert!(turn > 0.0);
    assert!(turn <= 0.005, "four hand points turned {turn} radians");
}

#[test]
fn device_motion_is_normalized_and_camera_pans_are_damped() {
    let scale = 2.0;
    let hand = hand_points(20.0, scale);
    assert_eq!(hand, 10.0, "Retina pixels must become hand points");
    assert_eq!(
        pan_pixels(hand, scale),
        10.0,
        "pan follows half the hand travel"
    );
}

#[test]
fn dmx_camera_targets_only_the_dedicated_external_perspective_view() {
    for mode in [ViewMode::Full3d, ViewMode::Simple3d, ViewMode::Lines3d] {
        assert!(is_external_camera_target(false, mode));
        assert!(!is_external_camera_target(true, mode));
    }
    for mode in [
        ViewMode::TopDown,
        ViewMode::FrontToBack,
        ViewMode::BackToFront,
        ViewMode::LeftToRight,
        ViewMode::RightToLeft,
    ] {
        assert!(!is_external_camera_target(false, mode));
    }
}

fn dmx_camera(x: f32) -> Camera {
    Camera {
        position: [x, 2.0, 3.0].into(),
        target: [x, 2.0, 2.0].into(),
        ..Camera::default()
    }
}

#[test]
fn local_camera_latches_until_live_dmx_is_explicitly_released() {
    let mut ownership = ExternalCameraOwnership::default();
    let first = dmx_camera(1.0);
    assert_eq!(ownership.observe(Some((first, false))), Some(first));
    ownership.latch_local();

    let moved = dmx_camera(4.0);
    assert_eq!(ownership.observe(Some((moved, false))), None);
    assert_eq!(
        ownership.status(),
        ui::DmxCameraControlStatus::Local { can_release: true }
    );
    assert_eq!(ownership.release_to_dmx(), Some(moved));
    assert_eq!(ownership.status(), ui::DmxCameraControlStatus::Dmx);
}

#[test]
fn stale_or_absent_dmx_holds_the_last_pose_and_cannot_fake_a_release() {
    let mut ownership = ExternalCameraOwnership::default();
    let last = dmx_camera(8.0);
    ownership.observe(Some((last, false)));
    assert_eq!(ownership.observe(Some((last, true))), Some(last));
    assert_eq!(ownership.status(), ui::DmxCameraControlStatus::Held);

    ownership.latch_local();
    assert_eq!(ownership.observe(None), None);
    assert_eq!(ownership.release_to_dmx(), None);
    assert_eq!(
        ownership.status(),
        ui::DmxCameraControlStatus::Local { can_release: false }
    );
}
