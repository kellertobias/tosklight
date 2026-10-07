//! Immutable ORIGINAL native Color models owned by one captured show generation.
//! Portable profile decoding and model compilation occur only at the cold compiler boundary.
//! A missing or unsupported original never resolves through the current patch or fixture library.

use light_core::{
    FixtureId, NativeColorIdentity,
    programming::{IntentError, NativeColorEditModel},
};
use light_dynamics::{
    DynamicNativeModelResolver, NativeColorModelCapability, NativeColorModelUnavailable,
    NativeColorUnavailableReason,
};
use light_fixture::{CompiledNativeColorEditModel, FixtureProfile};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

mod capture;
pub use capture::{CapturedDirectColor, DirectColorObservation};

/// The store digest is the digest of the complete retained raw JSON. It is deliberately
/// separate from NativeColorIdentity::profile_digest, which identifies the typed source model.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NativeColorSourceRevisionKey {
    pub profile_id: FixtureId,
    pub revision: u64,
    pub raw_store_digest: String,
}

type SourceKey = (Uuid, Uuid, Uuid);
type RevisionKey = (FixtureId, u64);

#[derive(Debug)]
struct SourceEntry {
    identity: NativeColorIdentity,
    model: Result<Arc<CompiledNativeColorEditModel>, String>,
}

/// One retained profile revision, including explicit unavailable outcomes. Entries are reused
/// by Arc when an unrelated revision is added, without decoding or recompiling the original.
#[derive(Debug)]
pub struct NativeColorSourceRevision {
    key: NativeColorSourceRevisionKey,
    unavailable: Option<String>,
    sources: HashMap<SourceKey, SourceEntry>,
}

impl NativeColorSourceRevision {
    pub fn key(&self) -> &NativeColorSourceRevisionKey {
        &self.key
    }

    /// Whole-revision decoding/validation failure. Individual unsupported optical paths are
    /// retained separately and reported only when that exact source is requested.
    pub fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }

    pub fn source_identities(&self) -> impl Iterator<Item = &NativeColorIdentity> {
        self.sources.values().map(|entry| &entry.identity)
    }

    fn compile(key: NativeColorSourceRevisionKey, decoded: Result<FixtureProfile, String>) -> Self {
        let mut result = Self {
            key,
            unavailable: None,
            sources: HashMap::new(),
        };
        let profile = match decoded {
            Ok(profile) => profile,
            Err(error) => {
                result.unavailable = Some(error);
                return result;
            }
        };
        if profile.id != result.key.profile_id || u64::from(profile.revision) != result.key.revision
        {
            result.unavailable =
                Some("retained native Color profile does not match its revision key".into());
            return result;
        }
        if let Err(error) = profile.validate() {
            result.unavailable = Some(error.to_string());
            return result;
        }
        // A mode without an authored model compiles the derived one its identity names.
        for mode in profile
            .modes
            .iter()
            .filter(|mode| profile.native_color_source(mode.id).is_some())
        {
            let sources = match CompiledNativeColorEditModel::compile_mode(&profile, mode.id) {
                Ok(sources) => sources
                    .into_iter()
                    .map(|(identity, model)| (identity, model.map_err(|error| error.to_string())))
                    .collect::<Vec<_>>(),
                Err(error) => {
                    let identities = match profile.native_color_identities(mode.id) {
                        Ok(identities) => identities,
                        Err(error) => {
                            // Invalid identity evidence cannot authorize any source from this
                            // revision. A model-capacity failure, below, is narrower than that.
                            result.sources.clear();
                            result.unavailable = Some(error.to_string());
                            return result;
                        }
                    };
                    let reason = error.to_string();
                    identities
                        .into_iter()
                        .map(|identity| (identity, Err(reason.clone())))
                        .collect()
                }
            };
            for (identity, model) in sources {
                let model = model.map(Arc::new);
                result.sources.insert(
                    (identity.mode_id, identity.head_id, identity.path_id),
                    SourceEntry { identity, model },
                );
            }
        }
        result
    }
}

