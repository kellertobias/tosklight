//! Retained pending candidates for observational Color Release. This is desk editing state, not
//! Cue/Preset content. Live and already active candidates remain in their original source lanes;
//! only candidates actually removed from the pending lane need a retained copy.
use crate::{GroupProgrammerValue, ProgrammerState};
use light_core::{AttributeKey, AttributeValue, FixtureId, TimedValue, programming::*};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, DynamicTrackKey};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleasedPreloadColors {
    pub fixtures: HashMap<FixtureId, ReleasedPreloadFixtureColor>,
    pub groups: HashMap<String, GroupProgrammerValue>,
}

/// Keep both input lanes and their original stamps. A fixed complete Color and an ordinary
/// pending assignment can coexist; observer arbitration must decide which one actually won.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleasedPreloadFixtureColor {
    pub value: Option<TimedValue>,
    pub fixed: Option<DynamicAddressValue>,
    /// Typed whole/component holds retain complete source intent independently. Keep the
    /// legacy fixed field readable for existing editing-state snapshots.
    pub fixed_components: Vec<DynamicAddressValue>,
}

impl ReleasedPreloadColors {
    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty() && self.groups.is_empty()
    }

    pub(crate) fn required_contract(&self) -> u16 {
        self.fixtures
            .values()
            .flat_map(|candidate| {
                candidate
                    .value
                    .iter()
                    .map(|value| value.value.required_programming_contract())
                    .chain(
                        candidate
                            .fixed
                            .iter()
                            .chain(candidate.fixed_components.iter())
                            .map(|value| value.value.required_programming_contract()),
                    )
            })
            .chain(
                self.groups
                    .values()
                    .map(|value| value.value.required_programming_contract()),
            )
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn validate(
        &self,
        releases: &[DynamicAddressValue],
        groups: &[crate::GroupReleaseProgrammerValue],
    ) -> Result<(), IntentError> {
        let color = AttributeKey("color".into());
        let group_releases = groups
            .iter()
            .filter(|v| v.attribute == color)
            .map(|v| {
                (
                    v.group_id.as_str(),
                    (v.programmer_order, v.changed_at_millis),
                )
            })
            .collect::<HashMap<_, _>>();
        for (fixture, candidate) in &self.fixtures {
            if candidate.value.is_none()
                && candidate.fixed.is_none()
                && candidate.fixed_components.is_empty()
            {
                return Err(IntentError(
                    "retained Color candidate requires its pending Release".into(),
                ));
            }
            if let Some(value) = &candidate.value {
                let release =
                    matching_release(releases, *fixture, None).ok_or_else(missing_release)?;
                validate_released_stamp(
                    (release.programmer_order, release.changed_at_millis),
                    value.programmer_order,
                    value.changed_at,
                )?;
                if value.fixture_id != *fixture
                    || value.attribute != color
                    || !complete_color(&value.value)
                {
                    return Err(IntentError(
                        "retained Color candidate has a different owner".into(),
                    ));
                }
                validate_programming_entries(
                    ProgrammingValueScope::Fixture,
                    [(&color, &value.value)],
                )?;
            }
            let mut tracks = std::collections::HashSet::new();
            for (value, typed) in candidate
                .fixed
                .iter()
                .map(|value| (value, false))
                .chain(candidate.fixed_components.iter().map(|value| (value, true)))
            {
                let release = matching_release(releases, *fixture, Some(value.value.track_key()))
                    .ok_or_else(missing_release)?;
                validate_released_stamp(
                    (release.programmer_order, release.changed_at_millis),
                    value.programmer_order,
                    retained_time(value.changed_at_millis)?,
                )?;
                let payload = match (&value.value, typed) {
                    (DynamicSemanticValue::Static { value, .. }, false) => value,
                    (DynamicSemanticValue::ProgrammingFixAt { mask, .. }, true) => {
                        mask.validate()?;
                        &mask.family
                    }
                    _ => {
                        return Err(IntentError(
                            "retained fixed Color must contain a complete captured family".into(),
                        ));
                    }
                };
                if value.fixture_id != *fixture
                    || value.attribute != color
                    || !complete_color(payload)
                {
                    return Err(IntentError(
                        "retained fixed Color has a different owner".into(),
                    ));
                }
                value.value.validate_programming_at(&color)?;
                if !tracks.insert(value.value.track_key()) {
                    return Err(IntentError("duplicate retained Color hold track".into()));
                }
                validate_programming_entries(ProgrammingValueScope::Fixture, [(&color, payload)])?;
            }
        }
        for (group, value) in &self.groups {
            if !group_releases.contains_key(group.as_str()) || !complete_color(&value.value) {
                return Err(IntentError(
                    "retained Group Color requires its matching pending Release".into(),
                ));
            }
            validate_released_stamp(
                group_releases[group.as_str()],
                value.programmer_order,
                value.changed_at,
            )?;
            validate_programming_entries(
                ProgrammingValueScope::LiveGroup,
                [(&color, &value.value)],
            )?;
        }
        Ok(())
    }
}

