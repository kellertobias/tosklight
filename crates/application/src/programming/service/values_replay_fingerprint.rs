use super::super::{
    ProgrammingPreloadValueMutation, ProgrammingPreloadValueTiming,
    ProgrammingPreloadValuesCommand, ProgrammingPreloadValuesRequest, ProgrammingValueMutation,
    ProgrammingValueTiming, ProgrammingValuesCommand, ProgrammingValuesRequest,
};
use light_core::AttributeValue;
use sha2::{Digest, Sha256};

pub(super) type RequestFingerprint = [u8; 32];

pub(super) fn values_request_fingerprint(
    expected_revision: u64,
    request: &ProgrammingValuesRequest,
) -> RequestFingerprint {
    let mut hasher = request_hasher(
        b"programmer-values-v1",
        expected_revision,
        request.expected_capture_mode_revision,
    );
    hash_values_command(&mut hasher, &request.command);
    hasher.finalize().into()
}

pub(super) fn preload_request_fingerprint(
    expected_revision: u64,
    request: &ProgrammingPreloadValuesRequest,
) -> RequestFingerprint {
    let mut hasher = request_hasher(
        b"programmer-preload-values-v1",
        expected_revision,
        request.expected_capture_mode_revision,
    );
    hash_preload_command(&mut hasher, &request.command);
    hasher.finalize().into()
}

fn request_hasher(domain: &[u8], expected_revision: u64, capture_revision: u64) -> Sha256 {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, domain);
    hasher.update(expected_revision.to_le_bytes());
    hasher.update(capture_revision.to_le_bytes());
    hasher
}

fn hash_values_command(hasher: &mut Sha256, command: &ProgrammingValuesCommand) {
    match command {
        ProgrammingValuesCommand::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([0]);
            hash_fixture_set(hasher, fixture_id.0.as_bytes(), &attribute.0, value);
            hash_timing(hasher, timing.fade, timing.fade_millis, timing.delay_millis);
        }
        ProgrammingValuesCommand::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            hasher.update([1]);
            hash_fixture_release(hasher, fixture_id.0.as_bytes(), &attribute.0);
        }
        ProgrammingValuesCommand::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([2]);
            hash_group_set(hasher, group_id, &attribute.0, value);
            hash_timing(hasher, timing.fade, timing.fade_millis, timing.delay_millis);
        }
        ProgrammingValuesCommand::ReleaseGroup {
            group_id,
            attribute,
        } => {
            hasher.update([3]);
            hash_group_release(hasher, group_id, &attribute.0);
        }
        ProgrammingValuesCommand::Batch { mutations } => {
            hasher.update([4]);
            hash_len(hasher, mutations.len());
            mutations
                .iter()
                .for_each(|mutation| hash_value_mutation(hasher, mutation));
        }
        ProgrammingValuesCommand::Clear => hasher.update([5]),
    }
}

fn hash_value_mutation(hasher: &mut Sha256, mutation: &ProgrammingValueMutation) {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([0]);
            hash_fixture_set(hasher, fixture_id.0.as_bytes(), &attribute.0, value);
            hash_value_timing(hasher, *timing);
        }
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            hasher.update([1]);
            hash_fixture_release(hasher, fixture_id.0.as_bytes(), &attribute.0);
        }
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([2]);
            hash_group_set(hasher, group_id, &attribute.0, value);
            hash_value_timing(hasher, *timing);
        }
        ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => {
            hasher.update([3]);
            hash_group_release(hasher, group_id, &attribute.0);
        }
    }
}

fn hash_preload_command(hasher: &mut Sha256, command: &ProgrammingPreloadValuesCommand) {
    match command {
        ProgrammingPreloadValuesCommand::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([0]);
            hash_fixture_set(hasher, fixture_id.0.as_bytes(), &attribute.0, value);
            hash_preload_timing(hasher, *timing);
        }
        ProgrammingPreloadValuesCommand::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            hasher.update([1]);
            hash_fixture_release(hasher, fixture_id.0.as_bytes(), &attribute.0);
        }
        ProgrammingPreloadValuesCommand::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([2]);
            hash_group_set(hasher, group_id, &attribute.0, value);
            hash_preload_timing(hasher, *timing);
        }
        ProgrammingPreloadValuesCommand::ReleaseGroup {
            group_id,
            attribute,
        } => {
            hasher.update([3]);
            hash_group_release(hasher, group_id, &attribute.0);
        }
        ProgrammingPreloadValuesCommand::Batch { mutations } => {
            hasher.update([4]);
            hash_len(hasher, mutations.len());
            mutations
                .iter()
                .for_each(|mutation| hash_preload_mutation(hasher, mutation));
        }
    }
}

