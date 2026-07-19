use super::model::RecordRepresentation;
use super::{
    PORTABLE_PATCH_RECORD_SCHEMA_VERSION, PatchedFixturePatch, PatchedFixtureProfileReference,
    PortablePatchError, PortablePatchedFixtureRecord,
};
use super::{
    identity::{IdentityPolicy, validate_new_patch_identities, validate_patch_identities},
    legacy::{retained_definition_fields, write_retained_extensions},
    merge::merge_typed_delta,
};
use crate::{FIXTURE_PROFILE_SCHEMA_VERSION, PatchedFixture};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Deserialize)]
struct StoredReference {
    patch_record_schema: u16,
    profile_id: light_core::FixtureId,
    profile_revision: light_core::Revision,
    mode_id: uuid::Uuid,
}

impl Serialize for PortablePatchedFixtureRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.body.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PortablePatchedFixtureRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let body = Value::deserialize(deserializer)?;
        Self::decode(body).map_err(serde::de::Error::custom)
    }
}

impl PortablePatchedFixtureRecord {
    /// Decodes either a reference-only record or the current inline `PatchedFixture` shape.
    pub fn decode(body: Value) -> Result<Self, PortablePatchError> {
        let object = body.as_object().ok_or_else(|| {
            PortablePatchError::InvalidRecord("record must be a JSON object".into())
        })?;
        let representation = record_representation(object)?;
        validate_record_body(&body, representation)?;
        Ok(Self {
            body,
            representation,
        })
    }

    /// Creates the lean record written by new patch mutations.
    ///
    /// The runtime still carries an expanded definition for the engine. Only its immutable
    /// profile identity and selected mode cross this persistence boundary.
    pub fn from_runtime_fixture(fixture: &PatchedFixture) -> Result<Self, PortablePatchError> {
        validate_runtime_fixture(fixture)?;
        let reference = runtime_profile_reference(fixture)?;
        Self::from_profile_reference(reference, PatchedFixturePatch::from_fixture(fixture))
    }

    /// Creates a new reference-only record without first expanding a runtime definition.
    pub fn from_profile_reference(
        reference: PatchedFixtureProfileReference,
        patch: PatchedFixturePatch,
    ) -> Result<Self, PortablePatchError> {
        validate_new_patch_identities(&patch)?;
        let mut body = object_value(&patch)?;
        body.insert(
            "patch_record_schema".into(),
            Value::from(PORTABLE_PATCH_RECORD_SCHEMA_VERSION),
        );
        write_profile_reference(&mut body, reference)?;
        Self::decode(Value::Object(body))
    }

    pub fn is_legacy_inline(&self) -> bool {
        self.representation == RecordRepresentation::LegacyInline
    }

    /// Exact retained JSON, including fields unknown to this build.
    pub fn body(&self) -> &Value {
        &self.body
    }

    pub fn into_body(self) -> Value {
        self.body
    }

    pub fn profile_reference(
        &self,
    ) -> Result<Option<PatchedFixtureProfileReference>, PortablePatchError> {
        if self.is_legacy_inline() {
            return Ok(None);
        }
        let stored =
            serde_json::from_value::<StoredReference>(self.body.clone()).map_err(invalid_record)?;
        Ok(Some(PatchedFixtureProfileReference {
            profile_id: stored.profile_id,
            profile_revision: stored.profile_revision,
            mode_id: stored.mode_id,
        }))
    }

    /// Selected immutable profile identity for either stored representation.
    pub fn selected_profile_reference(
        &self,
    ) -> Result<Option<PatchedFixtureProfileReference>, PortablePatchError> {
        if !self.is_legacy_inline() {
            return self.profile_reference();
        }
        let fixture = self.legacy_fixture()?;
        if fixture.definition.schema_version != FIXTURE_PROFILE_SCHEMA_VERSION {
            return Ok(None);
        }
        runtime_profile_reference(&fixture).map(Some)
    }

    pub fn patch(&self) -> Result<PatchedFixturePatch, PortablePatchError> {
        if self.is_legacy_inline() {
            let fixture = self.legacy_fixture()?;
            Ok(PatchedFixturePatch::from_fixture(&fixture))
        } else {
            serde_json::from_value(self.body.clone()).map_err(invalid_record)
        }
    }

