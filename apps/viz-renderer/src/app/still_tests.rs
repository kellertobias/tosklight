use super::*;

#[test]
fn exact_4k_shortcut_rejects_repeat_ime_text_modal_and_extra_modifiers() {
    let command = if cfg!(target_os = "macos") {
        ModifiersState::SUPER
    } else {
        ModifiersState::CONTROL
    };
    let chord = command | ModifiersState::SHIFT;
    let key = Key::Character("R".into());
    assert!(shortcut_allowed(&key, chord, false, false, false));
    assert!(shortcut_allowed(
        &Key::Character("r".into()),
        chord,
        false,
        false,
        false
    ));
    assert!(!shortcut_allowed(&key, chord, true, false, false));
    assert!(!shortcut_allowed(&key, chord, false, true, false));
    assert!(!shortcut_allowed(&key, chord, false, false, true));
    for modifiers in [
        command,
        ModifiersState::SHIFT,
        chord | ModifiersState::ALT,
        ModifiersState::SUPER | ModifiersState::CONTROL | ModifiersState::SHIFT,
    ] {
        assert!(!shortcut(&key, modifiers));
    }
    assert!(
        !shortcut(&Key::Character("s".into()), chord),
        "snapshot chord is independent"
    );
}

#[test]
fn still_controls_show_both_engines_reasons_cancel_and_saved_actions() {
    use crate::stills::{Phase, Status, StillJobId};
    let mut ui = StillUi::default();
    let mut status = Status {
        job_id: StillJobId("test-1".into()),
        request_id: None,
        pane_id: "architect".into(),
        follow_preload: false,
        scene_revision: 1,
        values_frame: 2,
        time_seconds: 3.0,
        width: 3840,
        height: 2160,
        framing: "16:9 framing".into(),
        engine: None,
        notice: None,
        phase: Phase::NeedsChoice {
            blender_version: "4.5".into(),
            ultimate_reasons: vec!["Live laser state requires Ultra".into()],
            ultra_explanation: "Ultra uses ToskLight".into(),
            ultimate_explanation: "Ultimate uses Blender/Cycles".into(),
        },
    };
    ui.observe(Some(status.clone()));
    assert!(ui.modal());
    let mut overlay = viz_render::Overlay::default();
    ui.draw(&mut overlay, 1280.0, 720.0);
    assert!(!overlay.is_empty());
    // The panel's disabled Ultimate button cannot dispatch a job choice.
    assert!(ui.action_at(640.0, 584.0).is_none());
    assert!(matches!(
        ui.action_at(300.0, 584.0),
        Some(ui::Action::Ultra)
    ));
    assert!(matches!(
        ui.action_at(950.0, 584.0),
        Some(ui::Action::Cancel)
    ));
    let explanation = ui.content().0.join(" ");
    assert!(explanation.contains("Ultra uses ToskLight"));
    assert!(explanation.contains("Ultimate uses Blender/Cycles"));
    assert!(explanation.contains("Live laser state requires Ultra"));
    assert!(ui.visible());
    ui.fail("Temporary choice error".into());
    ui.draw(&mut overlay, 1280.0, 720.0);
    assert!(
        ui.modal(),
        "errors do not dismiss the frozen chooser or its cancel controls"
    );
    status.phase = Phase::Saved {
        path: std::path::PathBuf::from("/saved/image.png"),
    };
    status.engine = Some(Engine::Ultimate);
    ui.reset();
    ui.observe(Some(status.clone()));
    let saved = ui.content().0.join(" ");
    assert!(saved.contains("3840 × 2160 image saved"));
    assert!(saved.contains("Ultimate · Blender"));
    assert!(saved.contains("16:9 framing"));
    assert!(!ui.modal());
    ui.draw(&mut overlay, 1280.0, 720.0);
    assert!(matches!(ui.action_at(300.0, 584.0), Some(ui::Action::Open)));
    assert!(matches!(
        ui.action_at(640.0, 584.0),
        Some(ui::Action::Reveal)
    ));
    ui.close();
    assert!(!ui.visible());
    ui.observe(Some(status));
    assert!(
        !ui.visible(),
        "unchanged status polling cannot reopen a dismissed result"
    );
}

#[test]
fn invocation_resolves_latest_local_camera_without_changing_cached_view() {
    let mut resolved = ViewConfiguration::default();
    resolved.mode = viz_scene::ViewMode::Lines3d;
    resolved.quality = viz_scene::RenderQuality::Draft;
    let mut camera = viz_render::CameraControl::default();
    camera.adopt(&resolved.camera);
    camera.zoom(0.5);
    let latest = invocation_view(&resolved, &camera, true, resolved.detailed_people);
    assert_ne!(latest.camera.position, resolved.camera.position);
    assert_eq!(latest.mode, resolved.mode);
    assert_eq!(latest.quality, resolved.quality);
    assert_eq!(
        invocation_view(&resolved, &camera, false, resolved.detailed_people)
            .camera
            .position,
        resolved.camera.position
    );
}

#[test]
fn modal_entry_and_exit_release_held_live_camera_input() {
    let mut app = Application::new(crate::settings::Options {
        embed: true,
        ..Default::default()
    });
    let status = crate::stills::Status {
        job_id: crate::stills::StillJobId("modal-job".into()),
        request_id: None,
        pane_id: "architect".into(),
        follow_preload: false,
        scene_revision: 1,
        values_frame: 2,
        time_seconds: 3.0,
        width: 3840,
        height: 2160,
        framing: "16:9".into(),
        engine: None,
        notice: None,
        phase: Phase::NeedsChoice {
            blender_version: "4.5".into(),
            ultimate_reasons: vec![],
            ultra_explanation: "Ultra".into(),
            ultimate_explanation: "Ultimate".into(),
        },
    };
    app.turning = true;
    app.panning_camera_plane = true;
    app.drag_moved_by_device = true;
    app.walk_keys.forward = true;
    app.apply_still_status(Some(status.clone()));
    assert!(app.still_ui.modal());
    assert!(
        !app.turning
            && !app.panning_camera_plane
            && !app.drag_moved_by_device
            && !app.walk_keys.forward
    );
    // A release swallowed while modal must not resurrect any pre-modal held input afterwards.
    app.turning = true;
    app.walk_keys.back = true;
    let mut rendering = status;
    rendering.phase = Phase::Rendering {
        detail: "Rendering frozen image".into(),
    };
    app.apply_still_status(Some(rendering));
    assert!(!app.still_ui.modal());
    assert!(!app.turning && !app.walk_keys.back);
}

#[test]
fn detailed_people_invocation_uses_current_preference_without_another_draw() {
    for (old, current) in [(false, true), (true, false)] {
        let resolved = ViewConfiguration {
            quality: viz_scene::RenderQuality::Draft,
            detailed_people: old,
            ..Default::default()
        };
        let camera = viz_render::CameraControl::default();
        let frozen_view = invocation_view(&resolved, &camera, false, current);
        assert_eq!(frozen_view.detailed_people, current);
        assert_eq!(resolved.detailed_people, old);
        assert_eq!(frozen_view.quality, resolved.quality);
        assert_eq!(frozen_view.camera, resolved.camera);
    }
}
