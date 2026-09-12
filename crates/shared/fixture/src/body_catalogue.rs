//! The generic bodies a fixture can be drawn as.
//!
//! Most profiles carry no model of their own, and until now the body was guessed from the
//! `fixture_type` string and the channels a mode happens to have. The guess is usually right and
//! stays as the fallback, but it cannot tell a PAR 64 from a PAR 16, or a two-cell blinder from an
//! eight — so a profile can name its body instead, and this is the list it names one from.
//!
//! Only the names live here. The geometry is a GLB shipped with the Visualizer, and the renderer
//! is what holds the bytes; putting the list in the fixture crate is what lets the desk and the
//! Architect offer it without either of them depending on a renderer.

/// One generic body, in the terms an operator picks it by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyModel {
    /// Stable name, which is also the file's name in `assets/models` and what a show stores.
    pub id: &'static str,
    /// What the picker calls it.
    pub label: &'static str,
    /// The heading it is offered under.
    pub group: BodyGroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum BodyGroup {
    MovingLight,
    Scanner,
    Profile,
    Fresnel,
    Flood,
    ParCan,
    LedPar,
    Blinder,
    Strip,
    Strobe,
    Atmospheric,
    Av,
}

impl BodyGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MovingLight => "Moving lights",
            Self::Scanner => "Scanners",
            Self::Profile => "Profiles",
            Self::Fresnel => "Fresnels",
            Self::Flood => "Floods",
            Self::ParCan => "PAR cans",
            Self::LedPar => "LED PARs",
            Self::Blinder => "Blinders",
            Self::Strip => "Strips and battens",
            Self::Strobe => "Strobes",
            Self::Atmospheric => "Atmospherics",
            Self::Av => "AV",
        }
    }
}

const fn body(id: &'static str, label: &'static str, group: BodyGroup) -> BodyModel {
    BodyModel { id, label, group }
}

/// Every generic body, grouped in the order the picker reads them.
pub const BODY_CATALOGUE: &[BodyModel] = &[
    body(
        "moving-head-profile",
        "Profile moving light, small",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-profile-large",
        "Profile moving light, large",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-wash",
        "Wash moving light, small",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-wash-large",
        "Wash moving light, large",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-led-wash-300",
        "LED wash moving light, 300",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-led-wash-400",
        "LED wash moving light, 400",
        BodyGroup::MovingLight,
    ),
    body(
        "moving-head-led-wash-500",
        "LED wash moving light, 500",
        BodyGroup::MovingLight,
    ),
    body(
        "scanner-compact",
        "Compact mirror scanner",
        BodyGroup::Scanner,
    ),
    body("scanner-mirror-spot", "Mirror scanner", BodyGroup::Scanner),
    body("profile-spot", "Profile, modern", BodyGroup::Profile),
    body(
        "profile-spot-classic",
        "Profile, classic",
        BodyGroup::Profile,
    ),
    body(
        "fresnel-barn-doors-500w",
        "Fresnel 500 W",
        BodyGroup::Fresnel,
    ),
    body("fresnel-barn-doors", "Fresnel 1 kW", BodyGroup::Fresnel),
    body("fresnel-barn-doors-2kw", "Fresnel 2 kW", BodyGroup::Fresnel),
    body("flood-asymmetric", "Asymmetric cyc flood", BodyGroup::Flood),
    body("acl-par-16", "ACL / PAR 16", BodyGroup::ParCan),
    body("par-20", "PAR 20 birdie", BodyGroup::ParCan),
    body("par-56-black", "PAR 56, black", BodyGroup::ParCan),
    body("par-56-silver", "PAR 56, silver", BodyGroup::ParCan),
    body(
        "par-64-short-nose-black",
        "PAR 64 short nose, black",
        BodyGroup::ParCan,
    ),
    body(
        "par-64-short-nose-silver",
        "PAR 64 short nose, silver",
        BodyGroup::ParCan,
    ),
    body(
        "par-64-long-nose-black",
        "PAR 64 long nose, black",
        BodyGroup::ParCan,
    ),
    body(
        "par-64-long-nose-silver",
        "PAR 64 long nose, silver",
        BodyGroup::ParCan,
    ),
    body("led-par-x-in-1", "LED PAR, x-in-1", BodyGroup::LedPar),
    body("led-par-pizza", "LED PAR, pizza lamp", BodyGroup::LedPar),
    body("flat-led-par", "Flat LED PAR", BodyGroup::LedPar),
    body("blinder-2-cell", "Blinder, 2 cell", BodyGroup::Blinder),
    body("blinder-4-cell", "Blinder, 4 cell", BodyGroup::Blinder),
    body("blinder-8-cell", "Blinder, 8 cell", BodyGroup::Blinder),
    body("sunstrip", "Sunstrip, ten lamps", BodyGroup::Strip),
    body("sunstrip-20", "Sunstrip, twenty lamps", BodyGroup::Strip),
    body(
        "led-strip-rgbcct-0500",
        "LED strip, 500 mm",
        BodyGroup::Strip,
    ),
    body(
        "led-strip-rgbcct-1000",
        "LED strip, 1000 mm",
        BodyGroup::Strip,
    ),
    body(
        "led-strip-rgbcct-1500",
        "LED strip, 1500 mm",
        BodyGroup::Strip,
    ),
    body(
        "led-strip-rgbcct-2000",
        "LED strip, 2000 mm",
        BodyGroup::Strip,
    ),
    body(
        "led-strip-rgbcct-2500",
        "LED strip, 2500 mm",
        BodyGroup::Strip,
    ),
    body(
        "led-strip-rgbcct-3000",
        "LED strip, 3000 mm",
        BodyGroup::Strip,
    ),
    body("strobe-xenon", "Xenon strobe", BodyGroup::Strobe),
    body("led-strobe", "LED strobe", BodyGroup::Strobe),
    body("hazer", "Hazer", BodyGroup::Atmospheric),
    body("show-laser", "Show laser", BodyGroup::Av),
    body("projector-small", "Projector, small", BodyGroup::Av),
    body("projector-large", "Projector, large", BodyGroup::Av),
];

/// The catalogue entry this name refers to, if the name is one this build ships.
///
/// A name that is not in the catalogue is not an error: a show may have been written by a newer
/// build, and the fixture falls back to the guess rather than refusing to draw.
pub fn body_model(id: &str) -> Option<&'static BodyModel> {
    BODY_CATALOGUE.iter().find(|model| model.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_body_has_a_unique_name_and_a_label() {
        let mut seen = HashSet::new();
        for model in BODY_CATALOGUE {
            assert!(seen.insert(model.id), "{} is listed twice", model.id);
            assert!(!model.label.is_empty(), "{} has no label", model.id);
            assert!(
                !model.id.contains("-no-clamp"),
                "{} names the unrigged variant; the catalogue names the body",
                model.id
            );
        }
    }

    /// The picker reads as a grouped list, so a group's entries have to be contiguous.
    #[test]
    fn groups_are_offered_in_one_run_each() {
        let mut finished = HashSet::new();
        let mut current: Option<BodyGroup> = None;
        for model in BODY_CATALOGUE {
            if current == Some(model.group) {
                continue;
            }
            if let Some(previous) = current {
                assert!(
                    finished.insert(previous.label()),
                    "{} is split into two runs",
                    previous.label()
                );
            }
            current = Some(model.group);
        }
    }
}
