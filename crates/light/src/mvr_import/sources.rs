use crate::{ActionError, ActionErrorKind};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_mvr::{MvrDocument, MvrFixture};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MVR_SOURCE_ARCHIVE_KIND: &str = "mvr_source_archive";

/// Opaque recovery evidence for unresolved fixtures; never an executable fixture profile.
pub struct RetainedMvrSources<'a> {
    document: &'a MvrDocument,
    references: BTreeMap<String, String>,
    archives: BTreeMap<String, serde_json::Value>,
}

impl<'a> RetainedMvrSources<'a> {
    pub fn new(document: &'a MvrDocument) -> Self {
        Self {
            document,
            references: BTreeMap::new(),
            archives: BTreeMap::new(),
        }
    }

    pub fn unresolved_fixture(
        &mut self,
        fixture: &MvrFixture,
    ) -> Result<serde_json::Value, ActionError> {
        let spec = fixture.gdtf_spec.replace('\\', "/").to_ascii_lowercase();
        let mut candidates = self
            .document
            .files
            .iter()
            .filter(|(path, _)| path.replace('\\', "/").eq_ignore_ascii_case(&spec))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            candidates = self
                .document
                .files
                .iter()
                .filter(|(path, _)| {
                    path.replace('\\', "/")
                        .to_ascii_lowercase()
                        .ends_with(&format!("/{spec}"))
                })
                .collect();
        }
        candidates.sort_by_key(|(path, _)| *path);
        let mut refs = Vec::new();
        for (path, bytes) in candidates {
            let digest = self.references.entry(path.clone()).or_insert_with(|| {
                let digest = format!("sha256:{:x}", Sha256::digest(bytes));
                self.archives.entry(digest.clone()).or_insert_with(|| serde_json::json!({"version":1, "archive_sha256":digest, "data_base64":STANDARD.encode(bytes)}));
                digest
            });
            refs.push(serde_json::json!({"member":path, "archive_ref":digest}));
        }
        let mut value = serde_json::to_value(fixture)
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        value["retained_sources"] = serde_json::Value::Array(refs);
        Ok(value)
    }

    pub fn archives(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.archives.iter().map(|(id, value)| (id.as_str(), value))
    }
}
