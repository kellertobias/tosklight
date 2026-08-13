use super::*;

#[test]
fn console_announcement_carries_the_active_show_as_a_peer_location() {
    let message = console_announcement("The Tempest");
    assert_eq!(&message[..4], b"CITP");
    assert_eq!(&message[16..24], b"PINFPLoc");
    assert!(message.windows(11).any(|window| window == b"The Tempest"));
    assert!(
        message
            .windows(15)
            .any(|window| window == b"LightingConsole")
    );
}

#[test]
fn discovery_reads_the_advertised_media_server_port_and_identity() {
    let mut body = 4809_u16.to_le_bytes().to_vec();
    body.extend_from_slice(b"MediaServer\0ToskLight Media\0Running\0");
    let size = 24 + body.len();
    let mut packet = b"CITP\x01\x00\x00\x00".to_vec();
    packet.extend_from_slice(&(size as u32).to_le_bytes());
    packet.extend_from_slice(&1_u16.to_le_bytes());
    packet.extend_from_slice(&0_u16.to_le_bytes());
    packet.extend_from_slice(b"PINFPLoc");
    packet.extend_from_slice(&body);
    assert_eq!(
        parse_peer_location(&packet),
        Some((
            4809,
            "MediaServer".to_owned(),
            "ToskLight Media".to_owned(),
            "Running".to_owned()
        ))
    );
}
use crate::protocol::{encode_packet, parse_thumbnail};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn read_wire_packet(stream: &mut TcpStream) -> Vec<u8> {
    let mut header = [0; 20];
    stream.read_exact(&mut header).await.unwrap();
    let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    let mut packet = header.to_vec();
    packet.resize(size, 0);
    stream.read_exact(&mut packet[20..]).await.unwrap();
    packet
}

fn sinf(layers: u8) -> Vec<u8> {
    vec![0, 0, 0, 1, layers]
}

fn push_ucs2(output: &mut Vec<u8>, value: &str) {
    for unit in value.encode_utf16() {
        output.extend_from_slice(&unit.to_le_bytes());
    }
    output.extend_from_slice(&0_u16.to_le_bytes());
}

#[tokio::test]
async fn negotiates_and_retrieves_thumbnail_and_preview() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let cinf = read_wire_packet(&mut stream).await;
        assert_eq!(&cinf[22..26], b"CInf");
        stream
            .write_all(&encode_packet((1, 2), 1, *b"SInf", &sinf(1)))
            .await
            .unwrap();
        let geth = read_wire_packet(&mut stream).await;
        assert_eq!(&geth[22..26], b"GETh");
        let mut thumbnail = vec![1, 0, 0, 0, 0, 7];
        thumbnail.extend_from_slice(b"JPEG");
        thumbnail.extend_from_slice(&2_u16.to_le_bytes());
        thumbnail.extend_from_slice(&1_u16.to_le_bytes());
        thumbnail.extend_from_slice(&3_u16.to_le_bytes());
        thumbnail.extend_from_slice(&[1, 2, 3]);
        stream
            .write_all(&encode_packet((1, 2), 2, *b"EThn", &thumbnail))
            .await
            .unwrap();
        let rqst = read_wire_packet(&mut stream).await;
        assert_eq!(&rqst[22..26], b"RqSt");
        let mut frame = vec![b'a'; 36];
        frame.extend_from_slice(&4_u16.to_le_bytes());
        frame.extend_from_slice(b"JPEG");
        frame.extend_from_slice(&2_u16.to_le_bytes());
        frame.extend_from_slice(&1_u16.to_le_bytes());
        frame.extend_from_slice(&3_u16.to_le_bytes());
        frame.extend_from_slice(&[4, 5, 6]);
        stream
            .write_all(&encode_packet((1, 2), 3, *b"StFr", &frame))
            .await
            .unwrap();
    });
    let mut client = CitpClient::connect(address, Duration::from_secs(1))
        .await
        .unwrap();
    let thumbnails = client
        .request_thumbnail(1, LibraryId::ROOT, &[7], 64, 64)
        .await
        .unwrap();
    assert_eq!(thumbnails[0].0, 7);
    assert_eq!(thumbnails[0].1.bytes, [1, 2, 3]);
    let preview = client.request_preview(4, 64, 64).await.unwrap();
    assert_eq!(preview.bytes, [4, 5, 6]);
    server.await.unwrap();
}

