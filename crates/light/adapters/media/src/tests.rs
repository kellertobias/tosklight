use super::*;
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

#[tokio::test]
async fn negotiates_and_retrieves_thumbnail_and_preview() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let cinf = read_wire_packet(&mut stream).await;
        assert_eq!(&cinf[22..26], b"CInf");
        stream
            .write_all(&encode_packet((1, 2), 1, *b"SInf", &[]))
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
            .write_all(&encode_packet((1, 0), 1, *b"SInf", &[]))
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
            .write_all(&encode_packet((1, 2), 1, *b"SInf", &[]))
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
