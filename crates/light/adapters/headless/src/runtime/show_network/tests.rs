use super::*;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, post},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct MockDesk {
    closed: Arc<AtomicUsize>,
    downloads: Arc<AtomicUsize>,
    id: Uuid,
}

async fn mock_desk() -> (String, MockDesk, tokio::task::JoinHandle<()>) {
    let state = MockDesk {
        closed: Arc::default(),
        downloads: Arc::default(),
        id: Uuid::new_v4(),
    };
    let router = Router::new()
        .route("/api/v2/sessions", post(|| async { Json(serde_json::json!({"session_id":Uuid::nil(),"token":"observer"})) }))
        .route("/api/v2/sessions/{id}", delete(|State(state):State<MockDesk>, headers:HeaderMap| async move {
            assert_eq!(headers.get("authorization").unwrap(), "Bearer observer");
            state.closed.fetch_add(1, Ordering::SeqCst);
            StatusCode::NO_CONTENT
        }))
        .route("/api/v2/shows", get(|State(state):State<MockDesk>, headers:HeaderMap| async move {
            assert_eq!(headers.get("authorization").unwrap(), "Bearer observer");
            Json(serde_json::json!({"shows":[{"id":state.id,"name":"Remote rig","path":"remote.show","revision":1,"updated_at":"saved","revisions":[{"show_id":state.id,"revision":2,"name":"Approved","created_at":"named"}]}]}))
        }))
        .route("/api/v2/shows/{id}/download", get(|State(state):State<MockDesk>| async move {
            state.downloads.fetch_add(1, Ordering::SeqCst); "latest bytes"
        }))
        .route("/api/v2/shows/{id}/revisions/{revision}/download", get(|State(state):State<MockDesk>| async move {
            state.downloads.fetch_add(1, Ordering::SeqCst); "named bytes"
        })).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (base, state, task)
}

#[tokio::test]
async fn catalog_and_latest_or_named_download_close_scoped_sessions() {
    let (base, state, task) = mock_desk().await;
    let client = client(Duration::from_secs(5)).unwrap();
    let catalog = desk_catalog(&client, &base).await.unwrap();
    assert_eq!(catalog.shows[0].revisions[0].revision, 2);
    let latest = desk_document(&client, &base, state.id, None).await.unwrap();
    assert_eq!(latest.0, "Remote rig");
    assert_eq!(latest.1, b"latest bytes");
    let named = desk_document(&client, &base, state.id, Some(2))
        .await
        .unwrap();
    assert_eq!(named.1, b"named bytes");
    assert_eq!(state.closed.load(Ordering::SeqCst), 3);
    task.abort();
}

#[tokio::test]
async fn missing_uuid_or_revision_never_downloads_a_substitute_and_closes_sessions() {
    let (base, state, task) = mock_desk().await;
    let client = client(Duration::from_secs(5)).unwrap();
    assert!(
        desk_document(&client, &base, Uuid::new_v4(), None)
            .await
            .unwrap_err()
            .contains("no longer available")
    );
    assert!(
        desk_document(&client, &base, state.id, Some(9))
            .await
            .unwrap_err()
            .contains("revision")
    );
    assert_eq!(state.downloads.load(Ordering::SeqCst), 0);
    assert_eq!(state.closed.load(Ordering::SeqCst), 2);
    task.abort();
}

#[tokio::test]
async fn editor_catalog_lists_only_its_advertised_open_document() {
    let peer = Peer {
        role: Role::Editor,
        name: "Architect".into(),
        show: Some("Tour".into()),
        addresses: vec!["127.0.0.1:1".into()],
        instance: "editor-instance".into(),
    };
    let client = client(Duration::from_secs(1)).unwrap();
    let entry = project_peer(&client, peer.clone()).await;
    assert_eq!(entry.shows.len(), 1);
    assert_eq!(entry.shows[0].name, "Tour");
    assert_eq!(entry.shows[0].id, None);
    assert!(entry.error.is_none());
    assert!(
        project_peer(&client, Peer { show: None, ..peer })
            .await
            .shows
            .is_empty()
    );
}

