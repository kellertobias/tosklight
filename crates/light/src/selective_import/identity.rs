use light_core::{FixtureId, ShowId};
use light_show::PortableShowObjectKey;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// Deterministic, indexed identity allocation for one import plan.
pub(super) struct IdentityAllocator {
    source_show: ShowId,
    target_show: ShowId,
    occupied_keys: BTreeSet<PortableShowObjectKey>,
    occupied_values: BTreeSet<String>,
    next_numeric: BTreeMap<String, Option<u64>>,
    next_prefixed: BTreeMap<(String, String), Option<u64>>,
}

impl IdentityAllocator {
    pub fn new(
        source_show: ShowId,
        target_show: ShowId,
        keys: impl IntoIterator<Item = PortableShowObjectKey>,
        identity_values: impl IntoIterator<Item = String>,
    ) -> Self {
        let occupied_keys = keys.into_iter().collect::<BTreeSet<_>>();
        let mut next_numeric = BTreeMap::<String, Option<u64>>::new();
        let mut next_prefixed = BTreeMap::<(String, String), Option<u64>>::new();
        for key in &occupied_keys {
            if let Ok(number) = key.id().parse::<u64>() {
                bump_next(
                    next_numeric.entry(key.kind().to_owned()).or_insert(Some(1)),
                    number,
                );
            }
            if let Some((prefix, number)) = numeric_suffix(key.id()) {
                bump_next(
                    next_prefixed
                        .entry((key.kind().to_owned(), prefix.to_owned()))
                        .or_insert(Some(1)),
                    number,
                );
            }
        }
        let mut occupied_values = identity_values.into_iter().collect::<BTreeSet<_>>();
        occupied_values.extend(occupied_keys.iter().map(|key| key.id().to_owned()));
        Self {
            source_show,
            target_show,
            occupied_keys,
            occupied_values,
            next_numeric,
            next_prefixed,
        }
    }

    pub fn duplicate_key(
        &mut self,
        source: &PortableShowObjectKey,
    ) -> Result<PortableShowObjectKey, String> {
        let key = if Uuid::parse_str(source.id()).is_ok() {
            self.first_available_key(source, "object", |attempt| {
                derived_uuid(
                    self.source_show,
                    self.target_show,
                    source,
                    "object",
                    attempt,
                )
                .to_string()
            })?
        } else if source.id().parse::<u64>().is_ok() {
            let occupied_keys = &self.occupied_keys;
            let occupied_values = &self.occupied_values;
            let next = self
                .next_numeric
                .entry(source.kind().to_owned())
                .or_insert(Some(1));
            numeric_key(source.kind(), next, occupied_keys, occupied_values)?
        } else if let Some((prefix, _)) = numeric_suffix(source.id()) {
            let occupied_keys = &self.occupied_keys;
            let occupied_values = &self.occupied_values;
            let next = self
                .next_prefixed
                .entry((source.kind().to_owned(), prefix.to_owned()))
                .or_insert(Some(1));
            prefixed_key(source.kind(), prefix, next, occupied_keys, occupied_values)?
        } else {
            self.first_available_key(source, "object", |attempt| {
                let hash = stable_hash(
                    self.source_show,
                    self.target_show,
                    source,
                    "object",
                    attempt,
                );
                if attempt == 0 {
                    format!("{}~import-{hash:08x}", source.id(), hash = hash as u32)
                } else {
                    format!(
                        "{}~import-{hash:08x}-{attempt}",
                        source.id(),
                        hash = hash as u32
                    )
                }
            })?
        };
        self.occupied_values.insert(key.id().to_owned());
        self.occupied_keys.insert(key.clone());
        Ok(key)
    }

    pub fn nested_uuid(
        &mut self,
        owner: &PortableShowObjectKey,
        slot: &str,
    ) -> Result<String, String> {
        for attempt in 0..=u32::MAX {
            let value =
                derived_uuid(self.source_show, self.target_show, owner, slot, attempt).to_string();
            if self.occupied_values.insert(value.clone()) {
                return Ok(value);
            }
        }
        Err(format!(
            "identity space exhausted for {}/{slot}",
            owner.id()
        ))
    }

    pub fn profile_id(&mut self, source: FixtureId) -> Result<FixtureId, String> {
        let key = PortableShowObjectKey::new("fixture_profile", source.0.to_string());
        self.nested_uuid(&key, "profile").and_then(|value| {
            Uuid::parse_str(&value)
                .map(FixtureId)
                .map_err(|error| error.to_string())
        })
    }

    fn first_available_key(
        &self,
        source: &PortableShowObjectKey,
        slot: &str,
        candidate: impl Fn(u32) -> String,
    ) -> Result<PortableShowObjectKey, String> {
        for attempt in 0..=u32::MAX {
            let key = PortableShowObjectKey::new(source.kind(), candidate(attempt));
            if key_is_available(&key, &self.occupied_keys, &self.occupied_values) {
                return Ok(key);
            }
        }
        Err(format!(
            "identity space exhausted for {}/{} ({slot})",
            source.kind(),
            source.id()
        ))
    }
}

fn bump_next(next: &mut Option<u64>, occupied: u64) {
    if next.is_some_and(|candidate| candidate <= occupied) {
        *next = occupied.checked_add(1);
    }
}

