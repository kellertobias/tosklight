//! TL-554 Direct (native) Color edit options and the semantic adoption report.
//!
//! A Direct edit names its reference head explicitly (or the first verified head of the ordered
//! selection is used); the first semantic edit of a Direct value whose visible appearance is
//! unknown needs the operator's explicit starting colour. Both are transport facts carried by
//! the intent; neither is stored in a show, an intent or Undo history.
use light_core::FixtureId;
use light_core::programming::ColorIntent;

/// The root head whose native controls a Direct edit names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingNativeReference {
    pub fixture_id: FixtureId,
    pub head_id: uuid::Uuid,
}

/// Colour-adoption options of one value intent. Default: none (OSC and HTTP integrators).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgrammingColorAdoptionRequest {
    pub native_reference: Option<ProgrammingNativeReference>,
    /// Used only when a Direct value's visible appearance is unknown; never invented.
    pub explicit_start: Option<ColorIntent>,
}

/// Where an adopted semantic starting value came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingColorAdoptionStart {
    /// The Direct value's modelled appearance; the virtual recipe is approximate.
    Approximate,
    /// Unknown visible appearance: the operator's explicit starting colour.
    Explicit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingColorAdoptionFixture {
    pub fixture_id: FixtureId,
    pub start: ProgrammingColorAdoptionStart,
    /// UV was unknown and adopted off.
    pub uv_unknown: bool,
}

/// What the first semantic edit of Direct values adopted, reported once with that edit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgrammingColorAdoption {
    pub fixtures: Vec<ProgrammingColorAdoptionFixture>,
    pub limitations: Vec<String>,
}
