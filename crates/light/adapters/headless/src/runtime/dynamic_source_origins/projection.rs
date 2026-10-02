//! Expand one family query against its captured catalogue. This is semantic source evidence;
//! physical resolvers later supply the fields actually consumed by each native control.
use super::*;
use light_dynamics::{FamilyTraceFootprint, FamilyTraceQuery, FamilyTraceRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum DynamicFamilySourceUnknown {
    Transfer,
    Identity,
    FieldScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourcePart {
    Dynamic(FamilyTraceFootprint),
    Static(usize),
}

/// Original metadata stays in its immutable record. `relationship` describes how this query
/// uses that record; a Current read never rewrites the original static edit as a Dynamic author.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct DynamicProjectedFamilySource {
    record: Arc<DynamicSourceRecord>,
    part: SourcePart,
    relationship: FamilyTraceRole,
    fields: ProgrammingFieldScope,
}

impl DynamicProjectedFamilySource {
    pub fn record(&self) -> &Arc<DynamicSourceRecord> {
        &self.record
    }

    pub fn static_source(&self) -> Option<&DynamicStaticSourceEntry> {
        match (&self.record.origin, self.part) {
            (DynamicSourceOrigin::StaticBaseline { sources }, SourcePart::Static(index)) => {
                Some(&sources[index])
            }
            _ => None,
        }
    }

    pub fn footprint(&self) -> FamilyTraceFootprint {
        match self.part {
            SourcePart::Dynamic(footprint) => footprint,
            SourcePart::Static(_) => match self.static_source().unwrap().footprint {
                DynamicStaticFootprint::Whole => FamilyTraceFootprint::Whole,
                DynamicStaticFootprint::Component(component) => {
                    FamilyTraceFootprint::Component(component)
                }
            },
        }
    }

    pub fn relationship(&self) -> FamilyTraceRole {
        self.relationship
    }

    /// A dependency at either layer remains a dependency. The original stored role is still
    /// available from `static_source`, independently of the use in this composition.
    pub fn role(&self) -> FamilyTraceRole {
        if self.relationship == FamilyTraceRole::CalculationDependency
            || self
                .static_source()
                .is_some_and(|source| source.role == DynamicStaticRole::CalculationDependency)
        {
            FamilyTraceRole::CalculationDependency
        } else {
            FamilyTraceRole::Authored
        }
    }

    /// Fields in the captured source representation, after tracing conversions backwards.
    /// These deliberately need not match the operator's original edit footprint.
    pub fn fields(&self) -> &ProgrammingFieldScope {
        &self.fields
    }
}

/// Reusable query workspace. Cloning a completed projection retains immutable records, so a
/// later composition, pruning or branch publication cannot invalidate the observed sources.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct DynamicFamilySourceProjection {
    entries: Vec<DynamicProjectedFamilySource>,
    unknown: Option<DynamicFamilySourceUnknown>,
}

impl Default for DynamicFamilySourceProjection {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            unknown: Some(DynamicFamilySourceUnknown::Transfer),
        }
    }
}

impl DynamicFamilySourceProjection {
    pub fn entries(&self) -> Option<&[DynamicProjectedFamilySource]> {
        self.unknown.is_none().then_some(self.entries.as_slice())
    }

    pub fn unknown(&self) -> Option<DynamicFamilySourceUnknown> {
        self.unknown
    }

    /// This consumes the query while its composition scratch still names the matching value.
    /// It never observes today's bindings to recover an older occurrence. Unknown evidence
    /// yields a passive status, not a failed operation or a partially certified source list.
    pub fn project(
        &mut self,
        origins: &DynamicSourceOrigins,
        target: FixtureId,
        owner: ProgrammingOwner,
        query: Option<&FamilyTraceQuery>,
        baseline: Option<DynamicSourceOccurrenceId>,
    ) -> Result<(), IntentError> {
        self.entries.clear();
        self.unknown = Some(DynamicFamilySourceUnknown::Transfer);
        let Some(query) = query else {
            return Ok(());
        };
        self.unknown = None;
        let result = self.project_known(origins, target, owner, query, baseline);
        if result.is_err() || self.unknown.is_some() {
            self.entries.clear();
        }
        if result.is_err() {
            self.unknown = Some(DynamicFamilySourceUnknown::Transfer);
        }
        result
    }