fn numeric_key(
    kind: &str,
    next: &mut Option<u64>,
    occupied_keys: &BTreeSet<PortableShowObjectKey>,
    occupied_values: &BTreeSet<String>,
) -> Result<PortableShowObjectKey, String> {
    loop {
        let value = next.ok_or_else(|| format!("numeric identity space exhausted for {kind}"))?;
        *next = value.checked_add(1);
        let key = PortableShowObjectKey::new(kind, value.to_string());
        if key_is_available(&key, occupied_keys, occupied_values) {
            return Ok(key);
        }
    }
}

fn prefixed_key(
    kind: &str,
    prefix: &str,
    next: &mut Option<u64>,
    occupied_keys: &BTreeSet<PortableShowObjectKey>,
    occupied_values: &BTreeSet<String>,
) -> Result<PortableShowObjectKey, String> {
    loop {
        let value =
            next.ok_or_else(|| format!("numeric identity space exhausted for {kind}/{prefix}"))?;
        *next = value.checked_add(1);
        let key = PortableShowObjectKey::new(kind, format!("{prefix}.{value}"));
        if key_is_available(&key, occupied_keys, occupied_values) {
            return Ok(key);
        }
    }
}

fn key_is_available(
    key: &PortableShowObjectKey,
    occupied_keys: &BTreeSet<PortableShowObjectKey>,
    occupied_values: &BTreeSet<String>,
) -> bool {
    !occupied_keys.contains(key) && !occupied_values.contains(key.id())
}

fn numeric_suffix(value: &str) -> Option<(&str, u64)> {
    let (prefix, suffix) = value.rsplit_once('.')?;
    (!prefix.is_empty())
        .then(|| suffix.parse::<u64>().ok().map(|number| (prefix, number)))
        .flatten()
}

fn derived_uuid(
    source_show: ShowId,
    target_show: ShowId,
    source: &PortableShowObjectKey,
    slot: &str,
    attempt: u32,
) -> Uuid {
    let high = stable_hash_with_seed(source_show, target_show, source, slot, attempt, FNV_OFFSET);
    let low = stable_hash_with_seed(
        source_show,
        target_show,
        source,
        slot,
        attempt,
        FNV_OFFSET ^ FNV_PRIME,
    );
    let mut bytes = ((u128::from(high) << 64) | u128::from(low)).to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn stable_hash(
    source_show: ShowId,
    target_show: ShowId,
    source: &PortableShowObjectKey,
    slot: &str,
    attempt: u32,
) -> u64 {
    stable_hash_with_seed(source_show, target_show, source, slot, attempt, FNV_OFFSET)
}

fn stable_hash_with_seed(
    source_show: ShowId,
    target_show: ShowId,
    source: &PortableShowObjectKey,
    slot: &str,
    attempt: u32,
    seed: u64,
) -> u64 {
    [
        source_show.0.as_bytes().as_slice(),
        target_show.0.as_bytes().as_slice(),
        source.kind().as_bytes(),
        source.id().as_bytes(),
        slot.as_bytes(),
        &attempt.to_be_bytes(),
    ]
    .into_iter()
    .flatten()
    .fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[cfg(test)]
mod tests {
    use super::*;

    fn shows() -> (ShowId, ShowId) {
        (
            ShowId(Uuid::from_u128(1_000)),
            ShowId(Uuid::from_u128(2_000)),
        )
    }

    fn allocator(
        source: &PortableShowObjectKey,
        identities: impl IntoIterator<Item = String>,
    ) -> IdentityAllocator {
        let (source_show, target_show) = shows();
        IdentityAllocator::new(source_show, target_show, [source.clone()], identities)
    }

    #[test]
    fn numeric_and_prefixed_keys_skip_occupied_semantic_values() {
        let numeric = PortableShowObjectKey::new("custom", "1");
        assert_eq!(
            allocator(&numeric, ["2".to_owned()])
                .duplicate_key(&numeric)
                .unwrap()
                .id(),
            "3"
        );

        let prefixed = PortableShowObjectKey::new("preset", "preset.1");
        assert_eq!(
            allocator(&prefixed, ["preset.2".to_owned()])
                .duplicate_key(&prefixed)
                .unwrap()
                .id(),
            "preset.3"
        );
    }

    #[test]
    fn derived_keys_skip_occupied_semantic_values() {
        let (source_show, target_show) = shows();
        let uuid = PortableShowObjectKey::new("fixture", Uuid::from_u128(3_000).to_string());
        let first_uuid = derived_uuid(source_show, target_show, &uuid, "object", 0).to_string();
        let mut uuid_allocator = IdentityAllocator::new(
            source_show,
            target_show,
            [uuid.clone()],
            [first_uuid.clone()],
        );
        assert_ne!(
            uuid_allocator.duplicate_key(&uuid).unwrap().id(),
            first_uuid
        );

        let named = PortableShowObjectKey::new("macro", "named");
        let hash = stable_hash(source_show, target_show, &named, "object", 0);
        let first_named = format!("named~import-{hash:08x}", hash = hash as u32);
        let mut named_allocator = IdentityAllocator::new(
            source_show,
            target_show,
            [named.clone()],
            [first_named.clone()],
        );
        assert_ne!(
            named_allocator.duplicate_key(&named).unwrap().id(),
            first_named
        );
    }
}