#[derive(Clone)]
struct SaveDesk {
    directory: std::path::PathBuf,
    closed: Arc<AtomicUsize>,
    requests: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    replies: Arc<std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>>>,
    dropped_reply: Arc<std::sync::atomic::AtomicBool>,
}

async fn mock_save_desk() -> (String, SaveDesk, tokio::task::JoinHandle<()>) {
    let directory = std::env::temp_dir().join(format!("network-save-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(directory.join("Tour")).unwrap();
    let state = SaveDesk {
        directory,
        closed: Arc::default(),
        requests: Arc::default(),
        replies: Arc::default(),
        dropped_reply: Arc::default(),
    };
    let router=Router::new()
        .route("/api/v2/sessions",post(|Json(input):Json<serde_json::Value>|async move {
            let role=input["role"].as_str().unwrap();
            assert!(role=="visualizer" || role=="operator");
            Json(serde_json::json!({"session_id":Uuid::new_v4(),"token":role}))
        }))
        .route("/api/v2/sessions/{id}",delete(|State(state):State<SaveDesk>,headers:HeaderMap|async move {
            assert!(matches!(headers.get("authorization").unwrap().to_str().unwrap(),"Bearer visualizer"|"Bearer operator"));
            state.closed.fetch_add(1,Ordering::SeqCst);
            StatusCode::NO_CONTENT
        }))
        .route("/api/v2/files/roots",get(|headers:HeaderMap|async move {
            assert_eq!(headers.get("authorization").unwrap(),"Bearer visualizer");
            Json(serde_json::json!([
                {"id":"shows","label":"Shows","icon":"folder","removable":false,"writable":true},
                {"id":"readonly","label":"Read only","icon":"folder","removable":false,"writable":false}
            ]))
        }))
        .route("/api/v2/files/shows/entries",get(|headers:HeaderMap|async move {
            assert_eq!(headers.get("authorization").unwrap(),"Bearer visualizer");
            Json(serde_json::json!({"root_id":"shows","path":"", "entries":[
                {"name":"Tour","path":"Tour","kind":"folder","size":0,"modified_millis":null,"created_millis":null,"hidden":false,"writable":true},
                {"name":"Saved.show","path":"Saved.show","kind":"file","size":8,"modified_millis":null,"created_millis":null,"hidden":false,"writable":true}
            ]}))
        }))
        .route("/api/v2/shows",post(|State(state):State<SaveDesk>,headers:HeaderMap,Json(input):Json<serde_json::Value>|async move {
            use axum::response::IntoResponse;
            assert_eq!(headers.get("authorization").unwrap(),"Bearer operator");
            state.requests.lock().unwrap().push(input.clone());
            let request_id=input["request_id"].as_str().unwrap();
            let name=input["action"]["name"].as_str().unwrap();
            if name=="Denied" {return (StatusCode::FORBIDDEN,Json(serde_json::json!({"error":"Destination read-only"}))).into_response();}
            if let Some(replay)=state.replies.lock().unwrap().get(request_id).cloned() {
                return Json(replay).into_response();
            }
            let path=state.directory.join("Tour").join(format!("{name}.show"));
            let mut file=match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file)=>file,
                Err(_)=>return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"Destination already exists"}))).into_response(),
            };
            use std::io::Write;
            let data=base64::engine::general_purpose::STANDARD.decode(input["action"]["data_base64"].as_str().unwrap()).unwrap();
            file.write_all(&data).unwrap();
            let reply=serde_json::json!({"request_id":request_id,"replayed":true,"result":{"type":"show","show":{
                "id":Uuid::new_v4(),"name":name,"path":path.display().to_string(),"revision":1,"updated_at":"now"
            }}});
            state.replies.lock().unwrap().insert(request_id.to_owned(),reply.clone());
            if !state.dropped_reply.swap(true,Ordering::SeqCst) {
                return "truncated response".into_response();
            }
            Json(reply).into_response()
        })).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (base, state, task)
}

