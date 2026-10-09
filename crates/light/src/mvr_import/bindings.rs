//! Canonical source/mode binding shared by Control and Architect. No writes occur here.
use super::plan::{matches_mvr_definition_metadata, mvr_source_spec};
use crate::{ActionError, ActionErrorKind};
use light_core::FixtureId;
use light_fixture::{FixtureDefinition, FixtureProfile};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
/// Preview owns the exact source/mode binding. Applying a token never repeats name matching.
#[derive(Clone, Default)]
pub struct MvrDefinitions {
    pub definitions: HashMap<Uuid, light_fixture::FixtureDefinition>,
    pub new_profiles: Vec<light_fixture::FixtureProfile>,
    pub imported_profiles: HashMap<Uuid, usize>,
    pub warnings: Vec<String>,
}

pub fn bind_mvr_sources(
    document: &light_mvr::MvrDocument,
    installed_profiles: &[FixtureProfile],
    legacy_definitions: &[FixtureDefinition],
    mut resolve_revision: impl FnMut(FixtureId, u32) -> Result<Option<FixtureProfile>, ActionError>,
    unknown_attributes: impl Fn(&FixtureProfile) -> Vec<String>,
) -> Result<MvrDefinitions, ActionError> {
    // Match cheap catalog metadata first. Projection validates and calibrates a full profile;
    // unrelated library entries must not add that work (or errors) to this MVR preview.
    let mut installed_ids = installed_profiles
        .iter()
        .filter(|profile| !profile.modes.is_empty())
        .map(|profile| profile.id)
        .collect::<HashSet<_>>();
    let legacy = legacy_definitions
        .iter()
        .filter(|definition| installed_ids.insert(definition.id))
        .collect::<Vec<_>>();
    let mut installed_projections = HashMap::<(FixtureId, u32, Uuid), FixtureDefinition>::new();
    let native = crate::mvr_export::tosklight_mvr_fixture_metadata(document);
    let mut result = MvrDefinitions::default();
    let mut parsed = HashMap::<String, Result<usize, String>>::new();
    let mut projections = HashMap::<(usize, Uuid), light_fixture::FixtureDefinition>::new();
    for fixture in &document.fixtures {
        if let Some(embedded) = native.get(&fixture.uuid) {
            result
                .definitions
                .insert(fixture.uuid, embedded.fixture.definition.clone());
            continue;
        }
        let member = match source_member(document, &fixture.gdtf_spec) {
            Ok(member) => member,
            Err(message) => {
                result.warnings.push(format!("{}: {message}", fixture.name));
                continue;
            }
        };
        let Some((path, bytes)) = member else {
            let spec = mvr_source_spec(fixture);
            let profiles = installed_profiles
                .iter()
                .flat_map(|profile| {
                    profile.modes.iter().filter_map(|mode| {
                        matches_mvr_definition_metadata(
                            &profile.manufacturer,
                            &profile.short_name,
                            &profile.name,
                            &mode.name,
                            &spec,
                            &fixture.gdtf_mode,
                        )
                        .then_some((profile.id, profile.revision, mode.id))
                    })
                })
                .collect::<Vec<_>>();
            let definitions = legacy
                .iter()
                .copied()
                .filter(|definition| {
                    matches_mvr_definition_metadata(
                        &definition.manufacturer,
                        &definition.model,
                        &definition.name,
                        &definition.mode,
                        &spec,
                        &fixture.gdtf_mode,
                    )
                })
                .collect::<Vec<_>>();
            let key = match (profiles.as_slice(), definitions.as_slice()) {
                ([key], []) => *key,
                ([], [definition]) => {
                    if let (Some(profile_id), Some(mode_id)) =
                        (definition.profile_id, definition.mode_id)
                    {
                        (profile_id, definition.revision, mode_id)
                    } else {
                        result
                            .definitions
                            .insert(fixture.uuid, (*definition).clone());
                        continue;
                    }
                }
                _ => {
                    result.warnings.push(format!(
                        "{}: archive {} is absent and no unique installed profile/mode matches",
                        fixture.name, fixture.gdtf_spec
                    ));
                    continue;
                }
            };
            if let std::collections::hash_map::Entry::Vacant(entry) =
                installed_projections.entry(key)
            {
                // Catalogs may omit source bytes. Retain the authoritative immutable snapshot.
                let Some(profile) = resolve_revision(key.0, key.1)? else {
                    result.warnings.push(format!(
                        "{}: the installed profile revision disappeared during preview; preview again",
                        fixture.name,
                    ));
                    continue;
                };
                if profile.id != key.0 || profile.revision != key.1 {
                    return Err(invalid(
                        "installed MVR profile resolver returned a different revision",
                    ));
                }
                entry.insert(
                    profile
                        .resolved_definition(key.2)
                        .map_err(|error| invalid(error.to_string()))?,
                );
            }
            result
                .definitions
                .insert(fixture.uuid, installed_projections[&key].clone());
            continue;
        };
        let parsed_profile = parsed.entry(path.to_owned()).or_insert_with(|| {
            let mut imported = light_fixture::gdtf::read::import_profile(bytes).map_err(|error| error.to_string())?;
            let unknown = unknown_attributes(&imported.profile);
            if !unknown.is_empty() {
                // A manually mapped library import may supply operator-approved canonical
                // attributes, but only with the exact archive and current source association.
                let digest = &imported.profile.source_gdtf.as_ref().expect("canonical source").archive_sha256;
                let candidates = installed_profiles.iter().filter(|profile| {
                    profile.source_gdtf.as_ref().is_some_and(|source| source.archive_sha256 == *digest && source.matches_profile(profile).unwrap_or(false))
                        && unknown_attributes(profile).is_empty()
                }).collect::<Vec<_>>();
                if let [mapped] = candidates.as_slice() {
                    imported.profile = (*mapped).clone();
                    result.warnings.push(format!("{path}: using the verified source mapping from Fixture Library"));
                } else {
                    return Err(format!("unmapped GDTF attributes {}; import this exact archive in Fixture Library and map them before MVR import (requires one verified mapping)", unknown.join(", ")));
                }
            }
            for diagnostic in imported.diagnostics {
                result.warnings.push(format!("{path}: {}: {}", diagnostic.node, diagnostic.message));
            }
            let index = result.new_profiles.len();
            result.new_profiles.push(imported.profile);
            Ok(index)
        });
        match parsed_profile {
            Ok(index) => {
                let profile = &result.new_profiles[*index];
                // GDTF references are names, not ordinals or manufacturer/model guesses.
                let Some(mode) = profile
                    .modes
                    .iter()
                    .find(|mode| mode.name == fixture.gdtf_mode)
                else {
                    result.warnings.push(format!(
                        "{}: {path} has no mode named {}",
                        fixture.name, fixture.gdtf_mode
                    ));
                    continue;
                };
                let key = (*index, mode.id);
                if let std::collections::hash_map::Entry::Vacant(entry) = projections.entry(key) {
                    entry.insert(
                        profile
                            .resolved_definition(mode.id)
                            .map_err(|error| invalid(error.to_string()))?,
                    );
                }
                let definition = projections[&key].clone();
                if definition.split_footprints().len() > 1 {
                    result.warnings.push(format!("{}: this MVR reader supplies one address; additional DMX breaks remain explicitly unpatched", fixture.name));
                }
                result.definitions.insert(fixture.uuid, definition);
                result.imported_profiles.insert(fixture.uuid, *index);
            }
            Err(message) => result
                .warnings
                .push(format!("{}: {path}: {message}", fixture.name)),
        }
    }
    Ok(result)
}

fn source_member<'a>(
    document: &'a light_mvr::MvrDocument,
    spec: &str,
) -> Result<Option<(&'a str, &'a [u8])>, String> {
    let normalized = spec.replace('\\', "/").to_ascii_lowercase();
    let matching = |suffix: bool| {
        document
            .files
            .iter()
            .filter(|(path, _)| {
                let path = path.replace('\\', "/").to_ascii_lowercase();
                path == normalized || (suffix && path.ends_with(&format!("/{normalized}")))
            })
            .collect::<Vec<_>>()
    };
    let exact = matching(false);
    let candidates = if exact.is_empty() {
        matching(true)
    } else {
        exact
    };
    match candidates.as_slice() {
        [] => Ok(None),
        [(path, bytes)] => Ok(Some((path.as_str(), bytes.as_slice()))),
        _ => Err(format!("archive reference {spec} is ambiguous")),
    }
}
