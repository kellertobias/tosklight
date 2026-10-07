//! TL-554 Direct (native) Color: encoder pages 3/4 and the modal overflow, the edit options of a
//! native or first semantic edit, and the passive per-head Direct replay status.
//!
//! Native controls are identified by profile channel and function UUIDs only. A label is shown,
//! never matched: no consumer may pair raw channels by name, alias or DMX slot.

use super::output_control::OutputFrameIdentity;
use super::programming_intent::ProgrammingNativeColorIdentity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// `ApplyIntent.native_reference`: the root head whose native controls a Direct (`native`)
/// edit names. Absent, the server takes the first verified head of the ordered selection (OSC
/// and HTTP integrators). The first native edit seeds that head's current premaster output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorReferenceRef {
    pub fixture_id: Uuid,
    pub head_id: Uuid,
}

/// `ApplyIntent.explicit_color_start`: the operator's explicit starting colour for the first
/// semantic edit of a Direct value whose visible appearance is unknown. A virtual RGB recipe
/// (0..1 each); `[0,0,0]` is black. The server never invents one.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExplicitColorStart {
    pub rgb: [f32; 3],
}

/// Where the semantic starting value of an adopted Direct value came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorAdoptionStart {
    /// The Direct value's modelled appearance; the virtual recipe is an approximation.
    Approximate,
    /// Visible appearance was unknown: the operator's explicit starting colour was used.
    Explicit,
}

/// One fixture adopted from Direct by the first semantic edit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorAdoptionFixture {
    pub fixture_id: Uuid,
    pub start: ColorAdoptionStart,
    /// UV was unknown and adopted off.
    pub uv_unknown: bool,
}

/// Outcome field: the first semantic edit of a Direct value adopted a semantic starting value.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorAdoptionReport {
    pub fixtures: Vec<ColorAdoptionFixture>,
    pub limitations: Vec<String>,
}

/// Width of a native control, from its full-width raw maximum.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NativeColorResolution {
    #[serde(rename = "8bit")]
    Bits8,
    #[serde(rename = "16bit")]
    Bits16,
    #[serde(rename = "24bit")]
    Bits24,
    #[serde(rename = "32bit")]
    Bits32,
}

/// One recordable function of a native control (service/control functions are excluded).
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorFunctionDescriptor {
    pub function_id: Uuid,
    /// Display label only (for example a wheel slot name); never an identity.
    pub label: String,
    pub raw_from: u32,
    pub raw_to: u32,
    /// Continuous functions take relative and spread edits; discrete ones are choices.
    pub continuous: bool,
}

/// One native control of the reference head's optical path.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorControlDescriptor {
    /// Stable slot id `native.<channel_id>`.
    pub id: String,
    pub channel_id: Uuid,
    /// Display label only; never an identity.
    pub label: String,
    /// Full-width raw maximum (U8 255 … U32 4294967295).
    pub raw_max: u32,
    pub resolution: NativeColorResolution,
    /// The profile declares an ultraviolet emitter on this control.
    pub ultraviolet: bool,
    pub functions: Vec<NativeColorFunctionDescriptor>,
}

/// One native encoder page: four slots in encoder order; `null` is an empty encoder.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorPage {
    /// Color family page number (3 or 4).
    pub number: u8,
    pub controls: Vec<Option<NativeColorControlDescriptor>>,
}

/// The clearly identified reference head of the native pages.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorReference {
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub fixture_name: String,
    pub head_id: Uuid,
    pub head_name: String,
    /// True when the operator chose it; false for the first verified head of the selection.
    pub chosen: bool,
    pub identity: ProgrammingNativeColorIdentity,
}

/// A selectable reference head of the ordered selection.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorReferenceCandidate {
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub fixture_name: String,
    pub head_id: Uuid,
    pub head_name: String,
}

/// How a selected fixture will replay a Direct value of the reference head.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NativeColorReplayPreview {
    /// Verified identical identity and native layout: the recipe replays exactly.
    Exact,
    /// Any other head: best-effort match from the recipe's portable estimate.
    Fallback,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorFixturePreview {
    pub fixture_id: Uuid,
    pub replay: NativeColorReplayPreview,
}

/// Why no native pages are published.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NativeColorPagesUnavailable {
    /// The runtime does not support the semantic programming contract (legacy pages stay).
    Contract,
    /// No selected head has a verified native Color identity. Quiet, not an error.
    NoVerifiedHead,
}

/// One current premaster native value of the reference head (read-only inspection).
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorValueReadout {
    pub channel_id: Uuid,
    pub function_id: Uuid,
    pub raw: u32,
}

