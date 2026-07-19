use super::*;
use uuid::Uuid;

#[test]
fn reordered_profile_heads_keep_their_fixture_identities() {
    let profile_a = Uuid::from_u128(1);
    let profile_b = Uuid::from_u128(2);
    let fixture_a = FixtureId(Uuid::from_u128(101));
    let fixture_b = FixtureId(Uuid::from_u128(102));
    let existing = vec![
        patched_head(profile_a, 0, fixture_a),
        patched_head(profile_b, 1, fixture_b),
    ];

    let reconciled = reconcile_heads(
        &[resolved_head(profile_b, 0), resolved_head(profile_a, 1)],
        existing,
    )
    .unwrap();

    assert_eq!(reconciled[0], patched_head(profile_b, 0, fixture_b));
    assert_eq!(reconciled[1], patched_head(profile_a, 1, fixture_a));
}

#[test]
fn removed_head_identity_is_not_reused_by_a_new_head_at_the_same_index() {
    let removed_profile_head = Uuid::from_u128(1);
    let new_profile_head = Uuid::from_u128(2);
    let removed_fixture_id = FixtureId(Uuid::from_u128(101));

    let reconciled = reconcile_heads(
        &[resolved_head(new_profile_head, 0)],
        vec![patched_head(removed_profile_head, 0, removed_fixture_id)],
    )
    .unwrap();

    assert_eq!(reconciled[0].profile_head_id, Some(new_profile_head));
    assert_ne!(reconciled[0].fixture_id, removed_fixture_id);
}

#[test]
fn legacy_index_match_is_persisted_and_not_reapplied_after_reordering() {
    let migrated_profile_head = Uuid::from_u128(1);
    let new_profile_head = Uuid::from_u128(2);
    let legacy_fixture_id = FixtureId(Uuid::from_u128(101));
    let legacy = PatchedHead {
        profile_head_id: None,
        head_index: 1,
        fixture_id: legacy_fixture_id,
    };

    let migrated =
        reconcile_heads(&[resolved_head(migrated_profile_head, 1)], vec![legacy]).unwrap();
    assert_eq!(
        migrated[0],
        patched_head(migrated_profile_head, 1, legacy_fixture_id)
    );

    let reconciled = reconcile_heads(
        &[
            resolved_head(migrated_profile_head, 0),
            resolved_head(new_profile_head, 1),
        ],
        migrated,
    )
    .unwrap();
    assert_eq!(reconciled[0].fixture_id, legacy_fixture_id);
    assert_eq!(reconciled[0].head_index, 0);
    assert_ne!(reconciled[1].fixture_id, legacy_fixture_id);
}

const fn resolved_head(profile_head_id: Uuid, head_index: u16) -> ResolvedLogicalHead {
    ResolvedLogicalHead {
        profile_head_id,
        head_index,
    }
}

const fn patched_head(
    profile_head_id: Uuid,
    head_index: u16,
    fixture_id: FixtureId,
) -> PatchedHead {
    PatchedHead {
        profile_head_id: Some(profile_head_id),
        head_index,
        fixture_id,
    }
}
