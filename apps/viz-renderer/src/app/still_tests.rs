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
fn still_controls_show_native_progress_cancel_and_saved_actions() {
    use crate::stills::{Engine, Phase, Status, StillJobId};
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
        engine: Some(Engine::Ultra),
        notice: None,
        phase: Phase::Rendering {
            detail: "Rendering native Ultra image".into(),
        },
    };
    ui.observe(Some(status.clone()));
    let mut overlay = viz_render::Overlay::default();
    ui.draw(&mut overlay, 1280.0, 720.0);
    assert!(!overlay.is_empty());
    assert!(
        ui.content()
            .0
            .join(" ")
            .contains("Rendering native Ultra image")
    );
    assert_eq!(ui.content().1, vec![("Cancel", Some(ui::Action::Cancel))]);
    assert!(ui.visible());
    status.phase = Phase::Saved {
        path: std::path::PathBuf::from("/saved/image.png"),
    };
    ui.observe(Some(status.clone()));
    let saved = ui.content().0.join(" ");
    assert!(saved.contains("3840 × 2160 image saved"));
    assert!(saved.contains("Ultra · ToskLight"));
    assert!(saved.contains("16:9 framing"));
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
        "unchanged status cannot reopen a dismissed result"
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