/// The reference head's premaster values in one accepted Live frame.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorValues {
    pub frame: OutputFrameIdentity,
    pub controls: Vec<NativeColorValueReadout>,
}

/// `GET /api/v2/programming/color/native-pages?fixture_ids=…[&reference=…&head=…]` response.
/// Reading it changes nothing: no history, no intent, no lease.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeColorPagesSnapshot {
    pub semantic: bool,
    #[ts(type = "number")]
    pub show_revision: u64,
    /// The requested fixtures, in request order (duplicates kept).
    pub fixture_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub unavailable: Option<NativeColorPagesUnavailable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub reference: Option<NativeColorReference>,
    /// Verified reference heads of the selection, in selection order.
    pub candidates: Vec<NativeColorReferenceCandidate>,
    /// Pages 3 and 4: at most eight controls in path order.
    pub pages: Vec<NativeColorPage>,
    /// Every further control, in path order, for the full Color modal. Nothing is omitted.
    pub overflow: Vec<NativeColorControlDescriptor>,
    /// Predicted replay of the reference recipe on each selected Color fixture.
    pub fixtures: Vec<NativeColorFixturePreview>,
    /// The reference head's values in the latest accepted frame, when it outputs Color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub values: Option<NativeColorValues>,
}

/// How a head replays a Direct value: passive status, never an error.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentDirectReplay {
    /// Verified compatible layout: the recorded native controls are written unchanged. This is
    /// native identity, not a claim of an exact visible match (see the head's `quality`).
    Exact,
    /// Best-effort match fitted from the recipe's portable estimate.
    Fallback,
    /// Incompatible head and unknown appearance: the visible output is held, never invented.
    NativeOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentDirectCompatibility {
    Compatible,
    NoNativeColor,
    DifferentSource,
    ChangedLayout,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentDirectUv {
    /// The recipe's UV amount (including zero) is applied.
    Apply,
    /// UV is unknown: UV emitters are parked off.
    ParkOff,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentDirectOrigin {
    /// Forward-evaluated this frame by the recipe's original model.
    Forward,
    /// The original model is unavailable; the recorded estimate was used.
    Recorded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentDriveLimit {
    Within,
    /// Passive diagnostic: the recipe drives above the model's maximum.
    AboveModelMaximum,
    Unknown,
}

/// One head's Direct replay status in an accepted frame.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorIntentDirectReport {
    pub replay: ColorIntentDirectReplay,
    /// Fallback only: why the head is not an exact replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub compatibility: Option<ColorIntentDirectCompatibility>,
    /// Fallback only: the UV decision, independent of the visible one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub uv: Option<ColorIntentDirectUv>,
    pub origin: ColorIntentDirectOrigin,
    pub drive_limit: ColorIntentDriveLimit,
    pub limitations: Vec<String>,
}

pub fn native_resolution(raw_max: u32) -> NativeColorResolution {
    match raw_max {
        0..=0xFF => NativeColorResolution::Bits8,
        0x100..=0xFFFF => NativeColorResolution::Bits16,
        0x1_0000..=0xFF_FFFF => NativeColorResolution::Bits24,
        _ => NativeColorResolution::Bits32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_color_contracts_are_snake_case_and_full_width() {
        assert_eq!(native_resolution(255), NativeColorResolution::Bits8);
        assert_eq!(native_resolution(65_535), NativeColorResolution::Bits16);
        assert_eq!(native_resolution(16_777_215), NativeColorResolution::Bits24);
        assert_eq!(native_resolution(u32::MAX), NativeColorResolution::Bits32);
        assert_eq!(
            serde_json::to_value(NativeColorResolution::Bits24).unwrap(),
            serde_json::json!("24bit")
        );
        let control = NativeColorFunctionDescriptor {
            function_id: Uuid::nil(),
            label: "Red".into(),
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        };
        let decoded: NativeColorFunctionDescriptor =
            serde_json::from_value(serde_json::to_value(&control).unwrap()).unwrap();
        assert_eq!(
            decoded.raw_to,
            u32::MAX,
            "32-bit maxima survive JSON exactly"
        );
        assert_eq!(
            serde_json::to_value(ColorIntentDirectReplay::NativeOnly).unwrap(),
            serde_json::json!("native_only")
        );
        let start: ExplicitColorStart =
            serde_json::from_value(serde_json::json!({"rgb":[0.0,0.0,0.0]})).unwrap();
        assert_eq!(start.rgb, [0.0; 3]);
    }
}
