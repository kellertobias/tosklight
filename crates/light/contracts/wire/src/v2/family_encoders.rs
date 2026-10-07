//! Per-selection semantic family encoder pages (TL-549 Position, TL-550 Color, TL-551 Focus/Zoom).
//!
//! The server publishes, for the requested fixture selection, the semantic encoder pages of each
//! programming family with the compiled component descriptor of every slot: unit, domain, steps,
//! display scale, the selection's common limits and (for Zoom) the opening convention. A desk uses
//! these pages only when `semantic` is true, which mirrors the runtime's supported programming
//! contract; at contract 0 the legacy normalized attribute pages stay in force unchanged.
//!
//! Presentation (Easy/Advanced Color) is a per-desk setting stored in desk configuration, never in
//! a portable show. The snapshot echoes the setting it was laid out with.

use super::programming_intent::{
    ProgrammingAttributeBounds, ProgrammingComponent, ProgrammingComponentDescriptor,
    ProgrammingOpeningConvention, ProgrammingOwner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The desk's Color encoder presentation. Switching it changes layout only; it never converts,
/// clears or re-authors a programmed Color value.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorEncoderPresentation {
    /// Page 1 Red, Green, Blue, White Blend. The default operator view.
    #[default]
    EasyRgbw,
    /// Easy RGBW plus page 2 Amber, UV.
    EasyRgbwauv,
    /// Page 1 as Easy; page 2 Temperature, Duv, Wheel 1, Wheel 2.
    Advanced,
}

/// The family tab that owns a set of semantic pages. Focus hosts both the Focus and Zoom owners.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FamilyEncoderFamily {
    Position,
    Color,
    Focus,
}

/// Where a slot's `limits` came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FamilyEncoderLimitsSource {
    /// The component descriptor's own bounded domain (for example Focus 0..1).
    Descriptor,
    /// Every selected fixture's profile declares the same physical range.
    Selection,
    /// Selected profiles disagree; no common limit is claimed (never an intersection guess).
    Mixed,
    /// A selected profile declares no physical range for the component.
    Unknown,
    /// The component is finite and has no published limit (Target offsets in metres).
    Unbounded,
}

/// How a slot is edited through `component_edits`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FamilyEncoderEditKind {
    /// A typed scalar component edit (`set` or `relative`).
    Scalar,
    /// A Target reference choice (`target { reference }`); not a scalar.
    TargetReference,
    /// No typed component edit exists yet (Color wheels); the slot is display-only.
    Unavailable,
}

/// One semantic component encoder.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FamilyEncoderComponentSlot {
    /// Stable slot id, for example `position.pan`, `position.target.x`, `color.white_blend`.
    pub id: String,
    pub label: String,
    pub component: ProgrammingComponent,
    /// The compiled runtime descriptor of `component`, unchanged.
    pub descriptor: ProgrammingComponentDescriptor,
    /// The selection's common limits in descriptor units, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub limits: Option<ProgrammingAttributeBounds>,
    pub limits_source: FamilyEncoderLimitsSource,
    /// Zoom only: the selection's common opening convention, when every profile declares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub convention: Option<ProgrammingOpeningConvention>,
    pub edit: FamilyEncoderEditKind,
    /// Requested fixtures that carry this component's owner, in request order.
    pub fixture_ids: Vec<Uuid>,
}

/// A slot of a semantic page: a component, or a registry attribute that keeps its normalized path.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FamilyEncoderSlot {
    Component(FamilyEncoderComponentSlot),
    /// A non-semantic registry attribute placed on a semantic page (Focus page: Softness).
    Attribute {
        attribute: String,
        label: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FamilyEncoderPage {
    /// 1-based page number within the family.
    pub number: u8,
    pub label: String,
    /// Four slots in encoder order; `null` is the standard empty encoder.
    pub slots: Vec<Option<FamilyEncoderSlot>>,
}

/// Why a page number is reserved without content.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FamilyEncoderReservation {
    /// Direct (native) Color pages 3/4, owned by TL-554.
    NativeColor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FamilyEncoderReservedPage {
    pub number: u8,
    pub reason: FamilyEncoderReservation,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FamilyEncoderGroup {
    pub family: FamilyEncoderFamily,
    /// Backend owners authored from this family's pages (Focus: `focus` and `zoom`).
    pub owners: Vec<ProgrammingOwner>,
    /// Requested fixtures carrying at least one owner of this family, in request order.
    pub fixture_ids: Vec<Uuid>,
    /// Registry attributes these pages replace exactly.
    pub replaces_attributes: Vec<String>,
    /// Registry attribute prefixes these pages replace (Color: `color.`).
    pub replaces_attribute_prefixes: Vec<String>,
    pub pages: Vec<FamilyEncoderPage>,
    /// Page numbers kept for later content; a client renders nothing for them yet.
    pub reserved_pages: Vec<FamilyEncoderReservedPage>,
}

/// `GET /api/v2/programming/family-encoder-pages?fixture_ids=…` response.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FamilyEncoderPagesSnapshot {
    /// True only when the runtime supports the semantic programming contract. A client uses
    /// `families` only then; otherwise its legacy normalized pages stay in force.
    pub semantic: bool,
    pub supported_programming_contract: u16,
    /// The contract version the semantic pages require.
    pub semantic_programming_contract: u16,
    /// The per-desk presentation the Color pages were laid out with.
    pub color_presentation: ColorEncoderPresentation,
    #[ts(type = "number")]
    pub show_revision: u64,
    /// The requested fixtures, in request order (duplicates kept).
    pub fixture_ids: Vec<Uuid>,
    pub families: Vec<FamilyEncoderGroup>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_and_slots_are_readable_snake_case_contracts() {
        assert_eq!(
            serde_json::to_value(ColorEncoderPresentation::default()).unwrap(),
            serde_json::json!("easy_rgbw")
        );
        let slot = FamilyEncoderSlot::Attribute {
            attribute: "softness".into(),
            label: "Softness".into(),
        };
        assert_eq!(
            serde_json::to_value(&slot).unwrap(),
            serde_json::json!({"kind":"attribute","attribute":"softness","label":"Softness"})
        );
        let decoded: ColorEncoderPresentation =
            serde_json::from_value(serde_json::json!("advanced")).unwrap();
        assert_eq!(decoded, ColorEncoderPresentation::Advanced);
    }
}
