//! Proof that a planning document is a scene source the visualizer can already read.
//!
//! Every response is decoded into `viz_desk::wire` — the exact types the renderer's lighting-desk
//! provider uses — so a drift between what this serves and what the renderer reads is a
//! compilation or decode failure here, not a blank window at a load-in.

use crate::{SceneSource, router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
use light_core::FixtureId;
use light_fixture::{
    FixtureLocation, FixtureProfile, FixtureVector, GelAssignment, InstalledLightSource,
    PatchedFixturePatch, PatchedFixtureProfileReference, SplitPatch,
};
use light_show::{FixtureProfileRevision, ShowStore};
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, path::PathBuf};
use tower::ServiceExt;
use uuid::Uuid;
use viz_document::PlanningDocument;

fn temp_path(name: &str) -> PathBuf {
    let base = std::env::var_os("LIGHT_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&base);
    base.join(format!("viz-planning-{name}-{}.show", Uuid::new_v4()))
}

/// A document with one patched fixture at 1.1, three metres up.
fn document(name: &str) -> (PlanningDocument, PathBuf) {
    let path = temp_path(name);
    let document = PlanningDocument::create(&path, "Planning show").expect("create");
    let mut profile = FixtureProfile::blank();
    profile.revision = 2;
    profile.manufacturer = "Acme".into();
    profile.name = "Planning Wash".into();
    profile.short_name = "Wash".into();
    profile.fixture_type = "wash".into();
    let profile_id = profile.id;
    let profile_revision = u64::from(profile.revision);
    let mode_id = profile.modes[0].id;
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    ShowStore::open(&path)
        .unwrap()
        .insert_fixture_profile_revision(&stored)
        .expect("retain profile");

    document
        .patch_fixtures(PatchFixturesCommand {
            show_id: document.show_id(),
            fixtures: vec![PatchFixtureCandidate {
                profile: PatchedFixtureProfileReference {
                    profile_id,
                    profile_revision,
                    mode_id,
                },
                patch: PatchedFixturePatch {
                    fixture_id: FixtureId(Uuid::new_v4()),
                    fixture_number: Some(7),
                    virtual_fixture_number: None,
                    name: "Wash 7".into(),
                    universe: Some(1),
                    address: Some(1),
                    split_patches: vec![SplitPatch {
                        split: 1,
                        universe: Some(1),
                        address: Some(1),
                    }],
                    layer_id: "default".into(),
                    direct_control: None,
                    location: FixtureLocation {
                        x: 1_000,
                        y: 2_000,
                        z: 3_000,
                    },
                    rotation: FixtureVector {
                        x: 0.0,
                        y: 45.0,
                        z: 0.0,
                    },
                    logical_heads: Vec::new(),
                    multipatch: Vec::new(),
                    group_masters_enabled: true,
                    grand_master_enabled: true,
                    invert_pan: false,
                    invert_tilt: true,
                    bracket_angle: 0.0,
                    shaper_angle: None,
                    installed_appearance: Default::default(),
                    move_in_black_enabled: true,
                    move_in_black_delay_millis: 0,
                    highlight_overrides: BTreeMap::new(),
                },
            }],
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: Vec::new(),
        })
        .expect("patch one fixture");
    (document, path)
}

async fn get<T: DeserializeOwned>(source: &SceneSource, path: &str) -> T {
    let response = router(source.clone())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .expect("route");
    assert_eq!(response.status(), StatusCode::OK, "GET {path}");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).expect("decodes into the renderer's own type")
}

