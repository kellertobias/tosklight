use crate::{ActionError, ActionErrorKind};
use light_core::{FixtureId, Revision};
use light_fixture::PortablePatchedFixtureRecord;
use light_show::PortableShowDocument;
use std::collections::HashMap;

pub(super) struct StoredFixtureRecord {
    pub(super) object_id: String,
    pub(super) revision: Revision,
    pub(super) record: PortablePatchedFixtureRecord,
}

pub(super) struct StoredFixtureRecords {
    records: Vec<StoredFixtureRecord>,
    by_fixture_id: HashMap<FixtureId, usize>,
}

impl StoredFixtureRecords {
    pub(super) fn load(document: &PortableShowDocument) -> Result<Self, ActionError> {
        let mut records = Vec::new();
        let mut by_fixture_id = HashMap::new();
        for object in document.objects_of_kind("patched_fixture") {
            let record = PortablePatchedFixtureRecord::decode(object.body().clone())
                .map_err(invalid_record)?;
            let fixture_id = record.fixture_id().map_err(invalid_record)?;
            if by_fixture_id.insert(fixture_id, records.len()).is_some() {
                return Err(invalid(format!(
                    "patched fixture identity {} is stored more than once",
                    fixture_id.0
                )));
            }
            records.push(StoredFixtureRecord {
                object_id: object.key().id().to_owned(),
                revision: object.revision(),
                record,
            });
        }
        Ok(Self {
            records,
            by_fixture_id,
        })
    }

    pub(super) fn get(&self, fixture_id: FixtureId) -> Option<&StoredFixtureRecord> {
        self.by_fixture_id
            .get(&fixture_id)
            .map(|index| &self.records[*index])
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &StoredFixtureRecord> {
        self.records.iter()
    }
}

fn invalid_record(error: light_fixture::PortablePatchError) -> ActionError {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