#[tokio::test]
async fn honors_legacy_msex_version_and_thumbnail_layout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = read_wire_packet(&mut stream).await;
        stream
            .write_all(&encode_packet((1, 0), 1, *b"SInf", &sinf(1)))
            .await
            .unwrap();
        let geth = read_wire_packet(&mut stream).await;
        assert_eq!(&geth[20..22], &[1, 0]);
        assert_eq!(&geth[35..39], &[1, 9, 1, 4]);
        let mut thumbnail = vec![1, 9, 4];
        thumbnail.extend_from_slice(b"JPEG");
        thumbnail.extend_from_slice(&1_u16.to_le_bytes());
        thumbnail.extend_from_slice(&1_u16.to_le_bytes());
        thumbnail.extend_from_slice(&1_u16.to_le_bytes());
        thumbnail.push(42);
        stream
            .write_all(&encode_packet((1, 0), 2, *b"EThn", &thumbnail))
            .await
            .unwrap();
    });
    let mut client = CitpClient::connect(address, Duration::from_secs(1))
        .await
        .unwrap();
    let images = client
        .request_thumbnail(
            1,
            LibraryId {
                level: 1,
                ids: [9, 0, 0],
            },
            &[4],
            64,
            64,
        )
        .await
        .unwrap();
    assert_eq!(images[0].0, 4);
    assert_eq!(images[0].1.bytes, [42]);
    server.await.unwrap();
}

#[tokio::test]
async fn reassembles_ordered_citp_fragments() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = read_wire_packet(&mut stream).await;
        stream
            .write_all(&encode_packet((1, 2), 1, *b"SInf", &sinf(1)))
            .await
            .unwrap();
        let _ = read_wire_packet(&mut stream).await;
        let mut payload = vec![1, 0, 0, 0, 0, 7];
        payload.extend_from_slice(b"JPEG");
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&4_u16.to_le_bytes());
        payload.extend_from_slice(&[1, 2, 3, 4]);
        let split = 9;
        for (part, bytes) in [payload[..split].to_vec(), payload[split..].to_vec()]
            .into_iter()
            .enumerate()
        {
            let mut packet = encode_packet((1, 2), 2, *b"EThn", &bytes);
            packet[12..14].copy_from_slice(&2_u16.to_le_bytes());
            packet[14..16].copy_from_slice(&(part as u16).to_le_bytes());
            stream.write_all(&packet).await.unwrap();
        }
    });
    let mut client = CitpClient::connect(address, Duration::from_secs(1))
        .await
        .unwrap();
    let images = client
        .request_thumbnail(1, LibraryId::ROOT, &[7], 64, 64)
        .await
        .unwrap();
    assert_eq!(images[0].1.bytes, [1, 2, 3, 4]);
    server.await.unwrap();
}