    /// Applies only changed typed patch fields while retaining untouched raw and unknown data.
    ///
    /// In particular, a legacy inline definition stays byte-for-byte represented in the raw JSON
    /// until an explicit migration replaces that record.
    pub fn update_patch(
        &mut self,
        updated: &PatchedFixturePatch,
    ) -> Result<(), PortablePatchError> {
        self.update_patch_with_policy(updated, IdentityPolicy::Preserve)
    }

    /// Applies an intentional logical-head or multipatch add/remove operation.
    pub fn update_patch_allowing_identity_changes(
        &mut self,
        updated: &PatchedFixturePatch,
    ) -> Result<(), PortablePatchError> {
        self.update_patch_with_policy(updated, IdentityPolicy::AllowChanges)
    }

    fn update_patch_with_policy(
        &mut self,
        updated: &PatchedFixturePatch,
        policy: IdentityPolicy,
    ) -> Result<(), PortablePatchError> {
        let current = self.patch()?;
        validate_patch_identities(&current, updated, policy)?;
        let before = serde_json::to_value(current)?;
        let after = serde_json::to_value(updated)?;
        let mut candidate = self.body.clone();
        merge_typed_delta(&mut candidate, &before, &after);
        let validated = Self::decode(candidate)?;
        self.body = validated.body;
        self.representation = validated.representation;
        Ok(())
    }

    /// Changes an existing lean record's immutable profile revision or selected mode reference.
    pub fn update_profile_reference(
        &mut self,
        updated: PatchedFixtureProfileReference,
    ) -> Result<(), PortablePatchError> {
        if self.is_legacy_inline() {
            return Err(PortablePatchError::InvalidRecord(
                "legacy record must be migrated before its profile reference can change".into(),
            ));
        }
        let mut candidate = self.body.clone();
        write_profile_reference(candidate_object(&mut candidate)?, updated)?;
        let validated = Self::decode(candidate)?;
        self.body = validated.body;
        self.representation = validated.representation;
        Ok(())
    }

    /// Converts one schema-v2 inline record after its exact profile revision has been
    /// materialized at show level.
    pub fn migrate_legacy_to_profile_reference(
        &mut self,
        reference: PatchedFixtureProfileReference,
    ) -> Result<(), PortablePatchError> {
        if !self.is_legacy_inline() {
            return Err(PortablePatchError::InvalidRecord(
                "only a legacy inline record can be migrated".into(),
            ));
        }
        let fixture = self.legacy_fixture()?;
        validate_runtime_fixture(&fixture)?;
        ensure_matching_reference(runtime_profile_reference(&fixture)?, reference)?;
        let extensions = retained_definition_fields(&self.body, &fixture)?;
        let candidate = migrated_reference_body(&self.body, reference, extensions)?;
        let validated = Self::decode(candidate)?;
        self.body = validated.body;
        self.representation = validated.representation;
        Ok(())
    }

    pub(crate) fn legacy_fixture(&self) -> Result<PatchedFixture, PortablePatchError> {
        serde_json::from_value(self.body.clone()).map_err(invalid_record)
    }

    pub(crate) fn legacy_profile_snapshot(&self) -> Result<&Value, PortablePatchError> {
        self.body
            .pointer("/definition/profile_snapshot")
            .filter(|snapshot| !snapshot.is_null())
            .ok_or_else(|| {
                PortablePatchError::InvalidRecord(
                    "schema-v2 legacy record has no inline profile snapshot".into(),
                )
            })
    }
}

fn invalid_record(error: serde_json::Error) -> PortablePatchError {
    PortablePatchError::InvalidRecord(error.to_string())
}

fn validate_record_body(
    body: &Value,
    representation: RecordRepresentation,
) -> Result<(), PortablePatchError> {
    if representation == RecordRepresentation::LegacyInline {
        serde_json::from_value::<PatchedFixture>(body.clone()).map_err(invalid_record)?;
    } else {
        let reference =
            serde_json::from_value::<StoredReference>(body.clone()).map_err(invalid_record)?;
        if reference.patch_record_schema != PORTABLE_PATCH_RECORD_SCHEMA_VERSION {
            return Err(PortablePatchError::UnsupportedRecordSchema(u64::from(
                reference.patch_record_schema,
            )));
        }
        serde_json::from_value::<PatchedFixturePatch>(body.clone()).map_err(invalid_record)?;
    }
    Ok(())
}