#[tokio::test]
async fn network_save_folders_and_write_retry_cleanup_collision_and_permission() {
    let (base, state, task) = mock_save_desk().await;
    let client = client(Duration::from_secs(5)).unwrap();
    let roots = desk_save_folders(&client, &base, &FolderQuery::default())
        .await
        .unwrap();
    assert_eq!(roots.roots.len(), 1);
    let folders = desk_save_folders(
        &client,
        &base,
        &FolderQuery {
            root_id: Some("shows".into()),
            path: String::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(folders.entries.len(), 1);
    assert_eq!(folders.entries[0].name, "Tour");
    assert_eq!(
        desk_save_folders(
            &client,
            &base,
            &FolderQuery {
                root_id: Some("readonly".into()),
                path: String::new()
            }
        )
        .await
        .unwrap_err()
        .status,
        StatusCode::FORBIDDEN
    );
    let action = serde_json::json!({"type":"save_copy","source_show_id":null,"data_base64":base64::engine::general_purpose::STANDARD.encode(b"portable show snapshot"),
        "name":"Saved","root_id":"shows","path":"Tour","is_base_show":false});
    let session = writable_session(&client, &base).await.unwrap();
    let result = save_with_session(&client, &base, &session, "save-one", &action)
        .await
        .unwrap();
    assert!(matches!(result, ShowLibraryActionResult::Show { .. }));
    assert_eq!(
        std::fs::read(state.directory.join("Tour/Saved.show")).unwrap(),
        b"portable show snapshot"
    );
    let requests = state.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    let session = writable_session(&client, &base).await.unwrap();
    assert_eq!(
        save_with_session(&client, &base, &session, "save-two", &action)
            .await
            .unwrap_err()
            .status,
        StatusCode::CONFLICT
    );
    let session = writable_session(&client, &base).await.unwrap();
    let mut denied = action.clone();
    denied["name"] = "Denied".into();
    assert_eq!(
        save_with_session(&client, &base, &session, "save-denied", &denied)
            .await
            .unwrap_err()
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(state.closed.load(Ordering::SeqCst), 6);
    assert!(!state.directory.join("Tour/Denied.show").exists());
    assert_eq!(
        std::fs::read_dir(state.directory.join("Tour"))
            .unwrap()
            .count(),
        1
    );
    task.abort();
    let _ = std::fs::remove_dir_all(state.directory);
}

#[test]
fn network_save_requires_a_current_discovered_control_desk() {
    assert_eq!(
        selected_desk(None).unwrap_err().status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        selected_desk(Some(Peer {
            role: Role::Editor,
            name: "Architect".into(),
            show: None,
            addresses: vec!["127.0.0.1:1".into()],
            instance: "editor".into(),
        }))
        .unwrap_err()
        .status,
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn network_sources_exclude_this_desk_but_keep_other_desks_and_local_architect() {
    let peer = |role, instance: &str, address: &str| Peer {
        role,
        name: "Same name".into(),
        show: Some("Tour".into()),
        addresses: vec![address.into()],
        instance: instance.into(),
    };
    let peers = vec![
        peer(
            Role::Desk,
            "This-Desk._tosklight._tcp.local.",
            "127.0.0.1:5000",
        ),
        peer(
            Role::Desk,
            "other-desk._tosklight._tcp.local.",
            "192.168.1.2:5000",
        ),
        peer(
            Role::Editor,
            "architect._tosklight._tcp.local.",
            "127.0.0.1:5001",
        ),
    ];
    assert_eq!(remote_peers(peers.clone(), None), peers);
    let sources = remote_peers(peers, Some("this-desk._tosklight._tcp.local."));
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].role, Role::Desk);
    assert_eq!(sources[1].role, Role::Editor);
    assert_eq!(sources[1].address(), "127.0.0.1:5001");
}
