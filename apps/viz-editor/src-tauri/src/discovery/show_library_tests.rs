use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn request(listener: &tokio::net::TcpListener) -> (tokio::net::TcpStream, String) {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0; 8192];
        let count = stream.read(&mut buffer).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some((headers, body)) = text.split_once("\r\n\r\n") {
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            if body.len() >= length {
                break;
            }
        }
    }
    (stream, String::from_utf8(bytes).unwrap())
}

async fn respond(mut stream: tokio::net::TcpStream, body: &str) {
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
}

#[tokio::test]
async fn a_lost_save_response_retries_the_same_session_request_and_snapshot() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, session_request) = request(&listener).await;
        assert!(session_request.starts_with("POST /api/v2/sessions "));
        respond(
            stream,
            r#"{"token":"operator-token","session_id":"save-session"}"#,
        )
        .await;
        let (stream, first) = request(&listener).await;
        assert!(first.starts_with("POST /api/v2/shows "));
        drop(stream); // The server committed, but its response was lost.
        let (stream, second) = request(&listener).await;
        assert!(second.starts_with("POST /api/v2/shows "));
        assert_eq!(
            first.split_once("\r\n\r\n").unwrap().1,
            second.split_once("\r\n\r\n").unwrap().1
        );
        assert!(
            second
                .to_ascii_lowercase()
                .contains("authorization: bearer operator-token")
        );
        respond(
            stream,
            r#"{"result":{"type":"document_updated","document_revision":42},"replayed":true}"#,
        )
        .await;
        let (stream, closed) = request(&listener).await;
        assert!(closed.starts_with("DELETE /api/v2/sessions/save-session "));
        respond(stream, "{}").await;
    });
    let directory = std::path::PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap())
        .join(format!("architect-retry-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let source = SyncBinding::new(None, document.show_id().0, base, "Desk".into(), 0);
    drop(document);
    let session = Session::default();
    session.open_from_desk(&path, source).unwrap();
    assert!(
        save_to_desk(&session)
            .await
            .unwrap_err()
            .contains("unconfirmed")
    );
    assert!(session.pending_desk_save.lock().is_some());
    session
        .rename_to("New local edit after response loss")
        .unwrap();
    assert_eq!(
        save_to_desk(&session).await.unwrap(),
        "Saved the earlier snapshot to Desk; press Save again to send newer local edits"
    );
    assert_eq!(
        session
            .binding
            .lock()
            .as_ref()
            .unwrap()
            .acknowledged_show_revision,
        42
    );
    assert!(session.pending_desk_save.lock().is_none());
    server.await.unwrap();
    drop(session);
    let _ = std::fs::remove_dir_all(directory);
}
