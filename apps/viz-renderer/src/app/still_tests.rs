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

/// Same ordering as draw -> about_to_wait provider poll -> native shortcut. The
/// provider deliberately supplies input values without the final render effects.
fn live_session() -> (
    crate::session::Session,
    std::sync::mpsc::Sender<viz_scene::ProviderEvent>,
) {
    struct Provider(std::sync::mpsc::Receiver<viz_scene::ProviderEvent>);
    impl viz_scene::SceneProvider for Provider {
        fn capabilities(&self) -> viz_scene::ProviderCapabilities {
            viz_scene::ProviderCapabilities {
                kind: viz_scene::ProviderKind::LightingDesk,
                available: true,
                unavailable_reason: None,
                default_host: "127.0.0.1".into(),
                default_port: 5000,
                uses_network_input: true,
            }
        }
        fn poll(&mut self) -> Vec<viz_scene::ProviderEvent> {
            self.0.try_iter().collect()
        }
        fn request_resync(&mut self) {}
        fn shutdown(&mut self) {}
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut session = crate::session::Session::new(
        Box::new(Provider(rx)),
        viz_scene::ProviderKind::LightingDesk,
        std::time::Instant::now(),
    );
    tx.send(viz_scene::ProviderEvent::Snapshot {
        scene: Box::new(viz_scene::Scene::default()),
        view: Some(ViewConfiguration::default()),
    })
    .unwrap();
    session.pump(std::time::Instant::now());
    (session, tx)
}

#[test]
fn live_44hz_input_between_draw_and_shortcut_freezes_only_final_resolved_values() {
    let (mut session, tx) = live_session();
    let now = std::time::Instant::now();
    let mut values = viz_scene::SceneValues {
        frame: 44,
        ..Default::default()
    };
    values.resize(1);
    values.emitters[0].intensity = 0.37; // final VM/physics output, absent from provider
    let resolved = ResolvedFrame::new(&session, &values, session.source_view, 1.25);
    tx.send(viz_scene::ProviderEvent::Values(Box::new(
        viz_scene::SceneValues {
            frame: 45,
            ..Default::default()
        },
    )))
    .unwrap();
    session.pump(now + std::time::Duration::from_secs_f64(1.0 / 44.0));
    assert_ne!(
        resolved.values.frame, session.values.frame,
        "old export guard refused here"
    );
    let frozen = resolved
        .freeze(&session, &resolved.view, Default::default(), 0.5)
        .unwrap();
    assert_eq!(frozen.values().frame, 44);
    assert_eq!(frozen.values().emitters[0].intensity, 0.37);
    assert_eq!(frozen.time_seconds(), 1.25);
    assert_eq!(frozen.crowd_amount(), 0.5);
    assert_eq!(session.values.frame, 45);
}

#[test]
fn pending_show_resync_and_same_mode_view_changes_refuse_old_freezes() {
    for change in 0..6 {
        let (mut session, tx) = live_session();
        let resolved = ResolvedFrame::new(&session, &session.values, session.source_view, 1.0);
        let event = match change {
            0 => viz_scene::ProviderEvent::Snapshot {
                scene: Box::new(session.scene.clone()),
                view: None,
            },
            1 => viz_scene::ProviderEvent::SceneDelta(Box::new(session.scene.clone())),
            2 => viz_scene::ProviderEvent::View(ViewConfiguration {
                exposure: 0.2,
                ..session.source_view
            }),
            3 => viz_scene::ProviderEvent::Connection(viz_scene::ConnectionState::LoadingScene {
                endpoint: "new show".into(),
            }),
            4 => viz_scene::ProviderEvent::Connection(viz_scene::ConnectionState::WaitingForShow {
                endpoint: "new show".into(),
            }),
            _ => viz_scene::ProviderEvent::ResyncRequired {
                reason: "new source".into(),
            },
        };
        tx.send(event).unwrap();
        session.pump(std::time::Instant::now());
        assert!(
            resolved
                .freeze(&session, &resolved.view, Default::default(), 1.0)
                .is_err()
        );
    }
}

#[test]
fn local_view_override_needs_one_draw_and_then_preserves_latest_camera() {
    let (mut session, _) = live_session();
    let old = ResolvedFrame::new(&session, &session.values, session.source_view, 1.0);
    session.source_view.mode = viz_scene::ViewMode::Lines3d;
    assert!(old.validate(&session).is_err());
    let resolved = ResolvedFrame::new(&session, &session.values, session.source_view, 2.0);
    let mut camera = viz_render::CameraControl::default();
    camera.adopt(&resolved.view.camera);
    camera.zoom(0.5);
    let view = invocation_view(&resolved.view, &camera, true, true);
    let frozen = resolved
        .freeze(&session, &view, Default::default(), 1.0)
        .unwrap();
    assert_eq!(frozen.view().mode, viz_scene::ViewMode::Lines3d);
    assert_ne!(frozen.view().camera, resolved.view.camera);
    assert!(frozen.view().detailed_people);
    assert_eq!(session.source_view, resolved.source_view);
}

#[test]
#[ignore = "serialized native GPU: production draw submission, key route and real 4K publication"]
fn native_draw_then_44hz_pump_then_shortcut_publishes_resolved_4k() {
    use crate::settings::Options;
    use std::time::{Duration, Instant};
    let (mut session, tx) = live_session();
    let mut demo = crate::demo::DemoProvider::new();
    for event in viz_scene::SceneProvider::poll(&mut demo) {
        tx.send(event).unwrap();
    }
    session.pump(Instant::now());
    let mut app = Application::new(Options {
        helper: true,
        ..Options::default()
    });
    app.renderer = Some(viz_render::Renderer::headless(640, 360).unwrap());
    app.camera.adopt(&session.source_view.camera);
    app.modifiers = ModifiersState::SHIFT
        | if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        };
    for local in [false, true] {
        if local {
            session.source_view.mode = viz_scene::ViewMode::Lines3d;
        }
        let view = session.effective_view(&app.preferences);
        let mut values = session.values.clone();
        values.frame = 440;
        values.emitters[0].intensity = 0.37;
        let renderer = app.renderer.as_mut().unwrap();
        let target = renderer.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("native key-route live frame"),
            size: wgpu::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.output_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let result = Some(renderer.render_into(
            &target.create_view(&Default::default()),
            &session.scene,
            &values,
            &view,
            &Default::default(),
            10.0,
        ));
        result
            .as_ref()
            .unwrap()
            .as_ref()
            .expect("production live frame submission");
        app.remember_still_frame(&session, &values, view, 10.0, &result);
        tx.send(viz_scene::ProviderEvent::Values(Box::new(
            viz_scene::SceneValues {
                frame: 441,
                ..Default::default()
            },
        )))
        .unwrap();
        session.pump(Instant::now() + Duration::from_secs_f64(1.0 / 44.0));
        app.session = Some(session);
        assert!(app.still_shortcut(
            &Key::Character("R".into()),
            winit::event::ElementState::Pressed,
            false
        ));
        let first = app
            .still_jobs
            .status()
            .expect("native key must start an independent job");
        assert_eq!(first.values_frame, 440);
        assert_eq!(first.time_seconds, 10.0);
        let deadline = Instant::now() + Duration::from_secs(120);
        let saved = loop {
            let status = app.still_jobs.status().unwrap();
            if status.phase.is_terminal() {
                break status;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        };
        let crate::stills::Phase::Saved { path } = saved.phase else {
            panic!("{:?}", saved.phase);
        };
        assert_eq!(image::image_dimensions(&path).unwrap(), (3840, 2160));
        println!("native key-route 4K local={local}: {}", path.display());
        assert_eq!(app.session.as_ref().unwrap().values.frame, 441);
        assert_eq!(app.session.as_ref().unwrap().source_view.mode, view.mode);
        assert_eq!(
            app.session
                .as_ref()
                .unwrap()
                .effective_view(&app.preferences)
                .quality,
            view.quality
        );
        while app.still_jobs.is_busy() {
            std::thread::sleep(Duration::from_millis(2));
        }
        session = app.session.take().unwrap();
    }
}