fn hash_preload_mutation(hasher: &mut Sha256, mutation: &ProgrammingPreloadValueMutation) {
    match mutation {
        ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([0]);
            hash_fixture_set(hasher, fixture_id.0.as_bytes(), &attribute.0, value);
            hash_preload_timing(hasher, *timing);
        }
        ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            hasher.update([1]);
            hash_fixture_release(hasher, fixture_id.0.as_bytes(), &attribute.0);
        }
        ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            hasher.update([2]);
            hash_group_set(hasher, group_id, &attribute.0, value);
            hash_preload_timing(hasher, *timing);
        }
        ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => {
            hasher.update([3]);
            hash_group_release(hasher, group_id, &attribute.0);
        }
    }
}

fn hash_fixture_set(hasher: &mut Sha256, fixture: &[u8], attribute: &str, value: &AttributeValue) {
    hasher.update(fixture);
    hash_bytes(hasher, attribute.as_bytes());
    hash_attribute_value(hasher, value);
}

fn hash_fixture_release(hasher: &mut Sha256, fixture: &[u8], attribute: &str) {
    hasher.update(fixture);
    hash_bytes(hasher, attribute.as_bytes());
}

fn hash_group_set(hasher: &mut Sha256, group: &str, attribute: &str, value: &AttributeValue) {
    hash_bytes(hasher, group.as_bytes());
    hash_bytes(hasher, attribute.as_bytes());
    hash_attribute_value(hasher, value);
}

fn hash_group_release(hasher: &mut Sha256, group: &str, attribute: &str) {
    hash_bytes(hasher, group.as_bytes());
    hash_bytes(hasher, attribute.as_bytes());
}

fn hash_value_timing(hasher: &mut Sha256, timing: ProgrammingValueTiming) {
    hash_timing(hasher, timing.fade, timing.fade_millis, timing.delay_millis);
}

fn hash_preload_timing(hasher: &mut Sha256, timing: ProgrammingPreloadValueTiming) {
    hash_timing(hasher, timing.fade, timing.fade_millis, timing.delay_millis);
}

fn hash_timing(
    hasher: &mut Sha256,
    fade: bool,
    fade_millis: Option<u64>,
    delay_millis: Option<u64>,
) {
    hasher.update([u8::from(fade)]);
    hash_optional_u64(hasher, fade_millis);
    hash_optional_u64(hasher, delay_millis);
}

fn hash_optional_u64(hasher: &mut Sha256, value: Option<u64>) {
    hasher.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        hasher.update(value.to_le_bytes());
    }
}

fn hash_attribute_value(hasher: &mut Sha256, value: &AttributeValue) {
    match value {
        AttributeValue::Normalized(value) => {
            hasher.update([0]);
            hash_f32(hasher, *value);
        }
        AttributeValue::Spread(values) => {
            hasher.update([1]);
            hash_len(hasher, values.len());
            values.iter().for_each(|value| hash_f32(hasher, *value));
        }
        AttributeValue::Discrete(value) => {
            hasher.update([2]);
            hash_bytes(hasher, value.as_bytes());
        }
        AttributeValue::ColorXyz(value) => {
            hasher.update([3]);
            hash_f32(hasher, value.x);
            hash_f32(hasher, value.y);
            hash_f32(hasher, value.z);
        }
        AttributeValue::RawDmx(value) => hasher.update([4, *value]),
        AttributeValue::RawDmxExact(value) => {
            hasher.update([5]);
            hasher.update(value.to_le_bytes());
        }
    }
}

fn hash_f32(hasher: &mut Sha256, value: f32) {
    let bits = if value == 0.0 { 0 } else { value.to_bits() };
    hasher.update(bits.to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, value: &[u8]) {
    hash_len(hasher, value.len());
    hasher.update(value);
}

fn hash_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeKey, FixtureId};

    #[test]
    fn fingerprints_are_fixed_size_and_structural() {
        let fixture_id = FixtureId::new();
        let request = |value| ProgrammingValuesRequest {
            expected_capture_mode_revision: 7,
            command: ProgrammingValuesCommand::SetFixture {
                fixture_id,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(value),
                timing: Default::default(),
            },
        };
        let first = values_request_fingerprint(3, &request(0.2));
        assert_eq!(first.len(), 32);
        assert_eq!(first, values_request_fingerprint(3, &request(0.2)));
        assert_ne!(first, values_request_fingerprint(3, &request(0.3)));
        assert_ne!(first, values_request_fingerprint(4, &request(0.2)));
    }
}
