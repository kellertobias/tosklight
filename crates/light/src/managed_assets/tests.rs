use std::collections::HashSet;

use super::test_support::{FakeAssetStore, FramedVecSink, VecSink, VecSource, fake_digest};
use super::*;

fn import_request(id: AssetId, name: &str, bytes: &[u8]) -> ImportAssetRequest {
    ImportAssetRequest {
        identity: Some(id),
        namespace: AssetNamespace("show:main".into()),
        name: name.into(),
        media_type: "audio/wav".into(),
        declared_length: bytes.len() as u64,
        declared_digest: fake_digest(bytes),
    }
}

#[test]
fn fake_store_proves_stable_identity_revision_stream_copy_export_and_cleanup() {
    let store = FakeAssetStore::default();
    let id = AssetId(Uuid::from_u128(1));
    let first_bytes = b"first";
    let second_bytes = b"second";
    let first = store
        .import(
            import_request(id, "Track", first_bytes),
            &mut VecSource::new(first_bytes),
        )
        .unwrap();
    let second = store
        .import(
            import_request(id, "Track", second_bytes),
            &mut VecSource::new(second_bytes),
        )
        .unwrap();

    assert_eq!(first.asset.id, second.asset.id);
    assert_eq!(first.asset.revision, AssetRevision(1));
    assert_eq!(second.asset.revision, AssetRevision(2));
    assert!(store.validate(second.asset).unwrap().valid);
    assert_eq!(store.revisions(id).unwrap().len(), 2);

    let mut streamed = VecSink::default();
    let stream = store.stream(first.asset, &mut streamed).unwrap();
    assert_eq!(stream.bytes_written, first_bytes.len() as u64);
    assert_eq!(streamed.0, first_bytes);
    assert_eq!(
        store
            .copy(CopyAssetRequest {
                asset: first.asset,
                destination: AssetNamespace("show:tour".into()),
            })
            .unwrap(),
        first.asset
    );
    assert!(
        store
            .copies()
            .contains(&(first.asset, AssetNamespace("show:tour".into())))
    );

    let mut exported = FramedVecSink::default();
    let report = store
        .export(
            ExportAssetsRequest {
                assets: vec![first.asset, second.asset],
            },
            &mut exported,
        )
        .unwrap();
    assert_eq!(report.assets_written, 2);
    assert_eq!(report.manifest.assets, [first.clone(), second.clone()]);
    assert_eq!(exported.assets.len(), 2);
    assert_eq!(exported.assets[0].0, first);
    assert_eq!(exported.assets[0].1, first_bytes);
    assert_eq!(exported.assets[1].0, second);
    assert_eq!(exported.assets[1].1, second_bytes);

    let cleanup = store
        .cleanup(CleanupAssetsRequest {
            namespace: AssetNamespace("show:main".into()),
            retain: HashSet::from([second.asset]),
            dry_run: false,
        })
        .unwrap();
    assert_eq!(cleanup.detached, vec![first.asset]);
    assert!(cleanup.removed.is_empty());
    assert!(matches!(
        store.availability(first.asset).unwrap(),
        AssetAvailability::Available(_)
    ));

    let tour_cleanup = store
        .cleanup(CleanupAssetsRequest {
            namespace: AssetNamespace("show:tour".into()),
            retain: HashSet::new(),
            dry_run: false,
        })
        .unwrap();
    assert_eq!(tour_cleanup.removed, vec![first.asset]);
    assert!(matches!(
        store.availability(first.asset).unwrap(),
        AssetAvailability::Missing(reference) if reference == first.asset
    ));
    assert!(matches!(
        store.availability(second.asset).unwrap(),
        AssetAvailability::Available(_)
    ));
}

#[test]
fn missing_and_invalid_assets_are_explicit() {
    let store = FakeAssetStore::default();
    let missing = AssetReference {
        id: AssetId(Uuid::from_u128(9)),
        revision: AssetRevision(1),
    };
    assert_eq!(
        store.availability(missing).unwrap(),
        AssetAvailability::Missing(missing)
    );
    let mut source = VecSource::new(b"actual");
    let mut request = import_request(missing.id, "Bad", b"actual");
    request.declared_digest = "wrong".into();
    assert_eq!(
        store.import(request, &mut source).unwrap_err().kind,
        AssetErrorKind::Invalid
    );
}
