use super::{DynamicSourceOccurrenceId, DynamicValueAddress};
use light_core::programming::{
    ColorComponent, IntentError, ProgrammingComponent, ProgrammingFieldScope,
    ProgrammingFieldTransfer, ProgrammingOwner, ProgrammingTraceField,
};
use serde::{Deserialize, Deserializer, Serialize};

/// A used Current input. An absent record means no dependency; an absent occurrence in a
/// present record means its identity is unknown. Neither case invents transfer evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DynamicSourceDependency {
    pub occurrence: Option<DynamicSourceOccurrenceId>,
    pub transfer: DynamicSourceTransfer,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "transfer",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DynamicSourceTransfer {
    Identity,
    Mapped(ProgrammingFieldTransfer),
    Unknown,
}

impl<'de> Deserialize<'de> for DynamicSourceDependency {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Record {
            occurrence: Option<DynamicSourceOccurrenceId>,
            transfer: DynamicSourceTransfer,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Legacy(DynamicSourceOccurrenceId),
            Record(Record),
        }
        Ok(match Wire::deserialize(deserializer)? {
            // A historical UUID proves identity, but carries no proof that the Current
            // value avoided geometry/appearance adoption in that old frame.
            Wire::Legacy(id) => Self::unknown(Some(id)),
            Wire::Record(record) => Self {
                occurrence: record.occurrence,
                transfer: record.transfer,
            },
        })
    }
}

impl DynamicSourceDependency {
    pub fn unknown(occurrence: Option<DynamicSourceOccurrenceId>) -> Self {
        Self {
            occurrence,
            transfer: DynamicSourceTransfer::Unknown,
        }
    }

    pub fn identity(occurrence: Option<DynamicSourceOccurrenceId>) -> Self {
        Self {
            occurrence,
            transfer: DynamicSourceTransfer::Identity,
        }
    }

    pub fn mapped(
        occurrence: Option<DynamicSourceOccurrenceId>,
        transfer: ProgrammingFieldTransfer,
    ) -> Self {
        Self {
            occurrence,
            transfer: DynamicSourceTransfer::Mapped(transfer),
        }
    }

    /// The producer has already proved that Current was read in this representation without
    /// adoption. Color component reads and writes have different scopes: recipe values feed
    /// the derived XYZ result, and HSV reads the recorded RGB recipe rather than XYZ.
    pub fn compatible(
        occurrence: Option<DynamicSourceOccurrenceId>,
        address: &DynamicValueAddress,
    ) -> Self {
        use ProgrammingTraceField as F;
        let Some(component) = address.component else {
            return Self::identity(occurrence);
        };
        let read = match component {
            ProgrammingComponent::Color(ColorComponent::Red) => {
                ProgrammingFieldScope::new([F::ColorRecipeRed])
            }
            ProgrammingComponent::Color(ColorComponent::Green) => {
                ProgrammingFieldScope::new([F::ColorRecipeGreen])
            }
            ProgrammingComponent::Color(ColorComponent::Blue) => {
                ProgrammingFieldScope::new([F::ColorRecipeBlue])
            }
            ProgrammingComponent::Color(ColorComponent::Amber) => {
                ProgrammingFieldScope::new([F::ColorRecipeAmber])
            }
            ProgrammingComponent::Color(ColorComponent::Hue | ColorComponent::Saturation) => {
                ProgrammingFieldScope::new([
                    F::ColorRecipeRed,
                    F::ColorRecipeGreen,
                    F::ColorRecipeBlue,
                ])
            }
            _ => return Self::identity(occurrence),
        };
        let written = ProgrammingFieldScope::from_component(component);
        Self::mapped(
            occurrence,
            ProgrammingFieldTransfer {
                identity: ProgrammingFieldScope::empty(),
                remap: read
                    .fields()
                    .flat_map(|input| written.fields().map(move |output| (input, output)))
                    .collect(),
            },
        )
    }

    pub fn validate(&self, owner: ProgrammingOwner) -> Result<(), IntentError> {
        if let DynamicSourceTransfer::Mapped(transfer) = &self.transfer {
            transfer.identity.validate(owner)?;
            ProgrammingFieldScope::new(
                transfer
                    .remap
                    .iter()
                    .flat_map(|(input, output)| [*input, *output]),
            )
            .validate(owner)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
