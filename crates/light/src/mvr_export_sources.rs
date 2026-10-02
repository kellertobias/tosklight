//! Keep potentially large source archives out of every fixture's native MVR metadata.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_fixture::{PatchedFixture, ProfileGdtfSource};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct SourceArchiveWriter {
    pub descriptors: BTreeMap<String, ProfileGdtfSource>,
    // Retain the original shared data URL for content-safe cache checks. Hashes supplied in the
    // profile are not trusted until the bytes have been validated.
    validated: Vec<(ProfileGdtfSource, String)>,
    archives: HashMap<String, (String, Arc<str>)>,
}

fn same_bytes(left: &Arc<str>, right: &Arc<str>) -> bool {
    Arc::ptr_eq(left, right) || left.as_ref() == right.as_ref()
}

impl SourceArchiveWriter {
    pub fn use_standard_member(
        &mut self,
        reference: &str,
        spec: &str,
        document: &mut light_mvr::MvrDocument,
    ) {
        let Some(descriptor) = self.descriptors.get(reference) else {
            return;
        };
        let old = descriptor.archive_asset.to_string();
        if old == spec || !old.starts_with("tosklight/source-") {
            return;
        }
        if document.files.get(&old) != document.files.get(spec) {
            return;
        }
        for descriptor in self.descriptors.values_mut() {
            if descriptor.archive_asset.as_ref() == old {
                descriptor.archive_asset = spec.into();
            }
        }
        for (path, _) in self.archives.values_mut() {
            if *path == old {
                *path = spec.to_owned();
            }
        }
        document.files.remove(&old);
    }

    pub fn detach(
        &mut self,
        fixture: &mut PatchedFixture,
        document: &mut light_mvr::MvrDocument,
        warnings: &mut Vec<String>,
    ) -> Option<String> {
        let profile = fixture.definition.profile_snapshot.as_mut()?;
        let source = profile.source_gdtf.as_ref()?;
        if let Some((_, reference)) = self.validated.iter().find(|(existing, _)| {
            existing.version == source.version
                && existing.archive_sha256 == source.archive_sha256
                && existing.profile_fingerprint == source.profile_fingerprint
                && same_bytes(&existing.archive_asset, &source.archive_asset)
        }) {
            let reference = reference.clone();
            profile.source_gdtf = None;
            return Some(reference);
        }
        let bytes = match source.decoded_archive() {
            Ok(bytes) => bytes,
            Err(error) => {
                // Keep invalid evidence inline for recovery, as older exports did. It cannot be
                // advertised as a validated shared source or used as the standard fixture GDTF.
                warnings.push(format!(
                    "{}: retained GDTF stays inline for recovery: {error}",
                    profile.name
                ));
                return None;
            }
        };
        let path = match self.archives.get(&source.archive_sha256) {
            Some((path, original)) if same_bytes(original, &source.archive_asset) => path.clone(),
            _ => {
                // Reuse the standard fixture's original file when possible. Stale source has a
                // separate ancillary member; generated GDTF must never replace its evidence.
                let existing = document
                    .files
                    .iter()
                    .filter(|(path, data)| {
                        path.to_ascii_lowercase().ends_with(".gdtf") && **data == bytes
                    })
                    .map(|(path, _)| path.clone())
                    .min();
                let path = existing
                    .unwrap_or_else(|| format!("tosklight/source-{}.gdtf", source.archive_sha256));
                document.files.entry(path.clone()).or_insert(bytes);
                self.archives.insert(
                    source.archive_sha256.clone(),
                    (path.clone(), source.archive_asset.clone()),
                );
                path
            }
        };
        let reference = format!("source-{}", self.descriptors.len() + 1);
        let mut descriptor = source.clone();
        descriptor.archive_asset = path.into();
        self.descriptors.insert(reference.clone(), descriptor);
        self.validated.push((source.clone(), reference.clone()));
        profile.source_gdtf = None;
        Some(reference)
    }
}

pub(super) fn read_sources(
    document: &light_mvr::MvrDocument,
    descriptors: BTreeMap<String, ProfileGdtfSource>,
) -> HashMap<String, ProfileGdtfSource> {
    let mut archives: HashMap<(String, String), Arc<str>> = HashMap::new();
    descriptors
        .into_iter()
        .filter_map(|(reference, mut descriptor)| {
            // These are exact archive-map lookups, never filesystem paths or external URLs.
            let path = descriptor.archive_asset.to_string();
            let key = (path.clone(), descriptor.archive_sha256.clone());
            let data_url = match archives.get(&key) {
                Some(archive) => archive.clone(),
                None => {
                    // The shared MVR reader canonicalizes archive member names to lowercase.
                    let bytes = document
                        .files
                        .get(&path)
                        .or_else(|| document.files.get(&path.to_ascii_lowercase()))?;
                    if bytes.len() > 64 * 1024 * 1024 {
                        return None;
                    }
                    format!(
                        "data:application/vnd.gdtf;base64,{}",
                        STANDARD.encode(bytes)
                    )
                    .into()
                }
            };
            descriptor.archive_asset = data_url.clone();
            // Validate the original descriptor, including its association. Never associate against
            // the fixture's current profile: stale and unverified evidence must stay that way.
            descriptor.decoded_archive().ok()?;
            archives.entry(key).or_insert(data_url);
            Some((reference, descriptor))
        })
        .collect()
}