    fn project_known(
        &mut self,
        origins: &DynamicSourceOrigins,
        target: FixtureId,
        owner: ProgrammingOwner,
        query: &FamilyTraceQuery,
        baseline: Option<DynamicSourceOccurrenceId>,
    ) -> Result<(), IntentError> {
        for (fields, role) in [
            (&query.base_fields, FamilyTraceRole::Authored),
            (
                &query.base_dependency_fields,
                FamilyTraceRole::CalculationDependency,
            ),
        ] {
            self.expand(origins, target, owner, baseline, fields, role, None)?;
        }
        for leaf in &query.sources {
            self.expand(
                origins,
                target,
                owner,
                leaf.source.occurrence,
                &leaf.fields,
                leaf.source.role,
                Some(leaf.source.footprint),
            )?;
        }
        Ok(())
    }

    fn expand(
        &mut self,
        origins: &DynamicSourceOrigins,
        target: FixtureId,
        owner: ProgrammingOwner,
        occurrence: Option<DynamicSourceOccurrenceId>,
        fields: &ProgrammingFieldScope,
        relationship: FamilyTraceRole,
        dynamic_footprint: Option<FamilyTraceFootprint>,
    ) -> Result<(), IntentError> {
        fields.validate(owner)?;
        if fields.is_empty() {
            return Ok(());
        }
        let Some(record) = occurrence.and_then(|id| origins.get(id)) else {
            self.unknown
                .get_or_insert(DynamicFamilySourceUnknown::Identity);
            return Ok(());
        };
        match record.binding {
            DynamicSourceBinding::StaticBaseline {
                target: recorded,
                owner: family,
            } if recorded != target || family != owner => {
                return Err(invalid("family query does not match its captured baseline"));
            }
            DynamicSourceBinding::Fixed {
                target: recorded,
                owner: family,
                ..
            } if recorded != target || family != owner => {
                return Err(invalid(
                    "family query does not match its captured fixed row",
                ));
            }
            DynamicSourceBinding::Authored {
                target: recorded, ..
            } if recorded != target => {
                return Err(invalid("family query does not match its authored target"));
            }
            _ => {}
        }
        if let DynamicSourceBinding::Fixed { component, .. } = record.binding {
            let footprint =
                component.map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component);
            if dynamic_footprint != Some(footprint) {
                return Err(invalid(
                    "family query changes the fixed row's authored footprint",
                ));
            }
        }
        match &record.origin {
            DynamicSourceOrigin::StaticBaseline { sources } => {
                for (index, source) in sources.iter().enumerate() {
                    let Some(effective) = &source.effective_fields else {
                        // A legacy original footprint does not prove a later converted scope.
                        // Never fill it from a current value or current fixture model.
                        self.unknown
                            .get_or_insert(DynamicFamilySourceUnknown::FieldScope);
                        continue;
                    };
                    effective.validate(owner)?;
                    let participating = fields.intersection(effective);
                    self.append(
                        record,
                        SourcePart::Static(index),
                        relationship,
                        participating,
                    );
                }
            }
            _ => {
                let footprint = dynamic_footprint
                    .ok_or_else(|| invalid("family baseline requires a static source record"))?;
                if matches!(footprint, FamilyTraceFootprint::Component(component) if component.owner() != owner)
                {
                    return Err(invalid("family query mixes authored component owners"));
                }
                self.append(
                    record,
                    SourcePart::Dynamic(footprint),
                    relationship,
                    fields.clone(),
                );
            }
        }
        Ok(())
    }

    fn append(
        &mut self,
        record: &Arc<DynamicSourceRecord>,
        part: SourcePart,
        relationship: FamilyTraceRole,
        fields: ProgrammingFieldScope,
    ) {
        if fields.is_empty() {
            return;
        }
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.record.occurrence_id == record.occurrence_id
                && entry.part == part
                && entry.relationship == relationship
        }) {
            existing.fields = existing.fields.union(&fields);
        } else {
            self.entries.push(DynamicProjectedFamilySource {
                record: record.clone(),
                part,
                relationship,
                fields,
            });
        }
    }
}

#[cfg(test)]
mod tests;
