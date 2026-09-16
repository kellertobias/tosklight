//! The Architect's selection reaches a running Visualizer.
//!
//! The CAD window writes its authoritative selection into the planning source
//! (`cad_replace_selection` → [`SceneSource::set_selection`]); the renderer's lighting-desk
//! provider hears `visualizer_selection_changed` and hands the new set to the frame build, which
//! outlines those fixtures and Venue objects in blue. This drives that whole path over a real
//! socket for a lamp and a generated 3D Venue object.

use crate::tests::{document, patch_one};
use crate::{SceneSource, router};
use light_core::FixtureId;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use uuid::Uuid;
use viz_scene::{ProviderEvent, Scene, SceneProvider};

fn truss_profile() -> light_fixture::FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/venue--two-point-truss.toskfixture");
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

/// Poll until the provider reports a selection equal to `expected`, or fail.
async fn selection_reaches(provider: &mut impl SceneProvider, expected: &HashSet<Uuid>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = None;
    while Instant::now() < deadline {
        for event in provider.poll() {
            if let ProviderEvent::Values(values) = event {
                last = Some(values.selected_fixtures.clone());
            }
        }
        if last.as_ref() == Some(expected) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the Visualizer shows {last:?}, the Architect selected {expected:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn architect_fixture_and_model_selection_follows_into_the_visualizer() {
    let (document, path) = document("cad-selection");
    let truss = FixtureId(Uuid::new_v4());
    patch_one(&document, &path, truss_profile(), truss, "Truss 1", None);
    let source = SceneSource::new(document);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let served = source.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router(served)).await;
    });
    let mut provider = viz_desk::DeskProvider::start(
        viz_desk::DeskConnection {
            host: "127.0.0.1".into(),
            port,
            retry: Duration::from_millis(50),
            ..viz_desk::DeskConnection::default()
        },
        Instant::now(),
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut scene: Option<Scene> = None;
    while Instant::now() < deadline && scene.is_none() {
        for event in provider.poll() {
            if let ProviderEvent::Snapshot { scene: read, .. } = event {
                scene = Some(*read);
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let scene = scene.expect("the Visualizer read the planning document");
    let lamp = scene
        .fixtures
        .iter()
        .find(|fixture| !fixture.drawn_as_scenery)
        .expect("the wash is a lamp")
        .fixture_id;
    let model = scene
        .fixtures
        .iter()
        .find(|fixture| fixture.drawn_as_scenery)
        .expect("the truss is a generated 3D Venue object");
    assert_eq!(model.fixture_id, truss.0);
    assert!(
        scene
            .scenery
            .iter()
            .any(|object| object.id == model.instance_id),
        "the Venue object is drawn by the scenery pass under its instance id"
    );

    // One lamp, then the lamp and the model together, then only the model, then nothing.
    let steps: [&[Uuid]; 4] = [&[lamp], &[lamp, truss.0], &[truss.0], &[]];
    for step in steps {
        source.set_selection(step.to_vec());
        let expected: HashSet<Uuid> = step.iter().copied().collect();
        selection_reaches(&mut provider, &expected).await;
    }
    // A selection never reaches into the rig: the scene is not rebuilt by it.
    assert!(
        !provider
            .poll()
            .iter()
            .any(|event| matches!(event, ProviderEvent::Snapshot { .. })),
        "selecting is presentation only and does not resynchronise the rig"
    );

    provider.shutdown();
    server.abort();
    let _ = std::fs::remove_file(path);
}
