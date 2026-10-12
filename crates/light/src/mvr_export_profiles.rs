//! Request-local sharing after canonical decode and verified source restoration.
use light_core::FixtureId;
use light_fixture::FixtureProfile;
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct NativeProfiles {
    profiles: HashMap<(FixtureId, u32), Vec<(serde_json::Value, Arc<FixtureProfile>)>>,
}

impl NativeProfiles {
    pub(super) fn intern(&mut self, profile: Arc<FixtureProfile>) -> Arc<FixtureProfile> {
        // A revision key narrows the lookup, never establishes identity. Calibration, retained
        // source evidence, geometry, every mode and all canonical fields must match as well.
        let Ok(contents) = serde_json::to_value(profile.as_ref()) else {
            // Sharing is optional; a serialization failure never changes accepted decode behavior.
            return profile;
        };
        let candidates = self
            .profiles
            .entry((profile.id, profile.revision))
            .or_default();
        if let Some((_, existing)) = candidates.iter().find(|(value, _)| *value == contents) {
            return Arc::clone(existing);
        }
        candidates.push((contents, Arc::clone(&profile)));
        profile
    }
}