fn missing_release() -> IntentError {
    IntentError("retained Color candidate requires its matching pending Release".into())
}

/// None is the ordinary pending lane: only owner Release removes that lane. Some selects an
/// exact hold track, which can be removed by its component Release or by owner Release.
fn matching_release(
    releases: &[DynamicAddressValue],
    fixture: FixtureId,
    track: Option<DynamicTrackKey>,
) -> Option<&DynamicAddressValue> {
    releases
        .iter()
        .filter(|release| {
            release.fixture_id == fixture
                && release.attribute.0.as_ref() == "color"
                && (matches!(release.value, DynamicSemanticValue::Release)
                    || matches!(
                        release.value,
                        DynamicSemanticValue::ProgrammingRelease { .. }
                    ) && track == Some(release.value.track_key()))
        })
        .max_by(|a, b| {
            if a.programmer_order > 0 && b.programmer_order > 0 {
                a.programmer_order.cmp(&b.programmer_order)
            } else {
                (a.changed_at_millis, a.programmer_order)
                    .cmp(&(b.changed_at_millis, b.programmer_order))
            }
        })
}

fn complete_color(value: &AttributeValue) -> bool {
    matches!(value, AttributeValue::ColorXyz(_))
        || value.programming_owner() == Some(ProgrammingOwner::Color)
}

fn retained_time(millis: u64) -> Result<chrono::DateTime<chrono::Utc>, IntentError> {
    i64::try_from(millis)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .ok_or_else(|| IntentError("retained Color has an invalid edit timestamp".into()))
}

fn validate_released_stamp(
    (order, millis): (u64, u64),
    programmer_order: u64,
    changed_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), IntentError> {
    if !(light_core::ProgrammerEditStamp {
        changed_at: retained_time(millis)?,
        programmer_order: order,
    })
    .supersedes(changed_at, programmer_order)
    {
        return Err(IntentError(
            "retained Color must precede its Release".into(),
        ));
    }
    Ok(())
}

impl ProgrammerState {
    pub(crate) fn retain_released_fixture_color(
        &mut self,
        fixture: FixtureId,
        attribute: &AttributeKey,
        include_static: bool,
    ) {
        self.retain_released_fixture_color_for(
            fixture,
            attribute,
            include_static,
            &DynamicSemanticValue::Release,
        );
    }

