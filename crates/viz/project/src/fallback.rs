//! Documented fallback rules for fixture data that does not describe its own optics.
//!
//! Shipped ToskLight profiles frequently omit emitter geometry and physical beam angles. The rules
//! here are derived from the profile's declared `fixture_type` and channel set — never from a
//! hardcoded manufacturer or product-name list — so a light-producing head is always rendered.

use glam::Vec3;
use viz_scene::{BodyKind, EmitterOptics, LightSource, SourceForm};

/// Broad optical class derived from `fixture_type`.
///
/// The classes exist to answer one question: what does light out of this thing look like? A
/// profile and a flood pointed at the same spot with the same angle do not resemble each other,
/// and the split below is the coarsest one that keeps them apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpticalClass {
    /// Narrow, hard-edged projector.
    Beam,
    /// Shuttered/gobo projector: a flat field with a crisp rim.
    Profile,
    /// Spherical lens: a soft rim and a gentle hot spot. Between a profile and a wash.
    Fresnel,
    /// Sealed or reflector lamp: an oval field with a bright middle and a soft halo.
    Par,
    /// Soft wash, LED or otherwise: no rim to speak of, but even across the middle.
    Wash,
    /// Cyc or ground row: the widest and softest of the lot.
    Flood,
    /// Aimable mirror in front of a fixed lamp.
    Scanner,
    /// Laser projector: a scanned path rather than a projected field.
    ///
    /// The odd one out. Every other class answers "what does the light out of this look like" with
    /// a cone and a falloff; a laser answers it with a figure drawn hundreds of times a second, and
    /// none of the field numbers below mean anything for it. It is a class so the classifier has
    /// somewhere to put it and the scanner defaults have somewhere to live.
    Laser,
    /// Scripted flame/spark particle producer rather than a projected light field.
    Effect,
    /// Face-visible source with no meaningful projected beam.
    Emissive,
    /// A bank of individual lamps behind round glass: a blinder, an audience bar, a sunstrip.
    ///
    /// Emissive in every optical respect — what an audience sees is the face, not a beam — and
    /// separate only because of what that face is made of. A strip lights up along its front; one
    /// of these lights up as the row of round lenses it is built from, and drawing it as a panel
    /// loses the one thing that makes a blinder recognisable.
    Blinder,
    /// Atmosphere machine.
    Atmosphere,
    /// Non-light-producing machine, for example a fan or relay.
    Machine,
    /// Visual-only scenery.
    Venue,
}

impl OpticalClass {
    /// Peak (narrow) and field (wide) cone angles in degrees.
    pub fn cone_angles(self) -> (f32, f32) {
        match self {
            Self::Beam => (3.0, 8.0),
            Self::Profile => (10.0, 32.0),
            Self::Fresnel => (12.0, 48.0),
            Self::Par => (10.0, 26.0),
            Self::Wash => (18.0, 55.0),
            Self::Flood => (45.0, 90.0),
            Self::Scanner => (8.0, 16.0),
            // Not a field angle: the divergence of a single beam, which is what a laser has
            // instead. Near enough to zero that nothing treats it as a cone by accident.
            Self::Laser => (0.06, 0.06),
            Self::Emissive | Self::Blinder => (60.0, 110.0),
            Self::Effect | Self::Atmosphere | Self::Machine | Self::Venue => (40.0, 80.0),
        }
    }

    /// How hard this class draws the rim of its field, `1.0` being as crisp as a lantern gets.
    ///
    /// This is the difference an operator names first: a profile cuts, a Fresnel blends, a wash
    /// has no edge at all. A focus or frost channel softens whatever is chosen here, so a profile
    /// out of focus still reads as a profile out of focus rather than as a wash.
    pub fn sharpness(self) -> f32 {
        match self {
            // A laser beam has the hardest edge there is.
            Self::Laser => 1.0,
            Self::Beam => 0.94,
            Self::Profile => 0.86,
            Self::Scanner => 0.72,
            Self::Fresnel => 0.42,
            Self::Par => 0.28,
            Self::Wash => 0.18,
            Self::Flood => 0.08,
            Self::Emissive
            | Self::Blinder
            | Self::Effect
            | Self::Atmosphere
            | Self::Machine
            | Self::Venue => 0.05,
        }
    }