#[tokio::test]
async fn the_renderer_reads_a_document_as_it_reads_a_desk() {
    let (document, path) = document("patch");
    let source = SceneSource::new(document);

    let readiness: viz_desk::wire::Readiness = get(&source, "/api/v2/readiness").await;
    assert_eq!(readiness.status, "ready");
    assert!(readiness.active_show.is_some());

    let patch: viz_desk::wire::PatchSnapshot = get(&source, "/api/v2/patch").await;
    assert_eq!(patch.fixtures.len(), 1);
    let fixture = &patch.fixtures[0];
    assert_eq!(fixture.fixture_number, Some(7));
    assert_eq!(fixture.split_patches[0].universe, Some(1));
    assert_eq!(fixture.split_patches[0].address, Some(1));
    assert_eq!(fixture.location.z, 3_000, "millimetres, not metres");
    assert!((fixture.rotation.y - 45.0).abs() < f32::EPSILON);
    assert!(fixture.invert_tilt);

    // Without this the renderer cannot decode DMX and every fixture stays dark.
    assert_eq!(patch.profile_revisions.len(), 1);
    assert!(
        patch.profile_revisions[0].profile_snapshot.is_object(),
        "the renderer decodes DMX from this; without it every fixture stays dark"
    );

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn a_session_can_be_opened_and_closed() {
    let (document, path) = document("session");
    let source = SceneSource::new(document);
    let response = router(source.clone())
        .oneshot(
            Request::post("/api/v2/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let session: viz_desk::wire::SessionResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        session.role.as_deref(),
        Some("read_only"),
        "a planning source promises read-only, so the renderer raises no warning"
    );

    let closed = router(source)
        .oneshot(
            Request::delete(format!("/api/v2/sessions/{}", session.session_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(closed.status(), StatusCode::NO_CONTENT);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn show_objects_are_served_for_the_kinds_the_renderer_reads() {
    let (document, path) = document("objects");
    let source = SceneSource::new(document);
    for kind in ["route", "stage_layout", "venue"] {
        let collection: viz_desk::wire::ObjectCollection =
            get(&source, &format!("/api/v2/objects/{kind}")).await;
        // A new document configures none of these; the renderer treats that as "no routes" and
        // listens on the Art-Net and sACN defaults instead of failing.
        assert!(collection.objects.is_empty(), "{kind}");
    }
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn a_source_with_no_document_yet_reports_itself_starting() {
    let source = SceneSource::default();
    let readiness: viz_desk::wire::Readiness = get(&source, "/api/v2/readiness").await;
    assert_eq!(
        readiness.status, "starting",
        "the editor may still be waiting for the operator to choose a file"
    );
    assert!(readiness.active_show.is_none());

    let response = router(source)
        .oneshot(Request::get("/api/v2/patch").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn opening_another_show_replaces_what_the_renderer_sees() {
    let (first, first_path) = document("first");
    let (second, second_path) = document("second");
    let second_id = second.show_id();
    let source = SceneSource::new(first);

    source.open(second);
    let readiness: viz_desk::wire::Readiness = get(&source, "/api/v2/readiness").await;
    assert_eq!(readiness.active_show, Some(second_id.0));

    let _ = std::fs::remove_file(&first_path);
    let _ = std::fs::remove_file(&second_path);
}

/// The whole path, with the renderer's own provider on the other end.
///
/// Everything above is a shape check; this is the claim: start the server on a real socket, point
/// the visualizer's lighting-desk provider at it, and get a scene with the patched fixture in it.
#[tokio::test(flavor = "multi_thread")]
async fn the_visualizer_provider_builds_a_scene_from_a_planning_document() {
    use std::time::{Duration, Instant};
    use viz_scene::{ProviderEvent, SceneProvider};

    let (document, path) = document("end-to-end");
    let source = SceneSource::new(document);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router(source)).await;
    });

    let mut provider = viz_desk::DeskProvider::start(
        viz_desk::DeskConnection {
            host: "127.0.0.1".to_owned(),
            port,
            ..viz_desk::DeskConnection::default()
        },
        Instant::now(),
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut fixtures = None;
    while Instant::now() < deadline && fixtures.is_none() {
        for event in provider.poll() {
            if let ProviderEvent::Snapshot { scene, .. } = event {
                fixtures = Some(scene.fixtures.len());
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    provider.shutdown();
    server.abort();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        fixtures,
        Some(1),
        "the visualizer connected to a planning document and drew the rig in it"
    );
}

/// The reason this server has an event stream at all.
///
/// Without one the renderer's provider finishes reading, finds nothing to wait on, and reconnects
/// on its retry interval for as long as the window is open — rebuilding the scene and rebinding
/// every DMX receiver each time. Connected, it must sit still; told the document changed, it must
/// resynchronise at once rather than on some later reconnection.
#[tokio::test(flavor = "multi_thread")]
async fn a_connected_renderer_sits_still_until_the_document_changes() {
    use std::time::{Duration, Instant};
    use viz_scene::SceneProvider;

    let (document, path) = document("event-stream");
    let source = SceneSource::new(document);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let served = source.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router(served)).await;
    });

    let mut provider = viz_desk::DeskProvider::start(
        viz_desk::DeskConnection {
            host: "127.0.0.1".to_owned(),
            port,
            // The interval the provider would otherwise reconnect on.
            retry: Duration::from_millis(200),
            ..viz_desk::DeskConnection::default()
        },
        Instant::now(),
    );

    let mut snapshots = 0_u32;
    let first = Instant::now() + Duration::from_secs(20);
    while Instant::now() < first && snapshots == 0 {
        snapshots += count_snapshots(&mut provider);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(snapshots, 1, "the renderer read the document once");

    // Nothing has changed, so nothing more may arrive — however many retry intervals pass.
    let quiet = Instant::now() + Duration::from_secs(2);
    let mut extra = 0;
    while Instant::now() < quiet {
        extra += count_snapshots(&mut provider);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        extra, 0,
        "an idle planning window must not make the renderer reconnect and rebind its receivers"
    );

    // A patch committed in the window is news, and the renderer has to see it now.
    source.mark_changed();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut after_change = 0;
    while Instant::now() < deadline && after_change == 0 {
        after_change += count_snapshots(&mut provider);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    provider.shutdown();
    server.abort();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        after_change, 1,
        "an edit in the planning window resynchronises the renderer"
    );
}

/// Anything that puts a new rig on the screen: the first read is a snapshot, and an edit
/// afterwards arrives as a delta applied to it rather than as a second whole scene.
fn count_snapshots(provider: &mut impl viz_scene::SceneProvider) -> u32 {
    provider
        .poll()
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                viz_scene::ProviderEvent::Snapshot { .. } | viz_scene::ProviderEvent::SceneDelta(_)
            )
        })
        .count() as u32
}

/// The mechanical angles reach the picture.
///
/// A bracket angle and a fitted shaper module are patch-owned facts nothing on the desk can drive.
/// If they stopped at the API the rig would be drawn hanging straight with its blades square,
/// which is exactly the rig nobody has.
#[tokio::test]
async fn the_renderer_reads_the_bracket_and_shaper_angles() {
    let (document, path) = document("mechanical-angles");
    let snapshot = document.patch_snapshot().expect("snapshot");
    let mut fixture = snapshot.fixtures[0].clone();
    fixture.patch.bracket_angle = -35.0;
    fixture.patch.shaper_angle = Some(22.5);
    fixture.patch.installed_appearance.light_source = InstalledLightSource::Tungsten;
    fixture.patch.installed_appearance.color_temperature_kelvin = Some(3_200);
    fixture.patch.installed_appearance.gel = GelAssignment::Custom {
        name: "Warm red".into(),
        color_srgb: "#C01020".into(),
        note: None,
    };
    fixture.patch.installed_appearance.shaper_angles_degrees = [10.0, 20.0, 30.0, 40.0];
    let expected_appearance = fixture.patch.installed_appearance.clone();
    document
        .patch_fixtures(PatchFixturesCommand {
            show_id: document.show_id(),
            fixtures: vec![PatchFixtureCandidate {
                profile: fixture.profile,
                patch: fixture.patch,
            }],
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: Vec::new(),
        })
        .expect("set the angles");

    let source = SceneSource::new(document);
    let patch: viz_desk::wire::PatchSnapshot = get(&source, "/api/v2/patch").await;
    assert_eq!(patch.fixtures[0].bracket_angle, -35.0);
    assert_eq!(patch.fixtures[0].shaper_angle, Some(22.5));
    assert_eq!(patch.fixtures[0].installed_appearance, expected_appearance);

    let _ = std::fs::remove_file(&path);
}

/// What a desk receives when it loads from this editor: an ordinary show file, with the rig in
/// it, that opens on a machine that has never seen this document.
#[tokio::test]
async fn the_open_document_downloads_as_a_show_a_desk_can_open() {
    let (document, path) = document("download");
    let source = SceneSource::new(document);
    let response = router(source.clone())
        .oneshot(
            Request::get("/api/v2/document/download")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("route");
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response
        .headers()
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        disposition.contains("Planning show.show"),
        "the copy is named after the document: {disposition}"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();

    let copy = temp_path("downloaded");
    std::fs::write(&copy, &body).expect("write the copy");
    let opened = PlanningDocument::open(&copy).expect("the copy opens as a show");
    assert_eq!(opened.name().expect("name"), "Planning show");
    assert_eq!(
        opened.patch_snapshot().expect("patch").fixtures.len(),
        1,
        "the rig travels with the copy"
    );

    // A copy, and only a copy: editing here does not reach the document it came from.
    opened.rename("Renamed on the desk").expect("rename");
    let original: viz_desk::wire::PatchSnapshot = get(&source, "/api/v2/patch").await;
    assert_eq!(original.fixtures.len(), 1);
    assert_eq!(
        source.with(|document| document.name().unwrap()).unwrap(),
        "Planning show"
    );

    let _ = std::fs::remove_file(&copy);
    let _ = std::fs::remove_file(&path);
}

/// An editor with nothing open says so, rather than sending an empty file a desk would import.
#[tokio::test]
async fn an_editor_with_no_document_has_nothing_to_download() {
    let response = router(SceneSource::default())
        .oneshot(
            Request::get("/api/v2/document/download")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("route");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// The same profile, with `transform` on its colour channels, so a subtractive fixture can be
/// tested against an additive one that is otherwise identical.
fn preview_profile_with(transform: light_fixture::CanonicalTransform) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Acme".into();
    profile.name = "Preview Wash".into();
    profile.short_name = "Preview".into();
    profile.fixture_type = "wash".into();
    let head_id = profile.modes[0].heads[0].id;
    let channel =
        |attribute: &str, resolution: light_fixture::ChannelResolution, secondary: Vec<u16>| {
            light_fixture::FixtureChannel {
                id: Uuid::new_v4(),
                head_id,
                split: 1,
                fixture_attribute: light_core::AttributeKey(attribute.to_owned()),
                attribute: light_core::AttributeKey(attribute.to_owned()),
                canonical_transform: if attribute.starts_with("color.") {
                    transform
                } else {
                    light_fixture::CanonicalTransform::Identity
                },
                resolution,
                secondary_slots: secondary,
                default_raw: 0,
                highlight_raw: 0,
                physical_min: None,
                physical_max: None,
                unit: None,
                invert: false,
                snap: false,
                reacts_to_virtual_intensity: false,
                reacts_to_sequence_master: false,
                reacts_to_group_master: false,
                reacts_to_grand_master: false,
                behavior: Default::default(),
                functions: Vec::new(),
            }
        };
    profile.modes[0].splits[0].footprint = 6;
    profile.modes[0].channels = vec![
        // Intensity is 16-bit so the fine byte has to be written, not merely declared.
        channel("intensity", light_fixture::ChannelResolution::U16, vec![2]),
        channel("pan", light_fixture::ChannelResolution::U8, Vec::new()),
        channel(
            "color.red",
            light_fixture::ChannelResolution::U8,
            Vec::new(),
        ),
        channel(
            "color.green",
            light_fixture::ChannelResolution::U8,
            Vec::new(),
        ),
        channel(
            "color.blue",
            light_fixture::ChannelResolution::U8,
            Vec::new(),
        ),
    ];
    profile
}

/// A document with one fixture of [`preview_profile`], patched at universe 1 address 1.
fn preview_document(name: &str) -> (PlanningDocument, PathBuf, Uuid) {
    preview_document_with(name, light_fixture::CanonicalTransform::Identity)
}

fn preview_document_with(
    name: &str,
    transform: light_fixture::CanonicalTransform,
) -> (PlanningDocument, PathBuf, Uuid) {
    let path = temp_path(name);
    let document = PlanningDocument::create(&path, "Preview show").expect("create");
    let profile = preview_profile_with(transform);
    let profile_id = profile.id;
    let profile_revision = u64::from(profile.revision);
    let mode_id = profile.modes[0].id;
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    ShowStore::open(&path)
        .unwrap()
        .insert_fixture_profile_revision(&stored)
        .expect("retain profile");
    let fixture_id = Uuid::new_v4();
    document
        .patch_fixtures(PatchFixturesCommand {
            show_id: document.show_id(),
            fixtures: vec![PatchFixtureCandidate {
                profile: PatchedFixtureProfileReference {
                    profile_id,
                    profile_revision,
                    mode_id,
                },
                patch: PatchedFixturePatch {
                    fixture_id: FixtureId(fixture_id),
                    fixture_number: Some(1),
                    virtual_fixture_number: None,
                    name: "Preview 1".into(),
                    universe: Some(1),
                    address: Some(1),
                    split_patches: vec![SplitPatch {
                        split: 1,
                        universe: Some(1),
                        address: Some(1),
                    }],
                    layer_id: "default".into(),
                    direct_control: None,
                    location: FixtureLocation { x: 0, y: 0, z: 0 },
                    rotation: FixtureVector {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    logical_heads: Vec::new(),
                    multipatch: Vec::new(),
                    group_masters_enabled: true,
                    grand_master_enabled: true,
                    invert_pan: false,
                    invert_tilt: false,
                    bracket_angle: 0.0,
                    shaper_angle: None,
                    installed_appearance: Default::default(),
                    move_in_black_enabled: true,
                    move_in_black_delay_millis: 0,
                    highlight_overrides: BTreeMap::new(),
                },
            }],
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: Vec::new(),
        })
        .expect("patch one fixture");
    (document, path, fixture_id)
}

fn only_universe(snapshot: &crate::PreviewSnapshot) -> Vec<u8> {
    assert_eq!(snapshot.universes.len(), 1, "one patched universe");
    assert_eq!(snapshot.universes[0].universe, 1);
    snapshot.universes[0].slots.clone()
}

#[test]
fn a_semantic_intensity_writes_its_coarse_and_fine_bytes() {
    let (document, _path, fixture_id) = preview_document("semantic-intensity");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 1.0,
        colour: [0.0; 3],
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!(slots[0], 255, "coarse");
    assert_eq!(slots[1], 255, "fine");

    // A level that lands between coarse steps is where the fine byte is the only way to be
    // right: 10% of 16-bit full scale is 6554, which no 8-bit channel could express.
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 0.1,
        colour: [0.0; 3],
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!((u16::from(slots[0]) << 8) | u16::from(slots[1]), 6_554);
    assert_ne!(slots[1], 0, "the fine byte carries the remainder");
}

#[test]
fn a_colour_reaches_every_component_the_fixture_has() {
    let (document, _path, fixture_id) = preview_document("semantic-colour");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Colour,
        value: 0.0,
        colour: [1.0, 0.5, 0.0],
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!(slots[3], 255, "red");
    assert_eq!(slots[4], 128, "green");
    assert_eq!(slots[5], 0, "blue");
}

/// Full DMX mode addresses a slot of the fixture's own footprint, not of the universe, so the
/// values follow the fixture when it is repatched.
#[test]
fn a_raw_slot_is_written_at_the_fixtures_own_address() {
    let (document, _path, fixture_id) = preview_document("raw-slot");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Slot {
        fixture_id,
        split: 1,
        offset: 3,
        value: 200,
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!(slots[2], 200, "offset 3 of a fixture at address 1");
}

/// A slot beyond the fixture's own footprint is refused rather than written into whatever is
/// patched next door.
#[test]
fn a_raw_slot_outside_the_footprint_touches_nothing() {
    let (document, _path, fixture_id) = preview_document("raw-slot-overrun");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Slot {
        fixture_id,
        split: 1,
        offset: 99,
        value: 200,
    });
    assert!(
        source.preview_snapshot().universes.is_empty(),
        "nothing was written for a slot the fixture does not have"
    );
}

#[test]
fn clearing_returns_every_fixture_to_its_defaults() {
    let (document, _path, fixture_id) = preview_document("clear");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 1.0,
        colour: [0.0; 3],
    });
    assert!(source.preview_is_active());
    source.clear_preview();
    assert!(!source.preview_is_active());
    assert!(source.preview_snapshot().universes.is_empty());
}

/// Preview values are session state of the window, not of the show. Opening another document must
/// not light its rig with the last one's look.
#[test]
fn opening_another_document_drops_the_preview_values() {
    let (document, _path, fixture_id) = preview_document("preview-open-one");
    let source = SceneSource::new(document);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 1.0,
        colour: [0.0; 3],
    });
    assert!(source.preview_is_active());

    let (other, _other_path, _other_fixture) = preview_document("preview-open-two");
    source.open(other);
    assert!(!source.preview_is_active());
}

/// The revision has to move on every change, because it is the only thing telling the renderer
/// that a look it already applied is no longer current.
#[test]
fn every_preview_change_moves_the_revision() {
    let (document, _path, fixture_id) = preview_document("preview-revision");
    let source = SceneSource::new(document);
    let start = source.preview_snapshot().revision;
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 0.25,
        colour: [0.0; 3],
    });
    let after_set = source.preview_snapshot().revision;
    source.clear_preview();
    let after_clear = source.preview_snapshot().revision;
    assert!(after_set > start, "setting a value moved the revision");
    assert!(after_clear > after_set, "clearing moved it again");
}

/// The show file is what the operator saves. A preview look must never reach it.
#[tokio::test]
async fn preview_values_never_enter_the_document() {
    let (document, path, fixture_id) = preview_document("preview-not-persisted");
    let source = SceneSource::new(document);
    let before = std::fs::read(&path).expect("show bytes");
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id,
        parameter: crate::PreviewParameter::Intensity,
        value: 1.0,
        colour: [0.0; 3],
    });
    // Read the plane back through the route the renderer uses, so the whole path has run.
    let snapshot: crate::PreviewSnapshot = get(&source, "/api/v2/preview-values").await;
    assert_eq!(snapshot.universes.len(), 1);
    assert_eq!(
        std::fs::read(&path).expect("show bytes"),
        before,
        "setting a preview value changed the show file"
    );
}