/// Runtime-only catalogue. Default (including EngineSnapshot deserialization) is explicitly
/// unprepared. An intentionally prepared empty catalogue is a different state.
#[derive(Debug, Default)]
pub struct NativeColorSourceCatalog {
    prepared: bool,
    revisions: HashMap<RevisionKey, Arc<NativeColorSourceRevision>>,
}

impl NativeColorSourceCatalog {
    /// Reuse precedes decoding so callers may pass an expensive raw-JSON decoder by closure.
    /// Bad unused profiles and unsupported models are retained as unavailable entries, not
    /// returned as show-open errors. The trusted caller supplies the immutable store digest.
    pub fn compile_revision(
        key: NativeColorSourceRevisionKey,
        previous: Option<&Self>,
        decode: impl FnOnce() -> Result<FixtureProfile, String>,
    ) -> Arc<NativeColorSourceRevision> {
        if let Some(revision) = previous.and_then(|catalog| catalog.revision(&key)) {
            return Arc::clone(revision);
        }
        Arc::new(NativeColorSourceRevision::compile(key, decode()))
    }

    pub fn from_revisions(
        revisions: impl IntoIterator<Item = Arc<NativeColorSourceRevision>>,
    ) -> Result<Self, IntentError> {
        let mut result = Self {
            prepared: true,
            revisions: HashMap::new(),
        };
        for revision in revisions {
            let key = (revision.key.profile_id, revision.key.revision);
            if let Some(previous) = result.revisions.get(&key) {
                if previous.key != revision.key {
                    return Err(IntentError(
                        "native Color source catalogue contains conflicting immutable revisions"
                            .into(),
                    ));
                }
                continue;
            }
            result.revisions.insert(key, revision);
        }
        Ok(result)
    }

    pub fn is_prepared(&self) -> bool {
        self.prepared
    }

    pub fn revision(
        &self,
        key: &NativeColorSourceRevisionKey,
    ) -> Option<&Arc<NativeColorSourceRevision>> {
        self.revisions
            .get(&(key.profile_id, key.revision))
            .filter(|revision| revision.key == *key)
    }

    pub fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.resolve_capability(source)?.require_available()
    }

    pub fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        source.validate()?;
        let unavailable = |reason, detail: String| {
            Ok(NativeColorModelCapability::Unavailable(
                NativeColorModelUnavailable {
                    source: source.clone(),
                    reason,
                    detail,
                },
            ))
        };
        if !self.prepared {
            return unavailable(
                NativeColorUnavailableReason::CatalogueNotPrepared,
                "native Color source catalogue is not prepared".into(),
            );
        }
        let Some(revision) = self.revisions.get(&(
            FixtureId(source.profile_id),
            u64::from(source.profile_revision),
        )) else {
            return unavailable(
                NativeColorUnavailableReason::MissingRevision,
                "original native Color profile revision is unavailable".into(),
            );
        };
        if let Some(error) = &revision.unavailable {
            return unavailable(
                NativeColorUnavailableReason::UnavailableRevision,
                format!("original native Color profile is unavailable: {error}"),
            );
        }
        let Some(entry) = revision
            .sources
            .get(&(source.mode_id, source.head_id, source.path_id))
        else {
            return unavailable(
                NativeColorUnavailableReason::MissingPath,
                "original native Color optical path is unavailable".into(),
            );
        };
        if entry.identity != *source {
            return unavailable(
                NativeColorUnavailableReason::UnverifiedOriginal,
                "native Color source does not match its exact original identity".into(),
            );
        }
        match &entry.model {
            Ok(model) => Ok(NativeColorModelCapability::Available(
                Arc::clone(model) as Arc<dyn NativeColorEditModel + Send + Sync>
            )),
            Err(error) => unavailable(
                NativeColorUnavailableReason::UnsupportedModel,
                format!("original native Color model is unavailable: {error}"),
            ),
        }
    }
}

impl DynamicNativeModelResolver for NativeColorSourceCatalog {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        NativeColorSourceCatalog::resolve(self, source)
    }

    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        NativeColorSourceCatalog::resolve_capability(self, source)
    }
}

#[cfg(test)]
mod tests;
