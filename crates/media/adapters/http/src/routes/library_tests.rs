use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use crate::diagnostics::{
    Diagnostics, ImportJob, ImportOutcome, Imports, LibraryAccess, LibraryEdit, LibraryNoteTarget,
    PendingImport, UploadStream,
};
use crate::routes::bench::{bench, bench_with, get, post, send};

/// A process with two files waiting and a record of what it was asked to start.
fn ready() -> (
    Diagnostics,
    Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>>,
) {
    let asked: Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>> =
        Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&asked);
    let diagnostics = Diagnostics {
        imports: Imports {
            state: Arc::new(|| {
                (
                    vec![
                        PendingImport {
                            destination: media_domain::MediaAddress::new(1, 1),
                            name: "Bars".to_owned(),
                            filename: "001-Bars.mp4".to_owned(),
                        },
                        PendingImport {
                            destination: media_domain::MediaAddress::new(1, 4),
                            name: "LoopTest".to_owned(),
                            filename: "004-LoopTest.mp4".to_owned(),
                        },
                    ],
                    vec![ImportJob {
                        id: "job-1".to_owned(),
                        batch_id: None,
                        destination: media_domain::MediaAddress::new(2, 1),
                        filename: "001.png".to_owned(),
                        outcome: ImportOutcome::Running,
                        attempts: 1,
                        fraction: Some(0.5),
                        frames_done: Some(50),
                        frames_total: Some(100),
                    }],
                )
            }),
            start: Arc::new(move |address| {
                recorder.lock().unwrap().push(address);
                2
            }),
            cancel: Arc::new(|job| job == "job-1"),
            available: true,
        },
        ..Default::default()
    };
    (diagnostics, asked)
}

