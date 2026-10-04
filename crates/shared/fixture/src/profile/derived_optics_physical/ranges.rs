//! Zoom degree ranges: documented ranges of known shipped profiles, else the nominal range of
//! the fixture type (the cone the Stage already draws for an undescribed Zoom).
use super::super::{OpeningConvention, PhysicalDataQuality};
use super::DERIVED_ZOOM_SOURCE;
use uuid::Uuid;

/// Where the degrees sit along the function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ZoomTravel {
    /// Nominal: `narrow` at no travel (0 or 0 %), `wide` at full travel, so the profile's own
    /// direction (ascending or descending) is kept.
    Nominal { narrow: f32, wide: f32 },
    /// Documented: degrees at the function's first and last DMX value, as the manufacturer's
    /// chart orders them (whatever direction the profile's normalized travel was authored in).
    Dmx { at_from: f32, at_to: f32 },
}

/// A derived Zoom opening and its provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ZoomRange {
    pub travel: ZoomTravel,
    pub convention: OpeningConvention,
    pub quality: PhysicalDataQuality,
    pub source: &'static str,
}

impl ZoomRange {
    /// Degrees at the function's `physical_min` and `physical_max` travel within `0..=domain`.
    pub fn degrees(&self, (min, max): (f32, f32), domain: f32) -> (f32, f32) {
        match self.travel {
            ZoomTravel::Nominal { narrow, wide } => {
                let at = |travel: f32| narrow + (travel / domain).clamp(0.0, 1.0) * (wide - narrow);
                (at(min), at(max))
            }
            ZoomTravel::Dmx { at_from, at_to } => (at_from, at_to),
        }
    }
}

const fn nominal(narrow: f32, wide: f32) -> ZoomRange {
    ZoomRange {
        travel: ZoomTravel::Nominal { narrow, wide },
        convention: OpeningConvention::Beam,
        quality: PhysicalDataQuality::Estimated,
        source: DERIVED_ZOOM_SOURCE,
    }
}

const fn documented(
    (at_from, at_to): (f32, f32),
    convention: OpeningConvention,
    quality: PhysicalDataQuality,
    source: &'static str,
) -> ZoomRange {
    ZoomRange {
        travel: ZoomTravel::Dmx { at_from, at_to },
        convention,
        quality,
        source,
    }
}

use OpeningConvention::{Beam, Field};
use PhysicalDataQuality::{Estimated, Manufacturer};

