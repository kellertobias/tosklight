use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};

use crate::diagnostics::{Diagnostics, ImportedModel, ModelAccess, ModelRejection};
use crate::routes::bench::{Bench, bench_with, get, post, send};

fn multipart(path: &str, filename: &str, bytes: &[u8]) -> Request<Body> {
    let boundary = "tosklight-model-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
         Content-Type: model/gltf-binary\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

struct Store {
    removed: Arc<Mutex<Vec<String>>>,
    failures: Arc<Mutex<Vec<(u8, String)>>>,
}

/// A bench whose model store accepts bytes starting with `glTF` and refuses anything else.
fn models_bench() -> (Bench, Store) {
    let removed = Arc::new(Mutex::new(Vec::new()));
    let failures = Arc::new(Mutex::new(Vec::new()));
    let removing = Arc::clone(&removed);
    let failing = Arc::clone(&failures);
    let diagnostics = Diagnostics {
        models: ModelAccess {
            import: Arc::new(|slot, bytes| {
                if bytes.starts_with(b"glTF") {
                    Ok(ImportedModel {
                        file: media_domain::ModelLibrary::stored_file_name(slot),
                        vertices: 24,
                        triangles: 12,
                    })
                } else {
                    Err(ModelRejection {
                        code: "model-missing-texture-coordinates".to_owned(),
                        message: "mesh \"Cube\" has no texture coordinates".to_owned(),
                    })
                }
            }),
            remove: Arc::new(move |file| {
                removing.lock().unwrap().push(file.to_owned());
                Ok(())
            }),
            failures: Arc::new(move || failing.lock().unwrap().clone()),
        },
        ..Default::default()
    };
    (bench_with(diagnostics), Store { removed, failures })
}

#[tokio::test]
async fn uploading_assigns_a_slot_named_after_the_file_and_persists_it() {
    let (bench, _) = models_bench();
    let (status, list) = send(&bench.router, get("/api/v2/models".into())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list, serde_json::json!([]));

    let (status, view) = send(
        &bench.router,
        multipart(
            "/api/v2/models/12/upload?requestId=upload-12",
            "Stage Cube.glb",
            b"glTF-bytes",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["slot"], 12);
    assert_eq!(view["name"], "Stage Cube");
    assert_eq!(view["triangles"], 12);
    assert_eq!(view["status"], "ready");
    let stored = bench.stored.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].models.resolve(12).unwrap().file, "model-012.glb");
}

#[tokio::test]
async fn a_refused_model_is_reported_and_changes_nothing() {
    let (bench, _) = models_bench();
    let (status, body) = send(
        &bench.router,
        multipart(
            "/api/v2/models/3/upload?requestId=bad-3",
            "cube.glb",
            b"no uvs here",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "model-missing-texture-coordinates");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("texture coordinates")
    );
    assert!(bench.stored.lock().unwrap().is_empty());
}

#[tokio::test]
async fn slot_zero_is_draw_flat_and_cannot_hold_a_model() {
    let (bench, _) = models_bench();
    let (status, body) = send(
        &bench.router,
        multipart("/api/v2/models/0/upload?requestId=zero", "a.glb", b"glTF"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "reserved-model-slot");
}

#[tokio::test]
async fn renaming_and_clearing_are_object_intent_edits() {
    let (bench, store) = models_bench();
    send(
        &bench.router,
        multipart("/api/v2/models/7/upload?requestId=u7", "a.glb", b"glTF"),
    )
    .await;

    let (status, renamed) = send(
        &bench.router,
        post(
            "/api/v2/models/7/update".into(),
            r#"{"requestId":"rename-7","name":"Back wall"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Back wall");

    let (status, body) = send(
        &bench.router,
        post(
            "/api/v2/models/8/update".into(),
            r#"{"requestId":"rename-8","name":"Nothing"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "model-slot-empty");

    let (status, cleared) = send(
        &bench.router,
        post(
            "/api/v2/models/7/update".into(),
            r#"{"requestId":"clear-7","clear":true}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cleared["assigned"], false);
    assert!(bench.configuration.load().models.resolve(7).is_none());
    assert_eq!(
        *store.removed.lock().unwrap(),
        vec!["model-007.glb".to_owned()]
    );

    // A retried clear is answered from the replay window, not executed again.
    send(
        &bench.router,
        post(
            "/api/v2/models/7/update".into(),
            r#"{"requestId":"clear-7","clear":true}"#,
        ),
    )
    .await;
    assert_eq!(store.removed.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_layer_view_reports_what_the_model_selection_resolved_to() {
    let (bench, store) = models_bench();
    send(
        &bench.router,
        multipart("/api/v2/models/1/upload?requestId=u1", "a.glb", b"glTF"),
    )
    .await;
    send(
        &bench.router,
        multipart("/api/v2/models/2/upload?requestId=u2", "b.glb", b"glTF"),
    )
    .await;
    store
        .failures
        .lock()
        .unwrap()
        .push((2, "cannot read model-002.glb".to_owned()));

    let status_of = |model: u8| {
        let mut media = media_domain::MediaState::clone(&bench.state.load());
        media.outputs[0].layers[0].model.model = model;
        bench.state.store(Arc::new(media));
    };
    let path = format!("/api/v2/outputs/{}/state", bench.output);
    for (model, expected) in [
        (0, "flat"),
        (1, "mapped"),
        (2, "unloadable"),
        (9, "missing"),
    ] {
        status_of(model);
        let (status, view) = send(&bench.router, get(path.clone())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["layers"][0]["modelStatus"], expected, "model {model}");
    }

    let (_, list) = send(&bench.router, get("/api/v2/models".into())).await;
    assert_eq!(list[1]["status"], "unloadable");
    assert_eq!(list[1]["detail"], "cannot read model-002.glb");
}