    /// How evenly this class fills its field, `1.0` being flat to the rim.
    ///
    /// Separate from the rim: a good LED wash has no edge and is still even across the middle,
    /// while a PAR is bright in the centre and falls away long before its own rim.
    pub fn uniformity(self) -> f32 {
        match self {
            Self::Laser => 1.0,
            Self::Beam => 0.92,
            Self::Profile => 0.86,
            Self::Scanner => 0.8,
            Self::Wash => 0.72,
            Self::Fresnel => 0.55,
            Self::Flood => 0.5,
            Self::Par => 0.3,
            Self::Emissive
            | Self::Blinder
            | Self::Effect
            | Self::Atmosphere
            | Self::Machine
            | Self::Venue => 0.75,
        }
    }

    /// The lens this class presents when the profile does not describe one.
    pub fn light_source(self) -> LightSource {
        match self {
            // The output window, not the beam: an aperture a few millimetres across.
            Self::Laser => LightSource::round(0.004),
            Self::Beam => LightSource::round(0.13),
            Self::Profile => LightSource::round(0.15),
            Self::Scanner => LightSource::round(0.1),
            Self::Fresnel => LightSource::round(0.2),
            // A PAR's field is famously oval, and so is the lens that makes it.
            Self::Par => LightSource {
                form: SourceForm::Oval,
                width: 0.2,
                height: 0.16,
            },
            Self::Wash => LightSource::round(0.18),
            Self::Flood => LightSource {
                form: SourceForm::Rectangular,
                width: 0.3,
                height: 0.2,
            },
            Self::Emissive => LightSource {
                form: SourceForm::Rectangular,
                width: 0.1,
                height: 0.06,
            },
            // One lamp of the bank, behind its own round glass. Sized for a sealed-beam blinder
            // lamp; a bank whose lamps sit closer together than this is trimmed to its own pitch.
            Self::Blinder => LightSource::round(0.185),
            Self::Effect | Self::Atmosphere | Self::Machine | Self::Venue => {
                LightSource::round(0.06)
            }
        }
    }

    /// Everything this class decides about its own light, before the profile gets a say.
    pub fn optics(self) -> EmitterOptics {
        EmitterOptics {
            output: 1.0,
            gobo_wheel: Vec::new(),
            sharpness: self.sharpness(),
            uniformity: self.uniformity(),
            source: self.light_source(),
        }
    }

    /// Whether a head of this class projects a beam that may be drawn as a cone.
    pub fn is_directional(self) -> bool {
        matches!(
            self,
            Self::Beam
                | Self::Profile
                | Self::Fresnel
                | Self::Par
                | Self::Wash
                | Self::Flood
                | Self::Scanner
        )
    }

    /// Whether this class draws a scanned path instead of a projected field.
    pub fn is_laser(self) -> bool {
        matches!(self, Self::Laser)
    }

    pub fn is_effect(self) -> bool {
        matches!(self, Self::Effect)
    }

    pub fn body_kind(self, moving: bool) -> BodyKind {
        match self {
            // A show laser is a box on a bracket. Its chart declares pan and tilt for the position
            // of the figure inside the scan field, which is not a yoke and must not draw one.
            Self::Laser | Self::Effect => BodyKind::Machine,
            _ if moving => BodyKind::MovingHead,
            Self::Emissive | Self::Blinder => BodyKind::Bar,
            Self::Atmosphere | Self::Machine => BodyKind::Machine,
            Self::Venue => BodyKind::Generic,
            _ => BodyKind::Lantern,
        }
    }
}

/// Classify a profile from its declared `fixture_type`.
///
/// The comparison is normalised so `Profile Moving Head`, `profile moving head`, and
/// `profile_moving_head` all classify identically.
pub fn classify(fixture_type: &str) -> OpticalClass {
    let normalised = fixture_type
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], " ");
    let words: Vec<&str> = normalised.split_whitespace().collect();
    let has = |needle: &str| words.contains(&needle);

    if has("venue") {
        return OpticalClass::Venue;
    }
    if has("effect") || has("pyro") || has("sparkler") || has("flame") {
        return OpticalClass::Effect;
    }
    if has("fogger") || has("hazer") || has("haze") || has("fog") {
        return OpticalClass::Atmosphere;
    }
    if has("fan") || has("relay") || has("other") {
        return OpticalClass::Machine;
    }
    // Before the face-visible rule: a product named "laser strobe" is a laser first.
    if has("laser") || has("lasers") {
        return OpticalClass::Laser;
    }
    // A blinder or sunstrip is face-visible too, and its face is a row of round lamp lenses.
    if has("blinder") || has("sunstrip") || has("audience") {
        return OpticalClass::Blinder;
    }
    // A strobe, strip, or pixel product is face-visible rather than beam-projecting.
    if has("strobe") || has("pixel") || has("strip") {
        return OpticalClass::Emissive;
    }
    if has("scanner") || has("mirror") {
        return OpticalClass::Scanner;
    }
    if has("beam") {
        return OpticalClass::Beam;
    }
    if has("profile") || has("spot") || has("ellipsoidal") {
        return OpticalClass::Profile;
    }
    if has("fresnel") {
        return OpticalClass::Fresnel;
    }
    // An ACL is a sealed-beam PAR in all but name, and it is the narrowest of them.
    if has("par") || has("parcan") || has("acl") {
        return OpticalClass::Par;
    }
    if has("flood") || has("cyc") || has("groundrow") {
        return OpticalClass::Flood;
    }
    if has("wash") {
        return OpticalClass::Wash;
    }
    // A bare dimmer channel is most often feeding a lantern nobody described, so it gets the
    // middle of the road rather than the extremes.
    if has("dimmer") {
        return OpticalClass::Fresnel;
    }
    // An unclassified light-producing head still gets a safe generic projector.
    OpticalClass::Profile
}

