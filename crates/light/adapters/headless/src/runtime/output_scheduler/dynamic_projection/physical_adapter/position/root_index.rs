//! Physical root/head ownership only. This index contains no Current, captured programs,
//! protected membership, geometry, fitted values or accepted continuity.
use super::*;

struct Entry {
    fixtures: Arc<Vec<PatchedFixture>>,
    roots: Arc<BTreeMap<Uuid, usize>>,
}

/// Retain at most one immutable fixtures collection and its exact index. Captures keep their
/// own map Arc when the adapter moves to another collection; no pointer-only identity survives.
#[derive(Default)]
pub(super) struct RootIndexCache(parking_lot::Mutex<Option<Entry>>);

impl RootIndexCache {
    pub(super) fn get(&self, fixtures: &Arc<Vec<PatchedFixture>>) -> Arc<BTreeMap<Uuid, usize>> {
        let mut entry = self.0.lock();
        if let Some(cached) = entry.as_ref()
            && Arc::ptr_eq(&cached.fixtures, fixtures)
        {
            return Arc::clone(&cached.roots);
        }
        let roots = Arc::new(build(fixtures));
        *entry = Some(Entry {
            fixtures: Arc::clone(fixtures),
            roots: Arc::clone(&roots),
        });
        roots
    }
}

fn build(fixtures: &[PatchedFixture]) -> BTreeMap<Uuid, usize> {
    let mut roots = BTreeMap::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        roots.insert(fixture.fixture_id.0, index);
        for head in &fixture.logical_heads {
            roots.insert(head.fixture_id.0, index);
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::super::tests::{moving_head, patched};
    use super::*;
    use light_fixture::PatchedHead;

    fn fixture(id: u128, heads: &[u128]) -> PatchedFixture {
        let profile = moving_head();
        let mut fixture = patched(&profile, FixtureId(Uuid::from_u128(id)), 1);
        fixture.logical_heads = heads
            .iter()
            .map(|head| PatchedHead {
                fixture_id: FixtureId(Uuid::from_u128(*head)),
                profile_head_id: Some(profile.modes[0].heads[0].id),
                head_index: fixture.definition.heads[0].index,
            })
            .collect();
        fixture
    }

    // The original per-observer loop, kept independently as an overwrite/order oracle.
    fn original(fixtures: &[PatchedFixture]) -> BTreeMap<Uuid, usize> {
        let mut result = BTreeMap::new();
        for (index, fixture) in fixtures.iter().enumerate() {
            result.insert(fixture.fixture_id.0, index);
            for head in &fixture.logical_heads {
                result.insert(head.fixture_id.0, index);
            }
        }
        result
    }

    #[test]
    fn same_exact_fixtures_reuse_one_root_index_arc() {
        let fixtures = Arc::new(vec![fixture(1, &[11]), fixture(2, &[21])]);
        let cache = RootIndexCache::default();
        let first = cache.get(&fixtures);
        let next_capture = cache.get(&Arc::clone(&fixtures));
        assert!(
            Arc::ptr_eq(&first, &next_capture),
            "unchanged immutable fixtures must reuse the compiled root index"
        );
        assert_eq!(*first, original(&fixtures));
    }

    #[test]
    fn replacing_reordering_and_removing_fixtures_rebuilds_without_changing_old_capture() {
        let fixtures = Arc::new(vec![fixture(1, &[11]), fixture(2, &[21])]);
        let cache = RootIndexCache::default();
        let old_capture = cache.get(&fixtures);
        let equal_replacement = Arc::new(fixtures.as_ref().clone());
        let replaced = cache.get(&equal_replacement);
        assert!(
            !Arc::ptr_eq(&old_capture, &replaced),
            "equal contents are not the same immutable collection"
        );
        assert_eq!(*replaced, original(&equal_replacement));
        let reordered = Arc::new(vec![fixture(2, &[21]), fixture(1, &[11])]);
        let current = cache.get(&reordered);
        assert_eq!(current[&Uuid::from_u128(1)], 1);
        assert_eq!(*current, original(&reordered));
        let removed = Arc::new(vec![fixture(2, &[])]);
        let current = cache.get(&removed);
        assert_eq!(*current, original(&removed));
        assert!(!current.contains_key(&Uuid::from_u128(11)));
        assert!(!current.contains_key(&Uuid::from_u128(1)));
        assert_eq!(*old_capture, original(&fixtures));
        assert_eq!(old_capture[&Uuid::from_u128(1)], 0);
        assert_eq!(old_capture[&Uuid::from_u128(21)], 1);
        assert!(cache.get(&Arc::new(Vec::new())).is_empty());
    }

    #[test]
    fn retained_collection_forces_copy_on_write_and_new_head_ids_are_not_stale() {
        let mut fixtures = Arc::new(vec![fixture(1, &[11])]);
        let cache = RootIndexCache::default();
        let original = cache.get(&fixtures);
        assert_eq!(
            Arc::strong_count(&fixtures),
            2,
            "cache must own the exact collection, not only its pointer"
        );
        Arc::make_mut(&mut fixtures)[0].logical_heads[0].fixture_id =
            FixtureId(Uuid::from_u128(12));
        let edited = cache.get(&fixtures);
        assert!(!Arc::ptr_eq(&original, &edited));
        assert_eq!(*edited, self::original(&fixtures));
        assert!(edited.contains_key(&Uuid::from_u128(12)));
        assert!(!edited.contains_key(&Uuid::from_u128(11)));
        assert!(original.contains_key(&Uuid::from_u128(11)));
        assert!(!original.contains_key(&Uuid::from_u128(12)));
    }

    #[test]
    fn duplicate_uuid_overwrites_and_roots_without_dmx_preserve_original_order() {
        let mut root_without_dmx = fixture(1, &[11]);
        root_without_dmx.universe = None;
        root_without_dmx.address = None;
        root_without_dmx.split_patches.clear();
        let mut unheaded_root = fixture(3, &[]);
        unheaded_root.universe = None;
        unheaded_root.address = None;
        unheaded_root.split_patches.clear();
        let fixtures = Arc::new(vec![
            root_without_dmx,
            fixture(2, &[1, 11]),
            fixture(2, &[22]),
            unheaded_root,
        ]);
        let result = RootIndexCache::default().get(&fixtures);
        assert_eq!(*result, original(&fixtures));
        assert_eq!(
            result[&Uuid::from_u128(1)],
            1,
            "later head overwrites an earlier root"
        );
        assert_eq!(result[&Uuid::from_u128(11)], 1, "later duplicate head wins");
        assert_eq!(result[&Uuid::from_u128(2)], 2, "later duplicate root wins");
        assert_eq!(result[&Uuid::from_u128(22)], 2);
        assert_eq!(
            result[&Uuid::from_u128(3)],
            3,
            "an unheaded unpatched root remains indexed"
        );
    }
}