    pub(crate) fn retain_released_fixture_color_for(
        &mut self,
        fixture: FixtureId,
        attribute: &AttributeKey,
        include_static: bool,
        release: &DynamicSemanticValue,
    ) {
        if attribute.0.as_ref() != "color" {
            return;
        }
        let value = include_static
            .then(|| {
                self.preload_pending
                    .iter()
                    .find(|v| {
                        v.fixture_id == fixture
                            && v.attribute == *attribute
                            && complete_color(&v.value)
                    })
                    .cloned()
            })
            .flatten();
        let removed = |v: &&DynamicAddressValue| {
            release.replaces_address(
                fixture,
                attribute,
                v.value.track_key(),
                v.fixture_id,
                &v.attribute,
            )
        };
        let fixed = self.preload_dynamic_pending.iter().filter(removed).find(|v|
            matches!(&v.value, DynamicSemanticValue::Static { value, .. } if complete_color(value))).cloned();
        let fixed_components = self
            .preload_dynamic_pending
            .iter()
            .filter(removed)
            .filter(|v| matches!(&v.value, DynamicSemanticValue::ProgrammingFixAt { .. }))
            .cloned()
            .collect::<Vec<_>>();
        if value.is_some() || fixed.is_some() || !fixed_components.is_empty() {
            let candidate = Arc::make_mut(&mut self.preload_released_colors)
                .fixtures
                .entry(fixture)
                .or_default();
            if let Some(value) = value {
                candidate.value = Some(value);
            }
            if let Some(fixed) = fixed {
                candidate
                    .fixed_components
                    .retain(|old| old.value.track_key() != fixed.value.track_key());
                candidate.fixed = Some(fixed);
            }
            for fixed in fixed_components {
                if candidate
                    .fixed
                    .as_ref()
                    .is_some_and(|old| old.value.track_key() == fixed.value.track_key())
                {
                    candidate.fixed = None;
                }
                if let Some(old) = candidate
                    .fixed_components
                    .iter_mut()
                    .find(|old| old.value.track_key() == fixed.value.track_key())
                {
                    *old = fixed;
                } else {
                    candidate.fixed_components.push(fixed);
                }
            }
            candidate
                .fixed_components
                .sort_by_key(|value| value.value.track_key().component);
        }
    }

    /// Replacing one hold must not discard candidates belonging to sibling pending Releases.
    /// Off/Go/clear still remove their entire owned scope in the existing lifecycle paths.
    pub(crate) fn prune_released_fixture_colors(&mut self) {
        if self.preload_released_colors.fixtures.is_empty() {
            return;
        }
        let releases = &self.preload_dynamic_pending;
        Arc::make_mut(&mut self.preload_released_colors)
            .fixtures
            .retain(|fixture, candidate| {
                if matching_release(releases, *fixture, None).is_none() {
                    candidate.value = None;
                }
                if candidate.fixed.as_ref().is_some_and(|value| {
                    matching_release(releases, *fixture, Some(value.value.track_key())).is_none()
                }) {
                    candidate.fixed = None;
                }
                candidate.fixed_components.retain(|value| {
                    matching_release(releases, *fixture, Some(value.value.track_key())).is_some()
                });
                candidate.value.is_some()
                    || candidate.fixed.is_some()
                    || !candidate.fixed_components.is_empty()
            });
    }

    pub(crate) fn retain_released_group_color(&mut self, group: &str, attribute: &AttributeKey) {
        if attribute.0.as_ref() != "color" {
            return;
        }
        if let Some(value) = self
            .preload_group_pending
            .get(group)
            .and_then(|attributes| attributes.get(attribute))
            .filter(|value| complete_color(&value.value))
            .cloned()
        {
            Arc::make_mut(&mut self.preload_released_colors)
                .groups
                .insert(group.to_owned(), value);
        }
    }

    pub(crate) fn clear_released_fixture_color(
        &mut self,
        fixture: FixtureId,
        attribute: &AttributeKey,
    ) {
        if attribute.0.as_ref() == "color"
            && self.preload_released_colors.fixtures.contains_key(&fixture)
        {
            Arc::make_mut(&mut self.preload_released_colors)
                .fixtures
                .remove(&fixture);
        }
    }

    pub(crate) fn clear_released_group_color(&mut self, group: &str, attribute: &AttributeKey) {
        if attribute.0.as_ref() == "color"
            && self.preload_released_colors.groups.contains_key(group)
        {
            Arc::make_mut(&mut self.preload_released_colors)
                .groups
                .remove(group);
        }
    }
}
