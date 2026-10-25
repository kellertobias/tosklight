use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

#[test]
fn double_flip_rejects_the_original_reply_and_idempotent_mode_retains_it() {
    let demand = OutputDemand::default();
    let live = demand.snapshot();
    demand.set_following(false);
    assert!(demand.accepts(live));
    demand.set_following(true);
    let preload = demand.snapshot();
    assert!(!demand.accepts(live));
    assert!(preload.follows_preload());
    demand.set_following(false);
    assert!(!demand.accepts(live));
    assert!(!demand.accepts(preload));
    assert!(!demand.snapshot().follows_preload());
}
fn mock(bodies: Vec<String>) -> (DeskClient, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut paths = Vec::new();
        for body in bodies {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "expected HTTP request never arrived"
                        );
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("mock accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            paths.push(first.split_whitespace().nth(1).unwrap().to_owned());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                assert!(!line.is_empty(), "HTTP request truncated");
            }
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        paths
    });
    (DeskClient::new("127.0.0.1", port).unwrap(), worker)
}
#[test]
fn http_reads_only_the_followed_lane_and_legacy_overlay_when_needed() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for native in [false, true] {
        for following in [false, true] {
            let mut bodies = vec![format!(
                r#"{{"native_protocol":{},"revision":1,"universes":[]}}"#,
                u8::from(native)
            )];
            if following && !native {
                bodies.push(r#"{"projection":{"fixture_values":[]}}"#.into());
            }
            let (client, worker) = mock(bodies);
            let demand = OutputDemand::default();
            demand.set_following(following);
            let request = demand.snapshot();
            let read = runtime.block_on(request.read(&client)).unwrap();
            assert_eq!(read.request, request);
            assert_eq!(read.preload.is_some(), following && !native);
            let mut expected = vec![
                if following {
                    "/api/v2/output/dmx?include_preload=true"
                } else {
                    "/api/v2/output/dmx"
                }
                .to_owned(),
            ];
            if following && !native {
                expected.push("/api/v2/programmer/preload-values/snapshot".into());
            }
            assert_eq!(worker.join().unwrap(), expected);
        }
    }
}
