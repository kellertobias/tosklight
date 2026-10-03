//! Captured fixed rows have explicit source/address identities, independent of Dynamic clocks.
//! This foundation classifies complete typed masks only. Native model availability is checked
//! by the later compiler; it never causes a valid Direct mask to disappear from this catalogue.
use super::*;
use light_core::{AttributeKey, AttributeValue};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, ProgrammingFamilyFixAt};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in crate::runtime) enum DynamicFixedSource {
    Programmer {
        programmer_id: ProgrammerId,
        lane: DynamicProgrammerSourceLane,
    },
    Cue {
        source: DynamicSequenceSource,
        temporary_kind: Option<DynamicTemporarySourceKind>,
    },
}

impl DynamicFixedSource {
    pub(in crate::runtime) fn validate(self) -> Result<(), IntentError> {
        match self {
            Self::Programmer { programmer_id, .. } => non_nil(programmer_id.0, "Programmer"),
            Self::Cue {
                source,
                temporary_kind,
            } => {
                source.validate()?;
                if source.temporary != temporary_kind.is_some() {
                    return Err(invalid("fixed Cue temporary source and kind disagree"));
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in crate::runtime) enum DynamicFixedStamp {
    Programmer {
        changed_at_millis: u64,
        programmer_order: u64,
    },
    Cue {
        authored_cue_id: Uuid,
        changed_at: DateTime<Utc>,
        transition_ordinal: u64,
    },
}

impl DynamicFixedStamp {
    /// Programmer stamps outside Chrono's domain remain unknown; never substitute frame time.
    pub fn exact_changed_at(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Programmer {
                changed_at_millis, ..
            } => i64::try_from(changed_at_millis)
                .ok()
                .and_then(DateTime::from_timestamp_millis),
            Self::Cue { changed_at, .. } => Some(changed_at),
        }
    }
}

/// The compiler uses the immutable record to obtain the original row and exact stamp. The
/// occurrence is source evidence, never an invented Dynamic instance, controller or lane ID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct CapturedFixedSource {
    pub binding: DynamicSourceBinding,
    pub occurrence_id: DynamicSourceOccurrenceId,
}

/// None means a genuinely different lane: non-fixed rows or untyped legacy attributes. Every
/// typed Direct/Target mask remains captured even if its original model/geometry is unavailable.
pub(in crate::runtime) fn captured_programming_fixed_mask(
    attribute: &AttributeKey,
    value: &DynamicSemanticValue,
) -> Result<Option<ProgrammingFamilyFixAt>, IntentError> {
    let mask = match value {
        DynamicSemanticValue::ProgrammingFixAt { mask, .. } => mask.clone(),
        DynamicSemanticValue::Static { value, .. } => {
            let owner = match value {
                AttributeValue::Position(_) => ProgrammingOwner::Position,
                AttributeValue::ColorProgram(_) => ProgrammingOwner::Color,
                AttributeValue::Zoom(_) => ProgrammingOwner::Zoom,
                AttributeValue::Normalized(_) if *attribute == ProgrammingOwner::Focus.key() => {
                    ProgrammingOwner::Focus
                }
                _ => return Ok(None),
            };
            ProgrammingFamilyFixAt::from_family(owner, None, value.clone())?
        }
        DynamicSemanticValue::FixAt { value, .. }
            if *attribute == ProgrammingOwner::Focus.key() =>
        {
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Focus,
                None,
                AttributeValue::Normalized(*value),
            )?
        }
        _ => return Ok(None),
    };
    mask.validate()?;
    if mask.address.owner().key() != *attribute {
        return Err(invalid("fixed row has a different attribute owner"));
    }
    Ok(Some(mask))
}

pub(super) fn validate_fixed_origin(
    binding: DynamicSourceBinding,
    stamp: &DynamicFixedStamp,
    value: &DynamicSemanticValue,
) -> Result<(), IntentError> {
    let DynamicSourceBinding::Fixed {
        source,
        owner,
        component,
        ..
    } = binding
    else {
        return Err(invalid("fixed origin requires a fixed binding"));
    };
    match (source, stamp) {
        (DynamicFixedSource::Programmer { .. }, DynamicFixedStamp::Programmer { .. }) => {}
        (
            DynamicFixedSource::Cue { .. },
            DynamicFixedStamp::Cue {
                authored_cue_id, ..
            },
        ) => non_nil(*authored_cue_id, "authored Cue")?,
        _ => return Err(invalid("fixed timestamp and source kinds disagree")),
    }
    let mask = captured_programming_fixed_mask(&owner.key(), value)?
        .ok_or_else(|| invalid("fixed record requires a complete typed mask"))?;
    if mask.address.component != component {
        return Err(invalid("fixed record and captured component disagree"));
    }
    Ok(())
}

impl DynamicSourceOrigins {
    /// Reconcile ONLY complete typed fixed rows from the exact frame capture, including disabled
    /// Cue rows. Output masks and master levels are applied later and do not rewrite authorship.
    /// Missing legacy Programmer sidecars stay unattributed: their valid masks still belong to
    /// semantic output. The compiler must traverse the original captured rows, using this result
    /// only for optional occurrence evidence; it must not use this list as its value source.
    /// All validation precedes mutation;
    /// only this explicit reconciliation retires fixed bindings, retaining immutable old records.
    pub fn reconcile_captured_programming_fixed_sources(
        &mut self,
        programmer_values: &[(Uuid, i16, DynamicAddressValue)],
        programmer_rows: Option<&[light_engine::CapturedDynamicProgrammerRow]>,
        cue_values: &[light_playback::ActiveCueDynamicValue],
    ) -> Result<Vec<CapturedFixedSource>, IntentError> {
        let mut planned = Vec::new();
        for (index, (programmer_id, priority, row)) in programmer_values.iter().enumerate() {
            let Some(mask) = captured_programming_fixed_mask(&row.attribute, &row.value)? else {
                continue;
            };
            let Some(captured) = programmer_rows.and_then(|rows| rows.get(index)) else {
                continue;
            };
            if captured.programmer_id.0 != *programmer_id
                || captured.changed_at_millis != row.changed_at_millis
                || captured.programmer_order != row.programmer_order
            {
                return Err(invalid(
                    "fixed Programmer sidecar does not match its captured row",
                ));
            }
            let (lane, expected_source) = match captured.lane {
                light_engine::CapturedDynamicProgrammerLane::Live => (
                    DynamicProgrammerSourceLane::Live,
                    light_engine::ContributionSourceId::programmer(captured.programmer_id),
                ),
                light_engine::CapturedDynamicProgrammerLane::Preload => (
                    DynamicProgrammerSourceLane::Preload,
                    light_engine::ContributionSourceId::preload(captured.programmer_id),
                ),
            };
            if captured.source != expected_source {
                return Err(invalid("fixed Programmer source and lane disagree"));
            }
            let stamp = DynamicFixedStamp::Programmer {
                changed_at_millis: row.changed_at_millis,
                programmer_order: row.programmer_order,
            };
            if captured
                .stamp
                .map(|stamp| (stamp.changed_at, stamp.programmer_order))
                != stamp
                    .exact_changed_at()
                    .map(|at| (at, row.programmer_order))
            {
                return Err(invalid(
                    "fixed Programmer exact stamp does not match its raw fields",
                ));
            }
            planned.push((
                DynamicSourceBinding::Fixed {
                    source: DynamicFixedSource::Programmer {
                        programmer_id: captured.programmer_id,
                        lane,
                    },
                    target: row.fixture_id,
                    owner: mask.address.owner(),
                    component: mask.address.component,
                },
                DynamicSourceOrigin::Fixed {
                    stamp,
                    priority: *priority,
                    value: row.value.clone(),
                },
            ));
        }
        for row in cue_values {
            let Some(mask) = captured_programming_fixed_mask(&row.attribute, &row.value)? else {
                continue;
            };
            if row.source_key.source() != row.source
                || row.cue_list_id != row.source.cue_list_id
                || row.changed_at_millis
                    != u64::try_from(row.changed_at.timestamp_millis()).unwrap_or_default()
            {
                return Err(invalid(
                    "fixed Cue source or timestamp disagrees with its capture",
                ));
            }
            let temporary_kind = match row.source_key {
                light_playback::CueDynamicSourceKey::Normal { .. } => None,
                light_playback::CueDynamicSourceKey::Temporary { kind, .. } => Some(kind.into()),
            };
            planned.push((
                DynamicSourceBinding::Fixed {
                    source: DynamicFixedSource::Cue {
                        source: row.source.into(),
                        temporary_kind,
                    },
                    target: row.fixture_id,
                    owner: mask.address.owner(),
                    component: mask.address.component,
                },
                DynamicSourceOrigin::Fixed {
                    stamp: DynamicFixedStamp::Cue {
                        authored_cue_id: row.authored_cue_id,
                        changed_at: row.changed_at,
                        transition_ordinal: row.transition_ordinal,
                    },
                    priority: row.priority,
                    value: row.value.clone(),
                },
            ));
        }
        let mut active =
            rustc_hash::FxHashSet::with_capacity_and_hasher(planned.len(), Default::default());
        for (binding, origin) in &planned {
            validate_binding(*binding)?;
            origin.validate(*binding)?;
            if !active.insert(*binding) {
                return Err(invalid("duplicate captured fixed source address"));
            }
        }
        let mut captured = Vec::with_capacity(planned.len());
        for (binding, origin) in planned {
            captured.push(CapturedFixedSource {
                binding,
                occurrence_id: self.bind(binding, origin)?,
            });
        }
        self.retain_bindings_by_key(|binding, _| {
            !matches!(binding, DynamicSourceBinding::Fixed { .. }) || active.contains(binding)
        });
        Ok(captured)
    }
}

#[cfg(test)]
mod tests;