/// Shipped profiles whose Zoom degrees are documented, by profile id. Manufacturer quality only
/// where a manufacturer-hosted document states the range, the convention and the direction.
const DOCUMENTED: [(&str, ZoomRange); 9] = [
    (
        "79e6cc1e-3031-68c2-29ec-026e0c28a505",
        documented(
            (45.0, 10.0),
            Beam,
            Manufacturer,
            "ROBE Robin DLS Profile user manual v1.3 (https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_DLS_Profile.pdf): beam angle 10° (gobo position) to 45° (free hole); DMX chart v1.0 (https://www.robe.cz/res/downloads/dmx_charts/Robin_DLSProfile_DMX_charts.pdf): Zoom from max. to min. beam angle; linear curve unverified",
        ),
    ),
    (
        "f4407d31-b677-c123-c7fc-41f78ff93056",
        documented(
            (5.5, 60.0),
            Beam,
            Manufacturer,
            "ROBE Robin DLF Wash user manual (https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_DLF_Wash.pdf): min. beam angle 5.5°, max. 60° (75° only with the separate Wide zoom channel on); DMX chart v1.3 (https://www.robe.cz/res/downloads/dmx_charts/Robin_DLF_Wash_DMX_charts.pdf): Zoom from min. to max. beam angle; the product page also lists 8°–55°; linear curve unverified",
        ),
    ),
    (
        "7dce29ad-3051-e2be-3004-58d56d4c9a69",
        documented(
            (60.0, 3.8),
            Beam,
            Manufacturer,
            "ROBE Robin LEDBeam 150 (https://www.robe.cz/ledbeam-150, user manual https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_LEDBeam_150_FWQ_ETH.pdf): zoom range 3.8°–60°; DMX chart (https://www.robe.cz/res/downloads/dmx_charts/Robin_LEDBeam_150_150Q_150FW_150FWQ_RGBA_DMX_charts.pdf): Zoom from max. to min. beam angle; linear curve unverified",
        ),
    ),
    (
        "74bdcfeb-6e43-465d-9f5c-ac55fdd9859e",
        documented(
            (15.0, 60.0),
            Beam,
            Estimated,
            "Estimated from ROBE: product page (https://www.robe.cz/ledwash-300) says linear zoom 15–60° but its specification list says 8°–63°; DMX chart (https://www.robe.cz/res/downloads/dmx_charts/Robin_300_LEDWash_DMX_charts.pdf): Zoom from min. to max. beam angle; endpoints and curve unverified",
        ),
    ),
    (
        "1d7c3c94-3f84-df94-04ef-faf0fc2daee9",
        documented(
            (8.0, 63.0),
            Beam,
            Estimated,
            "Estimated from the ROBE Robin 600X LEDWash user manual v1.4 (https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_600X_LEDWash.pdf): beam angle 8°–63° in Beam mode (15°–63° when the fixture menu selects Wash mode); Zoom from min. to max. beam angle; curve unverified",
        ),
    ),
    (
        "3ddb091b-9c99-90a1-59cc-a0ba0c7cbd7e",
        documented(
            (12.0, 36.0),
            Field,
            Estimated,
            "Estimated from the JB-Lighting JBLED A7 DMX protocol v1.4 (https://www.jb-lighting.de/download/DMX-Protokolle/JBLED_A7_DMX_Protocol.pdf): zoom 12°–36° measured at 1/10 peak (a field angle); the chart's order is the only hint that DMX 0 is 12°; curve unverified",
        ),
    ),
    (
        "3c7bcf5c-4706-dfb1-680a-e3f0e5a47652",
        documented(
            (4.0, 40.0),
            Beam,
            Estimated,
            "Estimated from the CHAUVET Professional COLORado 1 Solo user manual Rev8 (https://chauvetprofessional.com/wp-content/uploads/2018/01/COLORado_1_Solo_UM_Rev8.pdf): photometric beam angle 4°–40° (field 8°–55°); the chart gives Zoom only as 0–100%, so DMX 0 = narrow is assumed; curve unverified",
        ),
    ),
    (
        "a15bf57b-dc4c-53e3-9d82-725f0273b7bd",
        documented(
            (24.0, 16.0),
            Beam,
            Estimated,
            "Estimated from the Clay Paky Stage Zoom 1200 SV manual (doc 099550, a dealer-hosted copy: https://audiovias.com/descarga/251/clay-paky-manuales/6932/clay-paky-manual-stage-zoom-1200-sv-25-02-05.pdf): zoom lens 16°–24°, channel 7 0 = wide beam, 255 = narrow beam; beam/field not stated, Beam assumed; curve unverified",
        ),
    ),
    (
        "2c1886ae-eb8a-5339-9a29-1a776c3fb080",
        documented(
            (24.0, 16.0),
            Beam,
            Estimated,
            "Estimated from the Clay Paky Stage Zoom 1200 manual (a dealer-hosted copy: https://audiovias.com/descarga/251/clay-paky-manuales/6929/clay-paky-manual-stage-zoom-1200-audioviasl.pdf): zoom lens 16°–24°; direction (0 = wide) taken from the Stage Zoom 1200 SV manual's identical channel table; beam/field not stated, Beam assumed; curve unverified",
        ),
    ),
];

/// The range of a profile: documented when known, else the nominal range of its fixture type.
/// `None` for a fixture type without a beam opening (effects, atmosphere, machines, venue
/// objects, lasers).
pub(super) fn zoom_range(profile_id: Uuid, fixture_type: &[String]) -> Option<ZoomRange> {
    let has = |word: &str| fixture_type.iter().any(|w| w == word);
    let not_optical = [
        "venue", "effect", "pyro", "sparkler", "flame", "fogger", "hazer", "haze", "fog", "fan",
        "relay", "other", "laser", "lasers",
    ];
    if not_optical.iter().any(|w| has(w)) {
        return None;
    }
    if let Some((_, range)) = DOCUMENTED
        .iter()
        .find(|(id, _)| Uuid::parse_str(id).ok() == Some(profile_id))
    {
        return Some(*range);
    }
    // The Stage's nominal cones per fixture type (`viz_project::fallback::OpticalClass`).
    let (narrow, wide) = if [
        "blinder", "sunstrip", "audience", "strobe", "pixel", "strip",
    ]
    .iter()
    .any(|w| has(w))
    {
        (60.0, 110.0)
    } else if has("scanner") || has("mirror") {
        (8.0, 16.0)
    } else if has("beam") {
        (3.0, 8.0)
    } else if has("profile") || has("spot") || has("ellipsoidal") {
        (10.0, 32.0)
    } else if has("fresnel") || has("dimmer") {
        (12.0, 48.0)
    } else if has("par") || has("parcan") || has("acl") {
        (10.0, 26.0)
    } else if has("flood") || has("cyc") || has("groundrow") {
        (45.0, 90.0)
    } else if has("wash") {
        (18.0, 55.0)
    } else {
        (10.0, 32.0)
    };
    Some(nominal(narrow, wide))
}
