//! Nominal and uncalibrated physical Color models for modes that author none.
//!
//! The Live Color adapter, the engine's physical projection (Stage/preview) and the colour report
//! all evaluate `FixtureMode::color_physical` through `CompiledColorForward`/`CompiledColorFitting`.
//! A mode without that model would get no semantic colour at all. This module derives one, in the
//! transient runtime projection only ([`super::apply_runtime_profile_compatibility`]), from the
//! colour systems Color Intent already reads for the head ([`FixtureMode::intent_color_systems`]):
//!
//! - an authored colour system keeps its declared calibration: `measured` data stays measured,
//!   `nominal` (the default) becomes estimated provenance;
//! - a head whose profile authors no colour system is resolved from its channel names (sRGB
//!   primaries, D65 white, typical amber/lime/indigo LEDs) and carries **unknown** provenance, so
//!   every result reads Uncalibrated;
//! - UV is a controllable emitter whose visible output is unknown, never assumed black;
//! - CMY flags without measured filters are ideal sRGB complements of a D65 beam. Ideal block
//!   flags transmit `R·(1-c) + G·(1-m) + B·(1-y)`, which is exactly three reversed additive
//!   primaries, so the existing continuous solver drives them;
//! - a colour wheel on a head without emitters becomes one filter per slot function whose
//!   transmission is unknown, plus a whole-path observation per steady slot whose colour is
//!   declared (or, uncalibrated, named). A slot without a colour stays unknown;
//! - a hue/saturation engine becomes a nominal sRGB HSV grid of whole-path observations over a
//!   D65 source (estimated at best), so the existing fitter picks the nearest grid point.
//!
//! Every head picks its most capable engine (continuous emitters or flags, else hue/saturation,
//! else the wheel with the most coloured steady slots, else white only). Every other colour
//! control of the head (colour temperature, tint, colour point, a wheel in front of emitters, a
//! second wheel, a colour macro) is **parked**: bound to a filter whose only known state is its
//! neutral range with a unit transmission, so the fitter writes that state and the forward model
//! knows the output exactly while it stays there. A master-shared head's colour controls over
//! child heads with their own emitters stay at their defaults (several logical heads would
//! otherwise co-own them). What cannot be described yields an explicit reason
//! ([`DerivedColorOutcome::Excluded`]). The immutable profile is never changed.
use super::{ColorPhysicalModel, FixtureChannel, FixtureMode, FixtureProfile, OpticalSource};
use uuid::Uuid;

mod path;
use path::is_color_channel;

/// Source text of every derived provenance, so reports and tools can name the estimate.
pub const DERIVED_UNCALIBRATED_SOURCE: &str =
    "Uncalibrated: nominal sRGB/D65 values inferred from channel names; not fixture data";
pub const DERIVED_NOMINAL_SOURCE: &str =
    "Nominal: derived from the profile's colour system without a physical model";
/// Provenance of a colour control parked at its neutral state (unit transmission).
pub const DERIVED_PARKED_SOURCE: &str =
    "Parked: colour control held at its neutral default; its other states are not modelled";
/// Provenance of a hue/saturation engine's nominal sRGB HSV grid.
pub const DERIVED_HUE_SATURATION_SOURCE: &str =
    "Nominal: hue/saturation engine mapped to an sRGB HSV grid; not fixture data";

/// What the runtime colour fallback does for one mode.
#[derive(Clone, Debug)]
pub enum DerivedColorOutcome {
    /// The profile authors its own physical Color model.
    Authored,
    /// The mode has no colour control.
    NoColorControls,
    /// A Media Server personality: Media keeps its own colour path.
    MediaColor,
    Derived(ColorPhysicalModel),
    /// The colour controls cannot be described; the reason names the head and the control.
    Excluded(String),
}

/// Give every mode without an authored physical Color model the derived one, when the mode's
/// colour channels can be described. Authored models are never touched.
pub fn apply_derived_color_physical(profile: &mut FixtureProfile) {
    for mode in &mut profile.modes {
        if mode.color_physical.is_none() {
            mode.color_physical = mode.derived_color_physical();
        }
    }
}

impl FixtureMode {
    /// The nominal/uncalibrated physical Color model this mode would get at runtime, or `None`
    /// when it authors one, has no colour, or its colour cannot be described.
    pub fn derived_color_physical(&self) -> Option<ColorPhysicalModel> {
        match self.derived_color_outcome() {
            DerivedColorOutcome::Derived(model) => Some(model),
            _ => None,
        }
    }

    fn colored_heads(&self) -> Vec<Uuid> {
        self.heads
            .iter()
            .filter(|head| {
                self.channels
                    .iter()
                    .any(|c| c.head_id == head.id && is_color_channel(c))
            })
            .map(|head| head.id)
            .collect()
    }

    /// A master-shared head whose colour controls sit over child heads with colour of their
    /// own, while it brings no emitters itself. It gets no path: its controls stay at default.
    fn shared_colour_layer(&self, head: Uuid, colored: &[Uuid]) -> bool {
        colored.len() > 1
            && self.heads.iter().any(|h| h.id == head && h.master_shared)
            && !path::has_continuous_engine(self, head)
    }

