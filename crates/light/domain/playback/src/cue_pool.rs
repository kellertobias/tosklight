//! Cuelist addresses are an independent namespace. Physical assignments only supply legacy
//! addresses before explicit pool metadata has been authored.
use crate::{CueList, PlaybackDefinition, PlaybackTarget};
use light_core::CueListId;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueListPoolCatalog {
    addresses: BTreeMap<u16, CueListId>,
    canonical: HashMap<CueListId, u16>,
}

pub(crate) fn validate_pool_metadata(list: &CueList) -> Result<(), String> {
    if list
        .pool_number
        .is_some_and(|number| !(1..=1000).contains(&number))
    {
        return Err("Cuelist pool number must be within 1-1000".into());
    }
    if list.pool_number.is_none() && !list.legacy_pool_aliases.is_empty() {
        return Err("Cuelist aliases require a canonical pool number".into());
    }
    let mut aliases = BTreeSet::new();
    for &number in &list.legacy_pool_aliases {
        if !(1..=1000).contains(&number)
            || Some(number) == list.pool_number
            || !aliases.insert(number)
        {
            return Err(
                "Cuelist aliases must be distinct valid addresses other than its canonical number"
                    .into(),
            );
        }
    }
    Ok(())
}

impl CueListPoolCatalog {
    pub fn resolve(&self, number: u16) -> Option<CueListId> {
        self.addresses.get(&number).copied()
    }
    pub fn canonical_number(&self, id: CueListId) -> Option<u16> {
        self.canonical.get(&id).copied()
    }
    pub fn addresses(&self) -> impl Iterator<Item = (u16, CueListId)> + '_ {
        self.addresses.iter().map(|(&number, &id)| (number, id))
    }
    /// Preserve legacy assignment-derived addresses, including aliases. Only truly absent
    /// topology receives the historic enumerate-in-input-order default assignments.
    pub fn build(
        lists: &[CueList],
        playbacks: &[PlaybackDefinition],
        pages_absent: bool,
    ) -> Result<Self, String> {
        let mut result = Self {
            addresses: BTreeMap::new(),
            canonical: HashMap::new(),
        };
        let mut legacy = HashMap::<CueListId, BTreeSet<u16>>::new();
        {
            for playback in playbacks {
                if let PlaybackTarget::CueList { cue_list_id } = playback.target
                    && lists
                        .iter()
                        .any(|list| list.id == cue_list_id && list.pool_number.is_none())
                {
                    legacy
                        .entry(cue_list_id)
                        .or_default()
                        .insert(playback.number);
                }
            }
            if playbacks.is_empty() && pages_absent {
                for (index, list) in lists.iter().take(1000).enumerate() {
                    if list.pool_number.is_none() {
                        legacy.entry(list.id).or_default().insert(index as u16 + 1);
                    }
                }
            }
        }
        let mut identities = BTreeSet::new();
        for list in lists {
            if !identities.insert(list.id.0) {
                return Err("duplicate Cuelist identity".into());
            }
            validate_pool_metadata(list)?;
            if result.canonical.contains_key(&list.id) {
                return Err(format!("duplicate Cuelist identity {:?}", list.id));
            }
            if let Some(number) = list.pool_number {
                result.claim(number, list.id)?;
                result.canonical.insert(list.id, number);
                for &alias in &list.legacy_pool_aliases {
                    result.claim(alias, list.id)?;
                }
            } else if let Some(numbers) = legacy.get(&list.id) {
                for &number in numbers {
                    result.claim(number, list.id)?;
                }
                if let Some(&number) = numbers.first() {
                    result.canonical.insert(list.id, number);
                }
            }
        }
        // Unaddressed lists had no literal numeric contract. Allocate deterministically after
        // reserving every meaningful legacy address, rather than stealing a later alias.
        let mut orphans = lists
            .iter()
            .filter(|list| !result.canonical.contains_key(&list.id))
            .map(|list| list.id)
            .collect::<Vec<_>>();
        orphans.sort_unstable_by_key(|id| id.0);
        for id in orphans {
            let number = (1..=1000)
                .find(|number| !result.addresses.contains_key(number))
                .ok_or("Cuelist pool has no free address")?;
            result.claim(number, id)?;
            result.canonical.insert(id, number);
        }
        Ok(result)
    }
    fn claim(&mut self, number: u16, id: CueListId) -> Result<(), String> {
        if !(1..=1000).contains(&number) {
            return Err("legacy Cuelist address must be within 1-1000".into());
        }
        if let Some(previous) = self.addresses.insert(number, id)
            && previous != id
        {
            return Err(format!(
                "Cuelist address {number} is ambiguous between {previous:?} and {id:?}"
            ));
        }
        Ok(())
    }
    pub fn migrate(&self, list: &mut CueList) -> Result<(), String> {
        let canonical = self
            .canonical_number(list.id)
            .ok_or("Cuelist identity is absent from catalog")?;
        list.pool_number = Some(canonical);
        list.legacy_pool_aliases = self
            .addresses()
            .filter_map(|(number, id)| (id == list.id && number != canonical).then_some(number))
            .collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CueChange, CueRecordingContent};
    use light_core::{AttributeKey, AttributeValue, FixtureId};
    use uuid::Uuid;

    fn list(seed: u128) -> CueList {
        CueList::new_recording(
            CueListId(Uuid::from_u128(seed)),
            "test",
            CueRecordingContent {
                changes: vec![CueChange::set(
                    FixtureId(Uuid::from_u128(99)),
                    AttributeKey("intensity".into()),
                    AttributeValue::Normalized(0.5),
                )],
                ..Default::default()
            },
            None,
            false,
            false,
        )
        .unwrap()
        .cue_list
    }
    #[test]
    fn legacy_duplicate_assignments_become_canonical_and_explicit_aliases() {
        let mut list = list(1);
        let playbacks = [
            PlaybackDefinition::new_cue_list(190, "later", list.id),
            PlaybackDefinition::new_cue_list(101, "first", list.id),
        ];
        let catalog = CueListPoolCatalog::build(&[list.clone()], &playbacks, false).unwrap();
        assert_eq!(catalog.canonical_number(list.id), Some(101));
        assert_eq!(catalog.resolve(190), Some(list.id));
        catalog.migrate(&mut list).unwrap();
        assert_eq!(list.pool_number, Some(101));
        assert_eq!(list.legacy_pool_aliases, [190]);
        // Moving/replacing physical assignments cannot reinterpret migrated addresses.
        let new_catalog = CueListPoolCatalog::build(&[list.clone()], &[], false).unwrap();
        assert_eq!(catalog, new_catalog);
        assert_eq!(list.required_programming_contract(), 3);
    }
    #[test]
    fn ambiguous_legacy_address_is_rejected_instead_of_silently_rebound() {
        let a = list(1);
        let b = list(2);
        let playbacks = [
            PlaybackDefinition::new_cue_list(101, "a", a.id),
            PlaybackDefinition::new_cue_list(101, "b", b.id),
        ];
        assert!(
            CueListPoolCatalog::build(&[a, b], &playbacks, false)
                .unwrap_err()
                .contains("ambiguous")
        );
    }
    #[test]
    fn absent_topology_preserves_historic_input_order_but_explicit_empty_pages_do_not() {
        let lists = [list(2), list(1)];
        let absent = CueListPoolCatalog::build(&lists, &[], true).unwrap();
        assert_eq!(absent.resolve(1), Some(lists[0].id));
        let empty = CueListPoolCatalog::build(&lists, &[], false).unwrap();
        assert_eq!(empty.resolve(1), Some(lists[1].id));
    }
    #[test]
    fn independent_number_coexists_with_noncue_playback_and_legacy_body_remains_literal() {
        let mut list = list(1);
        let original = serde_json::to_value(&list).unwrap();
        assert!(original.get("pool_number").is_none());
        assert!(original.get("legacy_pool_aliases").is_none());
        list.pool_number = Some(101);
        let mut special = PlaybackDefinition::new_cue_list(101, "special", list.id);
        special.target = PlaybackTarget::SpeedGroup { group: "A".into() };
        let catalog = CueListPoolCatalog::build(&[list.clone()], &[special], false).unwrap();
        assert_eq!(catalog.resolve(101), Some(list.id));
        assert_eq!(
            list.cues,
            serde_json::from_value::<CueList>(original).unwrap().cues
        );
    }
}