#[tokio::test]
async fn inspects_advertised_library_sources_and_layer_status_without_conflating_ids() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = read_wire_packet(&mut stream).await;
        let mut server_info = Vec::new();
        push_ucs2(&mut server_info, "Peer");
        server_info.extend_from_slice(&[0, 1, 1]);
        stream
            .write_all(&encode_packet((1, 1), 1, *b"SInf", &server_info))
            .await
            .unwrap();

        assert_eq!(&read_wire_packet(&mut stream).await[22..26], b"GELI");
        let mut folders = vec![1, 1, 1, 2, 0, 0, 2, 2];
        push_ucs2(&mut folders, "Clips");
        folders.extend_from_slice(&[0, 1]);
        stream
            .write_all(&encode_packet((1, 1), 2, *b"ELIn", &folders))
            .await
            .unwrap();

        assert_eq!(&read_wire_packet(&mut stream).await[22..26], b"GEIn");
        let mut files = vec![1, 2, 0, 0, 1, 7, 7, 7];
        push_ucs2(&mut files, "Look");
        files.extend_from_slice(&42_u64.to_le_bytes());
        files.extend_from_slice(&1920_u16.to_le_bytes());
        files.extend_from_slice(&1080_u16.to_le_bytes());
        files.extend_from_slice(&250_u32.to_le_bytes());
        files.push(25);
        stream
            .write_all(&encode_packet((1, 1), 3, *b"MEIn", &files))
            .await
            .unwrap();

        assert_eq!(&read_wire_packet(&mut stream).await[22..26], b"GVSr");
        let mut sources = 1_u16.to_le_bytes().to_vec();
        sources.extend_from_slice(&9_u16.to_le_bytes());
        push_ucs2(&mut sources, "Program");
        sources.extend_from_slice(&[3, u8::MAX]);
        sources.extend_from_slice(&0_u16.to_le_bytes());
        sources.extend_from_slice(&320_u16.to_le_bytes());
        sources.extend_from_slice(&180_u16.to_le_bytes());
        stream
            .write_all(&encode_packet((1, 1), 4, *b"VSrc", &sources))
            .await
            .unwrap();

        let mut status = vec![1, 5, 3, 2, 7];
        push_ucs2(&mut status, "Look");
        status.extend_from_slice(&10_u32.to_le_bytes());
        status.extend_from_slice(&250_u32.to_le_bytes());
        status.push(25);
        status.extend_from_slice(&1_u32.to_le_bytes());
        stream
            .write_all(&encode_packet((1, 0), 0, *b"LSta", &status))
            .await
            .unwrap();
    });

    let mut client = CitpClient::connect(address, Duration::from_secs(1))
        .await
        .unwrap();
    let snapshot = client.inspect().await.unwrap();
    assert!(snapshot.library_revision.starts_with("citp-"));
    assert_eq!(snapshot.capabilities.provider, "citp_msex");
    assert!(snapshot.capabilities.native_action.is_none());
    assert_eq!(snapshot.server.name, "Peer");
    assert_eq!(snapshot.folders[0].id, 2);
    assert_eq!(snapshot.files[0].folder_id, 2);
    assert_eq!(snapshot.files[0].id, 7);
    assert_eq!(snapshot.preview_sources[0].id, 9);
    assert_eq!(snapshot.preview_sources[0].physical_output, 3);
    assert_eq!(snapshot.layers[0].layer, 5);
    assert_eq!(snapshot.layers[0].folder, 2);
    server.await.unwrap();
}

#[test]
fn cache_is_bounded_and_fixture_scoped() {
    let image = || MediaImage {
        format: ImageFormat::Jpeg,
        width: 1,
        height: 1,
        bytes: vec![1],
    };
    let mut cache = MediaCache::new(1, 1);
    let a = ThumbnailKey {
        fixture: "a".into(),
        library_type: 1,
        library: LibraryId::ROOT,
        element: 1,
    };
    let b = ThumbnailKey {
        fixture: "b".into(),
        library_type: 1,
        library: LibraryId::ROOT,
        element: 2,
    };
    cache.put_thumbnail(a.clone(), image()).unwrap();
    cache.put_thumbnail(b.clone(), image()).unwrap();
    assert!(cache.thumbnail(&a).is_none());
    assert!(cache.thumbnail(&b).is_some());
    cache.clear_fixture("b");
    assert!(cache.thumbnail(&b).is_none());
}

#[test]
fn rejects_malformed_image_lengths() {
    let mut payload = vec![1, 0, 0, 0, 0, 7];
    payload.extend_from_slice(b"JPEG");
    payload.extend_from_slice(&2_u16.to_le_bytes());
    payload.extend_from_slice(&1_u16.to_le_bytes());
    payload.extend_from_slice(&4_u16.to_le_bytes());
    payload.extend_from_slice(&[1, 2, 3]);
    assert!(parse_thumbnail(&payload, (1, 2)).is_err());
}
