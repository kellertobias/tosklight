//! Exact programming schema locations. Unknown extension objects are deliberately not scanned.
use super::{
    fixtures::FixtureIdentityCatalog,
    locations::{
        add_fixture_map_keys, add_fixture_value, add_optional_direct_reference, direct_reference,
        value_location,
    },
};
use crate::selective_import::{
    ImportIdentityFormat, ImportObjectDescriptor, ImportProfileKey,
    model::{ImportNativeColorReference, ImportProfileReference},
};
use light_core::{FixtureId, NativeColorIdentity};
use serde_json::Value;

pub(super) struct ProgrammingReferences<'a> {
    pub body: &'a Value,
    pub source: &'a FixtureIdentityCatalog,
    pub target: &'a FixtureIdentityCatalog,
    pub descriptor: &'a mut ImportObjectDescriptor,
}

impl ProgrammingReferences<'_> {
    fn kind(&self, pointer: &str) -> Option<&str> {
        self.body.pointer(pointer)?.get("kind")?.as_str()
    }

    fn indices(&self, pointer: &str) -> std::ops::Range<usize> {
        0..self
            .body
            .pointer(pointer)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }

    fn keys(&self, pointer: &str) -> Vec<String> {
        self.body
            .pointer(pointer)
            .and_then(Value::as_object)
            .map(|values| values.keys().map(|key| escape(key)).collect())
            .unwrap_or_default()
    }

    fn fixture(&mut self, pointer: &str) -> Result<(), String> {
        add_fixture_value(
            self.body,
            pointer,
            pointer.into(),
            self.source,
            self.target,
            self.descriptor,
        )
    }

    fn point(&mut self, pointer: &str) -> Result<(), String> {
        if self.kind(pointer) == Some("point") {
            self.fixture(&format!("{pointer}/point_id"))?;
        }
        Ok(())
    }

    fn native(&mut self, pointer: &str) -> Result<(), String> {
        let source: NativeColorIdentity = serde_json::from_value(
            self.body
                .pointer(pointer)
                .cloned()
                .ok_or_else(|| format!("missing native source at {pointer}"))?,
        )
        .map_err(|error| format!("invalid native source at {pointer}: {error}"))?;
        source.validate().map_err(|error| error.to_string())?;
        self.descriptor
            .profile_references
            .push(ImportProfileReference {
                key: ImportProfileKey {
                    profile_id: FixtureId(source.profile_id),
                    revision: u64::from(source.profile_revision),
                },
                id_locations: vec![],
                inline_profile: None,
            });
        self.descriptor
            .native_color_references
            .push(ImportNativeColorReference {
                source,
                pointer: pointer.into(),
            });
        Ok(())
    }

    /// One AttributeValue. Group wrappers may contain only direct family values.
    pub fn value(&mut self, pointer: &str) -> Result<(), String> {
        self.value_inner(pointer, false)
    }

    fn value_inner(&mut self, pointer: &str, inside_group: bool) -> Result<(), String> {
        match self.kind(pointer) {
            Some("position") => self.point(&format!("{pointer}/value/reference"))?,
            Some("color_program") => match self.kind(&format!("{pointer}/value")) {
                Some("direct") => self.native(&format!("{pointer}/value/recipe/source"))?,
                Some("semantic") => {
                    let wheels = format!("{pointer}/value/intent/wheel_constraints");
                    for index in self.indices(&wheels) {
                        self.native(&format!("{wheels}/{index}/source"))?;
                    }
                }
                _ => {}
            },
            Some("group_family") => {
                if inside_group {
                    return Err("nested Group family is invalid".into());
                }
                self.value_inner(&format!("{pointer}/value/template"), true)?;
                let members = format!("{pointer}/value/members");
                add_fixture_map_keys(
                    self.body,
                    &members,
                    self.source,
                    self.target,
                    self.descriptor,
                )?;
                for key in self.keys(&members) {
                    self.value_inner(&format!("{members}/{key}"), true)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn attribute_map(&mut self, pointer: &str) -> Result<(), String> {
        for key in self.keys(pointer) {
            self.value(&format!("{pointer}/{key}"))?;
        }
        Ok(())
    }

    pub fn preset(&mut self) -> Result<(), String> {
        for scope in ["values", "group_values"] {
            for key in self.keys(&format!("/{scope}")) {
                self.attribute_map(&format!("/{scope}/{key}"))?;
            }
        }
        self.attribute_map("/universal_values")
    }

    fn address(&mut self, pointer: &str) -> Result<(), String> {
        let representation = format!("{pointer}/representation");
        match self.kind(&representation) {
            Some("target") => self.point(&format!("{representation}/reference"))?,
            Some("direct_color") => self.native(&format!("{representation}/source"))?,
            _ => {}
        }
        Ok(())
    }

    fn dynamic_value(&mut self, pointer: &str) -> Result<(), String> {
        if self.kind(pointer) == Some("family") {
            self.value(&format!("{pointer}/value"))?;
        }
        Ok(())
    }

    fn template(&mut self, pointer: &str, fallback: bool) -> Result<(), String> {
        self.value(&format!("{pointer}/universal"))?;
        for (scope, identity) in [("groups", "group_id"), ("fixtures", "fixture_id")] {
            let scope = format!("{pointer}/{scope}");
            for index in self.indices(&scope) {
                let entry = format!("{scope}/{index}");
                let id = format!("{entry}/{identity}");
                if identity == "group_id" {
                    add_optional_direct_reference(self.body, &id, "group", self.descriptor)?;
                } else {
                    self.fixture(&id)?;
                }
                self.value(&format!("{entry}/value"))?;
            }
        }
        let nested = format!("{pointer}/fallback");
        if self.body.pointer(&nested).is_some_and(|v| !v.is_null()) {
            if fallback {
                return Err("retained Preset fallback cannot nest".into());
            }
            self.template(&nested, true)?;
        }
        Ok(())
    }

    fn source_value(&mut self, pointer: &str) -> Result<(), String> {
        match self.kind(pointer) {
            Some("value") => self.dynamic_value(&format!("{pointer}/value"))?,
            Some("preset") => {
                let id_pointer = format!("{pointer}/preset_id");
                let id = self
                    .body
                    .pointer(&id_pointer)
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("invalid typed Preset identity at {id_pointer}"))?;
                let mut reference = direct_reference(
                    "preset",
                    id.into(),
                    value_location(id_pointer, ImportIdentityFormat::Full),
                );
                reference.allow_missing = true;
                self.descriptor.references.push(reference);
                self.address(&format!("{pointer}/address"))?;
                let fallbacks = format!("{pointer}/last_valid_by_target");
                for index in self.indices(&fallbacks) {
                    self.fixture(&format!("{fallbacks}/{index}/target"))?;
                    self.dynamic_value(&format!("{fallbacks}/{index}/value"))?;
                }
                self.template(&format!("{pointer}/retained"), false)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn dynamic(&mut self, pointer: &str) -> Result<(), String> {
        let lanes = format!("{pointer}/lanes");
        for index in self.indices(&lanes) {
            let lane = format!("{lanes}/{index}/programming");
            self.address(&format!("{lane}/address"))?;
            let config = format!("{lane}/configuration/configuration");
            match self
                .body
                .pointer(&format!("{lane}/configuration/mode"))
                .and_then(Value::as_str)
            {
                Some("keyframes") => {
                    let points = format!("{config}/points");
                    for index in self.indices(&points) {
                        self.source_value(&format!("{points}/{index}/source"))?;
                    }
                }
                Some("max_min") => {
                    for key in ["minimum", "maximum"] {
                        self.source_value(&format!("{config}/{key}"))?;
                    }
                }
                Some("middle_amplitude") => {
                    self.source_value(&format!("{config}/middle"))?;
                    self.dynamic_value(&format!("{config}/amplitude"))?;
                }
                _ => {}
            }
        }
        let groups = format!("{pointer}/random_groups");
        for index in self.indices(&groups) {
            for bound in ["low", "high"] {
                self.source_value(&format!("{groups}/{index}/programming_range/{bound}"))?;
            }
        }
        Ok(())
    }

    pub fn dynamic_semantic(&mut self, pointer: &str) -> Result<(), String> {
        match self
            .body
            .pointer(pointer)
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
        {
            Some("static") => self.value(&format!("{pointer}/value"))?,
            Some("programming_fix_at") => {
                self.address(&format!("{pointer}/mask/address"))?;
                self.value(&format!("{pointer}/mask/family"))?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
