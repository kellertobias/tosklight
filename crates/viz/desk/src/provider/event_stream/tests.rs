//! Real loopback HTTP/WS: a held source read must not block config or operator resync.
use super::*;
use futures_util::SinkExt;
use std::sync::{Arc, atomic::AtomicUsize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinSet,
};

struct OwnedServer(tokio::task::JoinHandle<()>);
impl Drop for OwnedServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let byte = stream.read_u8().await.unwrap();
        bytes.push(byte);
        assert!(bytes.len() <= 16_384);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes).unwrap();
        }
    }
}
async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
}
async fn serve(mut stream: TcpStream, started: Arc<Notify>, reads: Arc<AtomicUsize>) {
    let mut prefix = [0; 256];
    let size = stream.peek(&mut prefix).await.unwrap();
    if String::from_utf8_lossy(&prefix[..size]).starts_with("GET /api/v2/events ") {
        let mut socket = tokio_tungstenite::accept_hdr_async(stream,
            |_: &tokio_tungstenite::tungstenite::handshake::server::Request,
             mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                response.headers_mut().insert("sec-websocket-protocol", "light.events.v2".parse().unwrap());
                Ok(response)
            }).await.unwrap();
        let subscribe = socket.next().await.unwrap().unwrap().into_text().unwrap();
        assert!(subscribe.contains("subscribe"));
        started.notified().await;
        for _ in 0..100 {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"kind":"timecode_projection_tick"}"#.into(),
                ))
                .await
                .unwrap();
        }
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"visualizer_selection_changed"}"#.into(),
            ))
            .await
            .unwrap();
        while socket.next().await.is_some() {}
        return;
    }
    let request = headers(&mut stream).await;
    let path = request.split_whitespace().nth(1).unwrap();
    match path {
        "/api/v2/sessions" => respond(&mut stream,"200 OK",r#"{"session_id":"12345678-1234-4234-8234-123456789012","token":"test","role":"visualizer"}"#).await,
        "/api/v2/output/dmx" => {
            reads.fetch_add(1,Ordering::Relaxed);
            started.notify_one();
            // Intentionally unresolved until this owned server task is aborted on cleanup.
            std::future::pending::<()>().await;
        },
        "/api/v2/selection" => respond(&mut stream,"200 OK",r#"{"revision":7,"selected_fixture_ids":[]}"#).await,
        _ => respond(&mut stream,"404 Not Found","{}").await,
    }
}
async fn server() -> (u16, Arc<AtomicUsize>, OwnedServer) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let reads = Arc::new(AtomicUsize::new(0));
    let count = reads.clone();
    let started = Arc::new(Notify::new());
    let worker = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            connections.spawn(serve(stream, started.clone(), count.clone()));
        }
    });
    (port, reads, OwnedServer(worker))
}
#[tokio::test]
async fn slow_source_http_does_not_block_configuration_or_resync_and_never_overlaps() {
    let (port, reads, _owned) = server().await;
    let mut client = DeskClient::new("127.0.0.1", port).unwrap();
    assert!(client.open_session().await.unwrap());
    let (outbox, inbox) = std::sync::mpsc::channel();
    let (commands, mut orders) = tokio::sync::mpsc::unbounded_channel();
    let watching = tokio::spawn(async move {
        let connection = DeskConnection {
            port,
            values_from_desk_output: true,
            ..Default::default()
        };
        let stop = AtomicBool::new(false);
        let demand = super::super::output_demand::OutputDemand::default();
        watch(
            &client,
            &format!("http://127.0.0.1:{port}"),
            &connection,
            &outbox,
            &mut orders,
            &stop,
            &demand,
        )
        .await;
    });
    let selection = tokio::task::spawn_blocking(move || {
        loop {
            match inbox
                .recv_timeout(Duration::from_secs(2))
                .expect("config must arrive while source response remains held")
            {
                Message::Selection(selection) => {
                    assert_eq!(selection.revision, 7);
                    break;
                }
                _ => {}
            }
        }
    });
    selection.await.unwrap();
    assert_eq!(
        reads.load(Ordering::Relaxed),
        1,
        "only one source read can be in flight despite event flood"
    );
    commands.send(Command::Resync).unwrap();
    tokio::time::timeout(Duration::from_secs(2), watching)
        .await
        .expect("resync must interrupt held source HTTP")
        .unwrap();
}