async fn post_ok(router: &axum::Router, path: impl Into<String>, body: &str) {
    let (status, _) = send(router, post(path.into(), body)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_library_reports_what_is_waiting_and_what_is_running() {
    let (diagnostics, _) = ready();
    let bench = bench_with(diagnostics);
    let (status, body) = send(&bench.router, get("/api/v2/library/imports".into())).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["canImport"], true);
    assert_eq!(body["pending"][0]["address"]["folder"], 1);
    assert_eq!(body["pending"][0]["filename"], "001-Bars.mp4");
    assert_eq!(body["pending"][1]["name"], "LoopTest");
    assert_eq!(body["jobs"][0]["state"], "running");
    assert_eq!(body["jobs"][0]["fraction"], 0.5);
}

#[tokio::test]
async fn importing_everything_waiting_names_no_address() {
    let (diagnostics, asked) = ready();
    let bench = bench_with(diagnostics);
    let (status, body) = send(
        &bench.router,
        post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(asked.lock().unwrap()[0], None, "everything waiting");
    assert!(body["jobs"].as_array().is_some());
}

#[tokio::test]
async fn one_address_imports_only_that_one() {
    let (diagnostics, asked) = ready();
    let bench = bench_with(diagnostics);
    send(
        &bench.router,
        post(
            "/api/v2/library/import".into(),
            r#"{"requestId":"a","folder":1,"file":4}"#,
        ),
    )
    .await;

    assert_eq!(
        asked.lock().unwrap()[0],
        Some(media_domain::MediaAddress::new(1, 4))
    );
}

#[tokio::test]
async fn half_an_address_is_refused_rather_than_guessed_at() {
    let (diagnostics, asked) = ready();
    let bench = bench_with(diagnostics);
    let (status, body) = send(
        &bench.router,
        post(
            "/api/v2/library/import".into(),
            r#"{"requestId":"a","folder":1}"#,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "incomplete-address");
    assert!(
        asked.lock().unwrap().is_empty(),
        "a folder's worth of transcoding did not start by accident"
    );
}

#[tokio::test]
async fn a_retried_start_does_not_transcode_the_library_twice() {
    let (diagnostics, asked) = ready();
    let bench = bench_with(diagnostics);
    let body = r#"{"requestId":"same"}"#;

    let (_, first) = send(&bench.router, post("/api/v2/library/import".into(), body)).await;
    let (status, second) = send(&bench.router, post("/api/v2/library/import".into(), body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(first, second);
    assert_eq!(asked.lock().unwrap().len(), 1, "it was started once");
}

#[tokio::test]
async fn a_machine_that_cannot_transcode_says_so_before_anything_is_queued() {
    let bench = bench();
    let (status, body) = send(
        &bench.router,
        post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "cannot-import");
    assert!(body["message"].as_str().unwrap().contains("FFmpeg"));
}

#[tokio::test]
async fn importing_when_nothing_is_waiting_says_so() {
    let diagnostics = Diagnostics {
        imports: Imports {
            available: true,
            start: Arc::new(|_| 0),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let (status, body) = send(
        &bench.router,
        post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "nothing-to-import");
}

#[tokio::test]
async fn a_running_import_can_be_stopped_and_stopping_it_twice_says_so() {
    let (diagnostics, _) = ready();
    let bench = bench_with(diagnostics);

    use tower::ServiceExt as _;
    let response = bench
        .router
        .clone()
        .oneshot(get("/api/v2/library/imports/job-1/cancel".into()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-store"
    );

    let (status, body) = send(
        &bench.router,
        get("/api/v2/library/imports/job-9/cancel".into()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "unknown-import");
}

#[tokio::test]
async fn item_and_folder_edits_carry_stable_intent_to_the_library() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let id = uuid::Uuid::new_v4();
    post_ok(
        &bench.router,
        format!("/api/v2/library/items/{id}/update"),
        r#"{"requestId":"rename","name":"Opening"}"#,
    )
    .await;
    post_ok(
        &bench.router,
        "/api/v2/library/folders/7/update",
        r#"{"requestId":"compact-folder","compact":true}"#,
    )
    .await;
    post_ok(
        &bench.router,
        "/api/v2/library/folders/7/update",
        r#"{"requestId":"park-folder","swapWith":900}"#,
    )
    .await;
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/update"),
            r#"{"requestId":"park-item","folder":900,"file":1}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/update"),
            r#"{"requestId":"bpm","intrinsicBpm":128.5}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            "/api/v2/library/folders/7/update".into(),
            r#"{"requestId":"folder","name":"Looks"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            "/api/v2/library/folders/7/update".into(),
            r#"{"requestId":"folder-icon","icon":"▶"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/notes/update".into(),
                &format!(
                    r#"{{"requestId":"notes","targets":[{{"kind":"item","id":"{id}"}},{{"kind":"folder","folder":7}}],"note":"Licence: CC BY 4.0"}}"#
                ),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/update"),
            r#"{"requestId":"disable","enabled":false}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/update"),
            r#"{"requestId":"move","folder":2,"file":8,"swap":true}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/delete"),
            r#"{"requestId":"delete"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        edits.lock().unwrap().as_slice(),
        [
            LibraryEdit::RenameItem {
                id: media_domain::AssetId::from_uuid(id),
                name: "Opening".to_owned(),
            },
            LibraryEdit::CompactFolder { folder: 7 },
            LibraryEdit::SwapFolders {
                first: 7,
                second: 900,
            },
            LibraryEdit::MoveItem {
                id: media_domain::AssetId::from_uuid(id),
                destination: media_domain::CatalogLocation::new(900, 1),
                swap: false,
            },
            LibraryEdit::SetItemBpm {
                id: media_domain::AssetId::from_uuid(id),
                bpm: Some(128.5),
            },
            LibraryEdit::RenameFolder {
                folder: 7,
                name: Some("Looks".to_owned()),
            },
            LibraryEdit::SetFolderIcon {
                folder: 7,
                icon: Some("▶".to_owned()),
            },
            LibraryEdit::SetNotes {
                targets: vec![
                    LibraryNoteTarget::Item(media_domain::AssetId::from_uuid(id)),
                    LibraryNoteTarget::Folder(7),
                ],
                note: Some("Licence: CC BY 4.0".to_owned()),
            },
            LibraryEdit::SetItemEnabled {
                id: media_domain::AssetId::from_uuid(id),
                enabled: false
            },
            LibraryEdit::MoveItem {
                id: media_domain::AssetId::from_uuid(id),
                destination: media_domain::MediaAddress::new(2, 8).into(),
                swap: true,
            },
            LibraryEdit::DeleteItem {
                id: media_domain::AssetId::from_uuid(id)
            },
        ]
    );
}

#[tokio::test]
async fn invalid_or_duplicate_note_targets_are_refused_before_the_library_changes() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);

    let (status, body) = send(
        &bench.router,
        post(
            "/api/v2/library/notes/update".into(),
            r#"{"requestId":"empty","targets":[],"note":"Licence"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid-note-targets");

    let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/notes/update".into(),
                r#"{"requestId":"duplicate","targets":[{"kind":"folder","folder":7},{"kind":"folder","folder":7}],"note":"Licence"}"#,
            ),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "duplicate-note-target");
    assert!(edits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_library_item_is_replay_safe() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let id = uuid::Uuid::new_v4();
    let request = post(
        format!("/api/v2/library/items/{id}/delete"),
        r#"{"requestId":"same-delete"}"#,
    );
    let (first, _) = send(&bench.router, request).await;
    let (second, _) = send(
        &bench.router,
        post(
            format!("/api/v2/library/items/{id}/delete"),
            r#"{"requestId":"same-delete"}"#,
        ),
    )
    .await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK);
    assert_eq!(
        edits.lock().unwrap().as_slice(),
        [LibraryEdit::DeleteItem {
            id: media_domain::AssetId::from_uuid(id),
        }]
    );
}

#[tokio::test]
async fn bulk_item_edits_use_stable_ids_and_are_replay_safe() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let enable_body =
        format!(r#"{{"requestId":"bulk-enable","ids":["{first}","{second}"],"enabled":false}}"#);
    let delete_body = format!(r#"{{"requestId":"bulk-delete","ids":["{first}","{second}"]}}"#);

    for _ in 0..2 {
        let (status, _) = send(
            &bench.router,
            post("/api/v2/library/items/update".into(), &enable_body),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    for _ in 0..2 {
        let (status, _) = send(
            &bench.router,
            post("/api/v2/library/items/delete".into(), &delete_body),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let ids = vec![
        media_domain::AssetId::from_uuid(first),
        media_domain::AssetId::from_uuid(second),
    ];
    assert_eq!(
        edits.lock().unwrap().as_slice(),
        [
            LibraryEdit::SetItemsEnabled {
                ids: ids.clone(),
                enabled: false,
            },
            LibraryEdit::DeleteItems { ids },
        ]
    );
}

#[tokio::test]
async fn bulk_item_edits_refuse_empty_and_duplicate_selections() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let id = uuid::Uuid::new_v4();
    let (empty, _) = send(
        &bench.router,
        post(
            "/api/v2/library/items/update".into(),
            r#"{"requestId":"empty-bulk","ids":[],"enabled":false}"#,
        ),
    )
    .await;
    let (duplicate, body) = send(
        &bench.router,
        post(
            "/api/v2/library/items/delete".into(),
            &format!(r#"{{"requestId":"duplicate-bulk","ids":["{id}","{id}"]}}"#),
        ),
    )
    .await;

    assert_eq!(empty, StatusCode::BAD_REQUEST);
    assert_eq!(duplicate, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "duplicate-item-selection");
    assert!(edits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn compacting_a_library_folder_is_replay_safe() {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&edits);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            edit: Arc::new(move |edit| {
                recorded.lock().unwrap().push(edit);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let request = post(
        "/api/v2/library/folders/7/update".into(),
        r#"{"requestId":"same-compact","compact":true}"#,
    );
    let (first, _) = send(&bench.router, request).await;
    let (second, _) = send(
        &bench.router,
        post(
            "/api/v2/library/folders/7/update".into(),
            r#"{"requestId":"same-compact","compact":true}"#,
        ),
    )
    .await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK);
    assert_eq!(
        edits.lock().unwrap().as_slice(),
        [LibraryEdit::CompactFolder { folder: 7 }]
    );
}

#[tokio::test]
async fn a_thumbnail_is_a_jpeg_and_a_missing_one_says_so() {
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            thumbnail: Arc::new(|address| {
                if address == media_domain::MediaAddress::new(3, 7).into() {
                    Ok(vec![0xff, 0xd8, 0xff, 0xd9])
                } else {
                    Err("missing".to_owned())
                }
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);

    let response = bench
        .router
        .clone()
        .oneshot(get("/api/v2/library/3/7/thumbnail".into()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        "image/jpeg"
    );
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-store"
    );
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        &[0xff, 0xd8, 0xff, 0xd9][..]
    );

    let (status, body) = send(&bench.router, get("/api/v2/library/3/8/thumbnail".into())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "thumbnail-not-found");
}

#[tokio::test]
async fn a_native_media_preview_uses_the_catalog_item_and_never_a_client_path() {
    let seen = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&seen);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            preview_frame: Arc::new(move |location, name, frame| {
                *recorded.lock().unwrap() = Some((location, name.to_owned(), frame));
                Ok(vec![0xff, 0xd8, 0xff, 0xd9])
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let mut catalog = media_domain::CatalogSnapshot::default();
    catalog
        .insert(
            3,
            media_domain::CatalogItem {
                id: media_domain::AssetId::new(),
                file: 7,
                name: "House loop".to_owned(),
                kind: media_domain::ItemKind::Video,
                width: 1920,
                height: 1080,
                frames: Some(42),
                intrinsic_bpm: None,
                note: None,
                enabled: true,
            },
        )
        .unwrap();
    bench.api.catalog.store(Arc::new(catalog));

    let response = bench
        .router
        .clone()
        .oneshot(get("/api/v2/library/3/7/preview?frame=9".into()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        "image/jpeg"
    );
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-store"
    );
    assert_eq!(
        seen.lock().unwrap().as_ref().unwrap().1,
        "House loop",
        "the server resolved the name from its immutable catalog"
    );
    assert_eq!(seen.lock().unwrap().as_ref().unwrap().2, 9);
}

#[tokio::test]
async fn retrying_a_thumbnail_uses_one_stable_id_and_replays_safely() {
    let regenerated = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&regenerated);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            regenerate_thumbnail: Arc::new(move |id| {
                recorded.lock().unwrap().push(id);
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let id = uuid::Uuid::new_v4();
    for _ in 0..2 {
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/thumbnail/retry"),
                r#"{"requestId":"same-thumbnail-retry"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    assert_eq!(
        regenerated.lock().unwrap().as_slice(),
        &[media_domain::AssetId::from_uuid(id)]
    );
}

#[tokio::test]
async fn custom_thumbnail_upload_is_bounded_typed_and_replay_safe() {
    let stored = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&stored);
    let diagnostics = Diagnostics {
        library: LibraryAccess {
            set_custom_thumbnail: Arc::new(move |id, bytes| {
                recorded.lock().unwrap().push((id, bytes.to_vec()));
                Ok(())
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let id = uuid::Uuid::new_v4();
    let boundary = "thumbnail-boundary";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"thumbnail.png\"\r\nContent-Type: image/png\r\n\r\nPNG-BYTES\r\n--{boundary}--\r\n"
    );
    for _ in 0..2 {
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v2/library/items/{id}/thumbnail/upload?requestId=same-custom-thumbnail"
            ))
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(payload.clone()))
            .unwrap();
        let response = bench.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert_eq!(stored.lock().unwrap().len(), 1);
    assert_eq!(stored.lock().unwrap()[0].1, b"PNG-BYTES");

    let boundary = "bad-thumbnail-boundary";
    let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v2/library/items/{id}/thumbnail/upload?requestId=bad-thumbnail"
            ))
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bad.txt\"\r\nContent-Type: text/plain\r\n\r\nNO\r\n--{boundary}--\r\n"
            )))
            .unwrap();
    let response = bench.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

struct RecordedUpload {
    bytes: Arc<Mutex<Vec<u8>>>,
    finished: Arc<AtomicBool>,
}

impl UploadStream for RecordedUpload {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.bytes.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<String, String> {
        self.finished.store(true, Ordering::SeqCst);
        Ok("job-upload".to_owned())
    }
}

#[tokio::test]
async fn upload_streams_one_file_and_returns_the_import_job() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let finished = Arc::new(AtomicBool::new(false));
    let received = Arc::new(Mutex::new(None));
    let diagnostics = Diagnostics {
        imports: Imports {
            available: true,
            ..Default::default()
        },
        library: LibraryAccess {
            begin_upload: {
                let bytes = Arc::clone(&bytes);
                let finished = Arc::clone(&finished);
                let received = Arc::clone(&received);
                Arc::new(move |address, name, filename, replace| {
                    *received.lock().unwrap() =
                        Some((address, name.to_owned(), filename.to_owned(), replace));
                    Ok(Box::new(RecordedUpload {
                        bytes: Arc::clone(&bytes),
                        finished: Arc::clone(&finished),
                    }))
                })
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let bench = bench_with(diagnostics);
    let boundary = "media-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"opening.mov\"\r\nContent-Type: video/quicktime\r\n\r\npixels\r\n--{boundary}--\r\n"
    );
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v2/library/3/7/upload?requestId=upload&name=Opening&replace=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .unwrap();
    let (status, answer) = send(&bench.router, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["jobId"], "job-upload");
    assert_eq!(&*bytes.lock().unwrap(), b"pixels");
    assert!(finished.load(Ordering::SeqCst));
    assert_eq!(
        *received.lock().unwrap(),
        Some((
            media_domain::MediaAddress::new(3, 7),
            "Opening".to_owned(),
            "opening.mov".to_owned(),
            true,
        ))
    );
}