fn record_representation(
    object: &Map<String, Value>,
) -> Result<RecordRepresentation, PortablePatchError> {
    let Some(schema) = object.get("patch_record_schema") else {
        return classify_unversioned_record(object);
    };
    let schema = schema.as_u64().ok_or_else(|| {
        PortablePatchError::InvalidRecord("patch_record_schema must be an integer".into())
    })?;
    if schema == u64::from(PORTABLE_PATCH_RECORD_SCHEMA_VERSION) {
        Ok(RecordRepresentation::ProfileReference)
    } else {
        Err(PortablePatchError::UnsupportedRecordSchema(schema))
    }
}

fn classify_unversioned_record(
    object: &Map<String, Value>,
) -> Result<RecordRepresentation, PortablePatchError> {
    if object.contains_key("definition") && contains_reference_fields(object) {
        return Err(PortablePatchError::AmbiguousRepresentation);
    }
    if object.contains_key("definition") {
        Ok(RecordRepresentation::LegacyInline)
    } else {
        Err(PortablePatchError::InvalidRecord(
            "reference record is missing patch_record_schema".into(),
        ))
    }
}

fn contains_reference_fields(object: &Map<String, Value>) -> bool {
    ["profile_id", "profile_revision", "mode_id"]
        .into_iter()
        .any(|field| object.contains_key(field))
}

fn validate_runtime_fixture(fixture: &PatchedFixture) -> Result<(), PortablePatchError> {
    fixture
        .definition
        .validate()
        .map_err(|error| PortablePatchError::InvalidRecord(error.to_string()))?;
    if fixture.definition.schema_version == FIXTURE_PROFILE_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(PortablePatchError::InvalidRecord(
            "new records require a schema-v2 fixture profile".into(),
        ))
    }
}

fn runtime_profile_reference(
    fixture: &PatchedFixture,
) -> Result<PatchedFixtureProfileReference, PortablePatchError> {
    let definition = &fixture.definition;
    Ok(PatchedFixtureProfileReference {
        profile_id: required_identity(definition.profile_id, "profile ID")?,
        profile_revision: definition.revision.into(),
        mode_id: required_identity(definition.mode_id, "mode ID")?,
    })
}

fn required_identity<T>(identity: Option<T>, name: &str) -> Result<T, PortablePatchError> {
    identity.ok_or_else(|| {
        PortablePatchError::InvalidRecord(format!("fixture definition has no {name}"))
    })
}

fn ensure_matching_reference(
    actual: PatchedFixtureProfileReference,
    expected: PatchedFixtureProfileReference,
) -> Result<(), PortablePatchError> {
    if actual == expected {
        Ok(())
    } else {
        Err(PortablePatchError::InvalidRecord(format!(
            "legacy profile reference {actual:?} does not match materialized reference {expected:?}"
        )))
    }
}

fn migrated_reference_body(
    source: &Value,
    reference: PatchedFixtureProfileReference,
    extensions: Vec<super::legacy::RetainedUnknownField>,
) -> Result<Value, PortablePatchError> {
    let mut candidate = source.clone();
    let body = candidate_object(&mut candidate)?;
    body.remove("definition");
    body.insert(
        "patch_record_schema".into(),
        Value::from(PORTABLE_PATCH_RECORD_SCHEMA_VERSION),
    );
    write_profile_reference(body, reference)?;
    write_retained_extensions(body, extensions)?;
    Ok(candidate)
}

fn candidate_object(candidate: &mut Value) -> Result<&mut Map<String, Value>, PortablePatchError> {
    candidate
        .as_object_mut()
        .ok_or_else(|| PortablePatchError::InvalidRecord("record must be an object".into()))
}

fn write_profile_reference(
    body: &mut Map<String, Value>,
    reference: PatchedFixtureProfileReference,
) -> Result<(), PortablePatchError> {
    body.insert(
        "profile_id".into(),
        serde_json::to_value(reference.profile_id)?,
    );
    body.insert(
        "profile_revision".into(),
        Value::from(reference.profile_revision),
    );
    body.insert("mode_id".into(), serde_json::to_value(reference.mode_id)?);
    Ok(())
}

fn object_value<T: serde::Serialize>(value: &T) -> Result<Map<String, Value>, PortablePatchError> {
    serde_json::to_value(value)?
        .as_object()
        .cloned()
        .ok_or_else(|| PortablePatchError::InvalidRecord("record must be an object".into()))
}
