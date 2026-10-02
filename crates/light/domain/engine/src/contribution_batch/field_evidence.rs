//! Source identities and original edit footprints survive field transfer. A generated field
//! can have no authored contributor; a missing participating input is instead unknown evidence.
use super::*;
use light_core::programming::{
    ProgrammingFieldScope, ProgrammingFieldTransfer, ProgrammingOwner, ProgrammingTransitionTrace,
};

impl ContributionFamilyEntry {
    /// Resolve legacy implicit scope only within the value's actual representation. A native
    /// edit alongside a semantic value needs an explicit conversion scope from its producer.
    pub(crate) fn fields_for_value(
        &self,
        owner: ProgrammingOwner,
        value: &AttributeValue,
    ) -> Option<ProgrammingFieldScope> {
        let available = ProgrammingFieldScope::for_value(owner, value).ok()?;
        let fields = match &self.effective_fields {
            Some(fields) => fields.clone(),
            None => match self.footprint {
                ContributionFamilyFootprint::Whole => available.clone(),
                ContributionFamilyFootprint::Component(component) => {
                    ProgrammingFieldScope::from_component(component)
                }
            },
        };
        fields.validate(owner).ok()?;
        // Never silently discard unexplained source fields and publish the remaining partial
        // history as complete. The producing adapter must supply its conversion explicitly.
        (fields.intersection(&available) == fields).then_some(fields)
    }

    fn same_authored_occurrence(&self, other: &Self) -> bool {
        self.source == other.source
            && self.stamp.changed_at == other.stamp.changed_at
            && self.stamp.programmer_order == other.stamp.programmer_order
            && self.transition_ordinal == other.transition_ordinal
            && self.authored_cue_id == other.authored_cue_id
            && self.footprint == other.footprint
            && self.role == other.role
    }
}

impl ContributionFamilyEvidence {
    /// Construct only at a transition/edit boundary. Frame sampling reuses the resulting Arc.
    pub(crate) fn transferred(
        owner: ProgrammingOwner,
        from_value: &AttributeValue,
        to_value: &AttributeValue,
        from: Option<&Arc<Self>>,
        to: Option<&Arc<Self>>,
        transfer: &ProgrammingTransitionTrace,
    ) -> Option<Arc<Self>> {
        let mut entries = Vec::<ContributionFamilyEntry>::new();
        let mut append = |value: &AttributeValue,
                          evidence: Option<&Arc<Self>>,
                          transfer: &ProgrammingFieldTransfer|
         -> Option<()> {
            // A known empty transfer reads no part of this endpoint, regardless of whether
            // that unused endpoint has a source record.
            let available = ProgrammingFieldScope::for_value(owner, value).ok()?;
            if transfer.forward(&available).is_empty() {
                return Some(());
            }
            let evidence = evidence.filter(|evidence| !evidence.entries().is_empty())?;
            for original in evidence.entries() {
                let fields = transfer.forward(&original.fields_for_value(owner, value)?);
                if fields.is_empty() {
                    continue;
                }
                if let Some(existing) = entries
                    .iter_mut()
                    .find(|existing| existing.same_authored_occurrence(original))
                {
                    existing.effective_fields = Some(
                        existing
                            .effective_fields
                            .as_ref()
                            .expect("transferred scope")
                            .union(&fields),
                    );
                } else {
                    entries.push(original.clone().with_effective_fields(fields));
                }
            }
            Some(())
        };
        append(from_value, from, &transfer.from)?;
        append(to_value, to, &transfer.to)?;
        Some(Arc::new(Self::new(entries)))
    }
}