/// Whether the plan renderer owns a recognizable type drawing for this declaration.
///
/// Optical fallback deliberately maps unknown lights to a safe projector. Plan artwork needs a
/// separate answer so a genuinely unknown body reaches the visibly plain box promised by the
/// package contract instead of masquerading as a profile lantern.
pub fn has_generic_plan_type(fixture_type: &str) -> bool {
    let normalised = fixture_type
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], " ");
    let known = [
        "acl",
        "beam",
        "blinder",
        "dimmer",
        "emissive",
        "fan",
        "flood",
        "fog",
        "fogger",
        "fresnel",
        "haze",
        "hazer",
        "laser",
        "machine",
        "moving head",
        "other",
        "par",
        "parcan",
        "pixel",
        "profile",
        "relay",
        "scanner",
        "spot",
        "strobe",
        "strip",
        "sunstrip",
        "venue",
        "wash",
    ];
    known.iter().any(|name| {
        normalised == *name
            || normalised.split_whitespace().any(|word| word == *name)
            || (name.contains(' ') && normalised.contains(name))
    })
}

/// Whether a profile describes a moving head, decided by the presence of pan or tilt movement
/// rather than by its name.
pub fn is_moving(has_pan: bool, has_tilt: bool) -> bool {
    has_pan || has_tilt
}

/// Fallback body size in metres when the profile carries no physical dimensions.
pub fn body_size(
    class: OpticalClass,
    moving: bool,
    width_mm: Option<f32>,
    height_mm: Option<f32>,
    depth_mm: Option<f32>,
) -> Vec3 {
    let declared = Vec3::new(
        width_mm.unwrap_or(0.0) / 1000.0,
        height_mm.unwrap_or(0.0) / 1000.0,
        depth_mm.unwrap_or(0.0) / 1000.0,
    );
    if declared.min_element() > 0.02 {
        return declared;
    }
    let fallback = match (class, moving) {
        (_, true) => Vec3::new(0.36, 0.54, 0.36),
        (OpticalClass::Emissive, _) => Vec3::new(1.0, 0.14, 0.14),
        // Deeper and taller than a pixel strip: a bank of lamps needs a housing behind them.
        (OpticalClass::Blinder, _) => Vec3::new(0.96, 0.44, 0.25),
        (OpticalClass::Atmosphere | OpticalClass::Machine, _) => Vec3::new(0.5, 0.36, 0.32),
        (OpticalClass::Venue, _) => Vec3::new(1.0, 1.0, 1.0),
        (OpticalClass::Beam, _) => Vec3::new(0.24, 0.3, 0.42),
        _ => Vec3::new(0.28, 0.34, 0.4),
    };
    // Keep any dimension the profile did declare.
    Vec3::new(
        if declared.x > 0.02 {
            declared.x
        } else {
            fallback.x
        },
        if declared.y > 0.02 {
            declared.y
        } else {
            fallback.y
        },
        if declared.z > 0.02 {
            declared.z
        } else {
            fallback.z
        },
    )
}