    /// The runtime colour fallback of this mode, with the reason when it cannot derive one.
    pub fn derived_color_outcome(&self) -> DerivedColorOutcome {
        if self.color_physical.is_some() {
            return DerivedColorOutcome::Authored;
        }
        if self
            .heads
            .iter()
            .any(|head| crate::media_color::has_media_color_identity(self, head.id))
        {
            return DerivedColorOutcome::MediaColor;
        }
        let colored = self.colored_heads();
        if colored.is_empty() {
            return DerivedColorOutcome::NoColorControls;
        }
        let mut paths = Vec::new();
        let mut revision = 0;
        for &head in &colored {
            if self.shared_colour_layer(head, &colored) {
                continue;
            }
            match path::derive_head_path(self, head) {
                Ok((path, head_revision)) => {
                    revision = revision.max(head_revision);
                    paths.push(path);
                }
                Err(reason) => {
                    return DerivedColorOutcome::Excluded(format!(
                        "{}: {reason}",
                        self.head_label(head)
                    ));
                }
            }
        }
        if paths.is_empty() {
            return DerivedColorOutcome::Excluded(
                "only shared colour controls without a light source of their own".into(),
            );
        }
        let model = ColorPhysicalModel {
            version: 1,
            revision,
            paths,
        };
        let mut probe = self.clone();
        probe.color_physical = Some(model);
        match probe.validate_color_physical() {
            Ok(()) => DerivedColorOutcome::Derived(probe.color_physical.unwrap()),
            Err(error) => DerivedColorOutcome::Excluded(format!(
                "the derived colour model is not valid: {error}"
            )),
        }
    }

    fn head_label(&self, head: Uuid) -> String {
        self.heads
            .iter()
            .find(|h| h.id == head)
            .map(|h| h.name.trim())
            .filter(|name| !name.is_empty())
            .map_or_else(|| "Head".into(), str::to_owned)
    }

    /// Operator note for a head shown through a derived model: parked colour controls, shared
    /// colour controls left at their defaults, a nominal hue/saturation grid or a white-only
    /// head. `None` for an authored model or a head with nothing to say.
    pub fn derived_color_note(&self, head_id: Uuid) -> Option<String> {
        let model = self.color_physical.as_ref()?;
        let path = model.paths.iter().find(|p| p.head_id == head_id)?;
        let name = |id: Uuid| self.channels.iter().find(|c| c.id == id).map(channel_name);
        let mut notes = Vec::new();
        let parked: Vec<String> = path
            .filters
            .iter()
            .filter(|f| f.provenance.source.as_deref() == Some(DERIVED_PARKED_SOURCE))
            .filter_map(|f| name(f.binding.channel_id))
            .collect();
        if !parked.is_empty() {
            notes.push(format!("parked at neutral: {}", parked.join(", ")));
        }
        let colored = self.colored_heads();
        let shared: Vec<String> = self
            .channels
            .iter()
            .filter(|c| {
                c.head_id != head_id
                    && is_color_channel(c)
                    && !path.controls.contains(&c.id)
                    && self.shared_colour_layer(c.head_id, &colored)
            })
            .map(channel_name)
            .collect();
        if !shared.is_empty() {
            notes.push(format!("shared, left at default: {}", shared.join(", ")));
        }
        if path
            .filters
            .iter()
            .any(|f| f.provenance.source.as_deref() == Some(DERIVED_HUE_SATURATION_SOURCE))
        {
            notes.push("hue/saturation engine on a nominal sRGB grid".into());
        } else if matches!(path.source, OpticalSource::Fixed { .. }) && path.measurements.is_empty()
        {
            notes.push("white only: no colour engine".into());
        }
        (!notes.is_empty()).then(|| notes.join("; "))
    }

    /// Why a head with colour controls has no colour path, for the colour report. `None` when
    /// the head has a path, no colour control, or Media's own colour path.
    pub fn color_model_exclusion(&self, head_id: Uuid) -> Option<String> {
        if self
            .color_physical
            .as_ref()
            .is_some_and(|model| model.paths.iter().any(|path| path.head_id == head_id))
        {
            return None;
        }
        let colored = self.colored_heads();
        if !colored.contains(&head_id) {
            return None;
        }
        if self.shared_colour_layer(head_id, &colored) {
            return Some("shared colour controls stay at their defaults".into());
        }
        let mut probe = self.clone();
        probe.color_physical = None;
        match probe.derived_color_outcome() {
            DerivedColorOutcome::Excluded(reason) => Some(reason),
            // Media keeps its own colour path; nothing is missing.
            DerivedColorOutcome::MediaColor => None,
            _ => Some("no colour model".into()),
        }
    }
}

/// A short operator name for a colour control: its single function's name, else its attribute.
fn channel_name(channel: &FixtureChannel) -> String {
    let named = channel
        .functions
        .first()
        .map(|f| f.name.trim())
        .filter(|name| !name.is_empty() && !name.contains('.') && channel.functions.len() == 1);
    match named {
        Some(name) => name.to_owned(),
        None => {
            let attribute = channel.fixture_attribute.0.as_ref();
            attribute
                .strip_prefix("color.")
                .or_else(|| attribute.strip_prefix("fixture."))
                .unwrap_or(attribute)
                .replace(['_', '.'], " ")
        }
    }
}