/// A subtractive fixture is canonically its additive opposite carrying an inverting transform — a
/// CMY fixture's cyan is `color.red`, inverted. Ignoring that writes the complement of the colour
/// the operator chose, which looks like a working colour picker doing the wrong thing.
#[test]
fn a_subtractive_colour_channel_is_inverted_before_it_is_written() {
    let (additive, _path, additive_id) = preview_document("colour-additive");
    let source = SceneSource::new(additive);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id: additive_id,
        parameter: crate::PreviewParameter::Colour,
        value: 0.0,
        colour: [1.0, 0.0, 0.0],
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!((slots[3], slots[4]), (255, 0), "additive red, no green");

    let (subtractive, _other, subtractive_id) = preview_document_with(
        "colour-subtractive",
        light_fixture::CanonicalTransform::InvertNormalized,
    );
    let source = SceneSource::new(subtractive);
    source.set_preview(crate::PreviewSet::Semantic {
        fixture_id: subtractive_id,
        parameter: crate::PreviewParameter::Colour,
        value: 0.0,
        colour: [1.0, 0.0, 0.0],
    });
    let slots = only_universe(&source.preview_snapshot());
    assert_eq!(
        (slots[3], slots[4]),
        (0, 255),
        "the same red on a subtractive fixture opens green and magenta rather than red"
    );
}