/// What a fixture of this class puts out, in lumens, when nothing says otherwise. A declared
/// figure is read against this, so a 20 000 lumen wash outshines a 4 000 lumen one by the ratio
/// the two manufacturers actually claim.
fn reference_lumens(class: OpticalClass) -> f32 {
    match class {
        OpticalClass::Beam => 4_000.0,
        OpticalClass::Profile => 6_000.0,
        OpticalClass::Scanner => 4_000.0,
        // A laser's brightness comes from its optical power, not from a lumen figure that would
        // be meaningless for a source this concentrated.
        OpticalClass::Laser | OpticalClass::Effect => 2_000.0,
        OpticalClass::Fresnel | OpticalClass::Par => 5_000.0,
        OpticalClass::Wash => 8_000.0,
        OpticalClass::Flood => 9_000.0,
        OpticalClass::Emissive | OpticalClass::Blinder => 3_000.0,
        OpticalClass::Atmosphere | OpticalClass::Machine | OpticalClass::Venue => 1_000.0,
    }
}

/// The optics one head is given: the class's own character, with anything the profile actually
/// declares taking precedence, and the lens kept inside the fixture that carries it.
pub fn emitter_optics(
    class: OpticalClass,
    body: Vec3,
    luminous_output_lumens: Option<f32>,
) -> EmitterOptics {
    let mut optics = class.optics();
    if let Some(lumens) = luminous_output_lumens.filter(|lumens| *lumens > 1.0) {
        // Bounded: one mistyped figure in a library must not wash out a whole rig.
        optics.output = (lumens / reference_lumens(class)).clamp(0.2, 5.0);
    }
    // No lens is wider than the lantern it is fitted to. The narrowest side of the body is the
    // honest bound, because a bar is long in one axis and a lens is not.
    let bound = (body.x.min(body.y).min(body.z) * 0.9).max(0.02);
    optics.source.width = optics.source.width.min(bound);
    optics.source.height = optics.source.height.min(bound);
    optics
}

/// Fallback pan and tilt travel in degrees when the geometry graph declares no motion.
pub const FALLBACK_PAN_DEGREES: f32 = 270.0;
pub const FALLBACK_TILT_DEGREES: f32 = 135.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_fixture_type_classifies_without_a_product_name() {
        let cases = [
            ("acl", OpticalClass::Par),
            ("beam moving head", OpticalClass::Beam),
            ("blinder", OpticalClass::Blinder),
            ("dimmer", OpticalClass::Fresnel),
            ("dimmer fresnel", OpticalClass::Fresnel),
            ("dimmer par can", OpticalClass::Par),
            ("dimmer profile", OpticalClass::Profile),
            ("fogger", OpticalClass::Atmosphere),
            ("other", OpticalClass::Machine),
            ("pixel bar", OpticalClass::Emissive),
            ("profile", OpticalClass::Profile),
            ("profile moving head", OpticalClass::Profile),
            ("scanner", OpticalClass::Scanner),
            ("strobe", OpticalClass::Emissive),
            ("strobe pixel bar", OpticalClass::Emissive),
            ("sunstrip", OpticalClass::Blinder),
            ("venue", OpticalClass::Venue),
            ("wash", OpticalClass::Wash),
            ("wash moving head", OpticalClass::Wash),
        ];
        for (fixture_type, expected) in cases {
            assert_eq!(classify(fixture_type), expected, "{fixture_type}");
        }
    }

    #[test]
    fn an_unknown_type_still_produces_a_visible_projector() {
        let class = classify("something nobody shipped yet");
        assert!(class.is_directional());
        let (narrow, wide) = class.cone_angles();
        assert!(narrow > 0.0 && wide > narrow);
    }

    #[test]
    fn emissive_classes_never_claim_a_projected_beam() {
        for fixture_type in ["strobe", "pixel bar", "blinder", "strobe pixel bar"] {
            assert!(!classify(fixture_type).is_directional(), "{fixture_type}");
        }
    }

    #[test]
    fn declared_physical_dimensions_win_over_the_fallback() {
        let size = body_size(
            OpticalClass::Profile,
            false,
            Some(300.0),
            Some(400.0),
            Some(500.0),
        );
        assert_eq!(size, Vec3::new(0.3, 0.4, 0.5));
        let partial = body_size(OpticalClass::Profile, false, Some(300.0), None, None);
        assert_eq!(partial.x, 0.3);
        assert!(partial.y > 0.0 && partial.z > 0.0);
    }

    #[test]
    fn a_moving_head_body_is_chosen_by_movement_not_by_name() {
        assert_eq!(
            classify("wash").body_kind(is_moving(true, true)),
            BodyKind::MovingHead
        );
        assert_eq!(
            classify("wash").body_kind(is_moving(false, false)),
            BodyKind::Lantern
        );
    }
}
