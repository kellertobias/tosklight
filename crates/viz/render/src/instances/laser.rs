//! Turning a laser's scan path into drawable geometry.
//!
//! The scan engine hands over a path in a normalised square: deflections in `-1..=1`, a colour per
//! point, and the share of one complete scan spent reaching it. Everything in this module is the
//! step from that to rays in the room — where each run of the path starts and ends, how wide the
//! beam is along it, and how bright it should be.
//!
//! # The two things there are to see
//!
//! A laser puts a figure on whatever it hits *and* fills the air between with light, and those are
//! different pictures made of the same geometry. The figure is the path across the landing
//! surface. The light in the air is the sheet the beam sweeps between the window and that path —
//! the fan an audience sees over their heads, and the reason a laser is worth hanging at all.
//!
//! The sheet is drawn by sampling it: a run's fan is filled with rays from the window, spaced
//! closer together than a beam is wide so their haloes overlap into one surface, and each carrying
//! its share of the run's light so the sheet's brightness does not depend on how many rays it took
//! to fill.
//!
//! # Why dwell decides brightness
//!
//! A scanner moves at a fixed speed, so a run the beam crosses quickly gets less light per metre
//! than one it lingers on. That is not a subtlety: it is why the corners of a real laser figure
//! are visibly brighter than its edges, why a small figure is brighter than a large one drawn at
//! the same speed, and why a pattern with a lot of blanked travel is dimmer than one without. A
//! renderer that gave every run the same brightness would draw wireframes, not lasers.

use super::{FLOOR_HEIGHT, LaserInstance};
use glam::{Quat, Vec3};
use std::collections::HashMap;
use viz_scene::{LaserOptics, LaserScan, ScanPoint};

/// How far a laser reaches before it is treated as lost in the air.
///
/// Longer than the beam throw a lantern gets, because a laser genuinely does keep going: a shot
/// that leaves the rig without hitting anything is a real part of the look rather than a fixture
/// wasting light.
const LASER_THROW_METRES: f32 = 60.0;

/// How much wider than its physical radius a laser is drawn.
///
/// A 3 mm beam thirty metres away is a small fraction of one pixel, and drawing it truthfully
/// gives a shimmering dotted line that looks nothing like a laser. Real lasers read as thicker
/// than they are for the same reason — bloom in the eye and in every camera that has ever filmed
/// one — so the geometry is widened to a floor the display can actually resolve and the extra
/// width is paid for in brightness, not added to it.
const MINIMUM_DRAWN_RADIUS: f32 = 0.012;

/// A run whose two ends are closer together than this is dropped.
///
/// Scan engines emit repeated points freely — as a way of dwelling on a corner, or simply as an
/// artefact of generating a path — and a zero-length run is both invisible and a degenerate
/// billboard whose orientation is undefined. A whole scan held at one point is handled separately
/// as one stationary beam, rather than adding a beam for every repeated corner.
const MINIMUM_RUN_METRES: f32 = 1e-4;

/// How far apart the rays filling a run's fan may be, in beam radii where they land.
///
/// The rays have to overlap or the sheet reads as the separate shots of a comb rather than as one
/// surface. Each is drawn soft and several radii wide, so a spacing inside that closes the gaps
/// while keeping the count down.
const FAN_SPACING_RADII: f32 = 2.0;

/// The most rays one run's fan is filled with.
///
/// A run across the whole scan field at thirty metres is a wide fan, and without a ceiling a
/// pattern of a few long throws could ask for more geometry than the rest of the rig put together.
const MAXIMUM_FAN_RAYS: usize = 32;

/// How much of a run's light is drawn as the beam in the air rather than as the figure it lands on.
///
/// The two are not alternatives — one beam does both — but they are nothing like the same
/// brightness. What reaches an eye from the side is the sliver of the beam that haze happens to
/// scatter its way, while the figure is the whole beam stopped dead by something solid. A fan
/// drawn at anything near the strength of its own figure swamps the rig it is hanging in.
const AIR_SHARE: f32 = 0.05;

/// Upper bound already used for moving runs that dwell on a very short part of the figure.
/// A stationary point is the limiting case: its share of the scan stays inside one beam footprint.
const MAXIMUM_SCAN_DENSITY: f32 = 8.0;

/// Append every visible run of one laser's scan.
///
/// `intensity` is the head's level after persistence of vision, which for a laser is doing real
/// work: a scanner completes hundreds of passes between two displayed frames, so what an observer
/// has is the accumulation, not the instant.
pub(super) fn push_laser(
    out: &mut Vec<LaserInstance>,
    origin: Vec3,
    orientation: Quat,
    optics: &LaserOptics,
    scan: &LaserScan,
    intensity: f32,
    colour_multiplier: Vec3,
) {
    if scan.points.is_empty() || intensity <= 0.002 {
        return;
    }
    // How many complete passes of this figure land inside one displayed frame. A fast scanner
    // drawing a small figure repeats it many times and the figure reads as solid; the same scanner
    // on a figure ten times longer repeats it a tenth as often and visibly dims. That ratio is the
    // whole reason the scanner's speed is part of the fixture profile.
    let gain = intensity * optics.optical_power_watts.clamp(0.05, 20.0);

    // Where the beam lands for every control point, resolved once: the runs need both ends, and
    // the whole path's length has to be known before any run can be told how bright it is.
    let landings: Vec<(Vec3, f32)> = scan
        .points
        .iter()
        .map(|point| {
            let direction = deflect(orientation, optics, point);
            let distance = throw(origin, direction);
            (origin + direction * distance, distance)
        })
        .collect();

    let whole_scan_stationary = landings
        .iter()
        .all(|(point, _)| point.distance(landings[0].0) < MINIMUM_RUN_METRES);
    let lit_motion = scan
        .points
        .windows(2)
        .zip(landings.windows(2))
        .any(|(points, ends)| {
            Vec3::from(points[1].colour).max_element() > 0.002
                && points[1].dwell > 0.0
                && ends[1].0.distance(ends[0].0) >= MINIMUM_RUN_METRES
        });
    if whole_scan_stationary || !lit_motion {
        // A dotted fan moves dark to each coordinate, then dwells there with its light on.
        // Aggregate each held position once; dark travel must never become a connecting fan.
        // This path is deliberately exclusive to scans without visible movement, so ordinary
        // repeated corners retain their existing run geometry and brightness.
        let mut by_position = HashMap::new();
        let mut held: Vec<(Vec3, f32, Vec3)> = Vec::new();
        for (index, point) in scan.points.iter().enumerate() {
            let (landing, distance) = landings[index];
            if !whole_scan_stationary
                && (index == 0 || landing.distance(landings[index - 1].0) >= MINIMUM_RUN_METRES)
            {
                continue;
            }
            let colour = Vec3::from(point.colour);
            if !landing.is_finite()
                || !colour.is_finite()
                || !point.dwell.is_finite()
                || point.dwell <= 0.0
                || colour.max_element() <= 0.002
            {
                continue;
            }
            // Quantise only below the existing minimum drawable run; this also merges signed
            // zero while keeping grouping linear in the number of scan points.
            let key = (
                (landing.x / MINIMUM_RUN_METRES).round() as i64,
                (landing.y / MINIMUM_RUN_METRES).round() as i64,
                (landing.z / MINIMUM_RUN_METRES).round() as i64,
            );
            let at = *by_position.entry(key).or_insert_with(|| {
                held.push((landing, distance, Vec3::ZERO));
                held.len() - 1
            });
            held[at].2 += colour.clamp(Vec3::ZERO, Vec3::ONE) * point.dwell;
        }
        for (landing, distance, colour_dwell) in held {
            let radiance = colour_dwell * colour_multiplier * gain * MAXIMUM_SCAN_DENSITY;
            if radiance.is_finite() && radiance.max_element() > 0.002 {
                push_stationary(out, origin, landing, distance, optics, radiance);
            }
        }
        return;
    }

    // The total length of everything actually drawn, which is what each run's share is measured
    // against. Using the path's own length rather than an absolute figure is what keeps a small
    // figure and a large one at comparable brightness per unit of dwell: the scanner puts the same
    // light into both, and a run's share of that is what decides how bright it is. An absolute
    // light-per-metre would instead make every large pattern nearly invisible and every tight one
    // blinding, and would swing with the point count as well.
    let drawn_length: f32 = scan
        .points
        .windows(2)
        .zip(landings.windows(2))
        .filter(|(points, _)| Vec3::from(points[1].colour).max_element() > 0.002)
        .map(|(_, ends)| (ends[1].0 - ends[0].0).length())
        .sum();
    if drawn_length <= f32::EPSILON {
        return;
    }

    for (index, point) in scan.points.iter().enumerate().skip(1) {
        let (from, from_distance) = landings[index - 1];
        let (to, to_distance) = landings[index];
        let colour = Vec3::from(point.colour).clamp(Vec3::ZERO, Vec3::ONE) * colour_multiplier;
        // A control point carries the colour and the dwell of the run that *arrives* at it, not of
        // the one that leaves it. That is the ILDA convention every scan engine is written
        // against, and it is what makes blanking expressible at all: a black point means the
        // scanner travelled to it dark. Taking the brighter of the two ends instead would draw
        // every jump between figures as a lit line, joining every corner of a pattern to every
        // other one.
        let dwell = point.dwell.max(0.0);
        if colour.max_element() <= 0.002 || dwell <= 0.0 {
            continue;
        }
        let length = (to - from).length();
        if length < MINIMUM_RUN_METRES {
            continue;
        }
        // A run's share of the scan's time against its share of the scan's length. Exactly `1.0`
        // for a run travelled at the figure's average speed, above it wherever the scanner is
        // moving slower than average, below it where it is moving faster. That ratio is the
        // corner-brightness effect falling out of the geometry rather than being painted on, and
        // being a ratio it neither changes with the size of the figure nor with how many points a
        // script chose to use.
        //
        // The same number serves the beam in the air, because it is the same light: the run and
        // the sheet that swept it are one pass of the scanner seen from two places.
        let density = (dwell * drawn_length / length.max(0.01)).min(MAXIMUM_SCAN_DENSITY);
        let radiance = colour * gain * density;
        push_run(out, from, to, from_distance, to_distance, optics, radiance);
        push_fan(
            out,
            origin,
            from,
            to,
            from_distance,
            to_distance,
            optics,
            radiance * AIR_SHARE,
        );
    }
}

/// One held beam and, only where it actually meets the floor, its small landing footprint.
///
/// The existing native primitive is a ribbon, whose axis must not be zero length. Its airborne
/// ray already has the full throw as an axis; the footprint uses one beam diameter across the
/// floor. This is the spot's finite physical width, not a fabricated scan movement.
fn push_stationary(
    out: &mut Vec<LaserInstance>,
    origin: Vec3,
    landing: Vec3,
    distance: f32,
    optics: &LaserOptics,
    radiance: Vec3,
) {
    // Unlike a fan sample, the scanner stays on this ray: it has a hot core as well as a halo.
    // Kind 2 retains airborne haze attenuation; kind 1 would falsely treat it as a surface.
    push_ray(
        out,
        origin,
        landing,
        distance,
        optics,
        radiance * AIR_SHARE,
        2.0,
    );
    // A beam that leaves the room upwards ends at the display's throw limit, not on a surface.
    // Do not invent a bright dot hanging sixty metres in the air at that limit.
    if landing.y < origin.y && (landing.y - FLOOR_HEIGHT).abs() < 1e-3 {
        let radius = drawn_radius(optics, distance);
        push_run(
            out,
            landing - Vec3::X * radius,
            landing + Vec3::X * radius,
            distance,
            distance,
            optics,
            radiance,
        );
    }
}

/// Fill the sheet one run sweeps between the window and the path it draws.
///
/// The beam is at the near end of this the whole time and at the far end for none of it, so what
/// is in the air is a triangle rather than a shaft. It is drawn as rays from the window through
/// the run, each one a beam in its own right and each carrying its share of the run's light.
///
/// The rays converge at the window, so the sheet is brightest where the beam is most concentrated
/// and thins out towards the figure. That is not an artefact of sampling it this way — it is the
/// shape of the light, and the reason the apex of a laser fan is the part that hurts to look at.
#[allow(clippy::too_many_arguments)]
fn push_fan(
    out: &mut Vec<LaserInstance>,
    origin: Vec3,
    from: Vec3,
    to: Vec3,
    from_distance: f32,
    to_distance: f32,
    optics: &LaserOptics,
    radiance: Vec3,
) {
    // How far apart the two edges of the fan are where they land, against how wide the beam is
    // there: a run the scanner barely moved through is one ray, and a sweep across the room is
    // filled until the rays touch.
    let far_radius = drawn_radius(optics, from_distance.max(to_distance));
    let rays = (((to - from).length() / (far_radius * FAN_SPACING_RADII)).ceil() as usize)
        .clamp(1, MAXIMUM_FAN_RAYS);
    // Every ray is as bright as the run it fills, not a share of it, because the run's brightness
    // is already light per unit of the figure rather than a total: a wide sweep is dimmer per
    // metre than a tight one and gets proportionally more rays to fill, so the two cancel. Handing
    // each ray a divided share instead would make a sheet brighter the more coarsely a script
    // happened to draw the figure that casts it.
    for ray in 0..rays {
        // Sampled at the middle of each ray's share of the run rather than at its edge, so a
        // one-ray fan is the run's own middle instead of one of its ends.
        let across = (ray as f32 + 0.5) / rays as f32;
        let landing = from.lerp(to, across);
        let distance = from_distance + (to_distance - from_distance) * across;
        push_ray(out, origin, landing, distance, optics, radiance, 0.0);
    }
}

/// A fan sample (kind 0) or a held beam (kind 2), both attenuated by the haze they travel through.
fn push_ray(
    out: &mut Vec<LaserInstance>,
    origin: Vec3,
    landing: Vec3,
    distance: f32,
    optics: &LaserOptics,
    radiance: Vec3,
    kind: f32,
) {
    out.push(LaserInstance {
        start_radius: origin.extend(drawn_radius(optics, 0.0)).to_array(),
        end_radius: landing.extend(drawn_radius(optics, distance)).to_array(),
        colour_landing: radiance.extend(kind).to_array(),
    });
}

/// How wide the beam is drawn at a distance: its physical radius, or the floor below which a
/// display cannot resolve it, whichever is larger.
fn drawn_radius(optics: &LaserOptics, distance: f32) -> f32 {
    optics.radius_at(distance).max(MINIMUM_DRAWN_RADIUS)
}

/// Where one control point sends the beam, in world space.
///
/// The scanner deflects about the emitter's own axes, so the figure turns with the fixture: a
/// laser rigged upside down draws its pattern upside down, exactly as the real one would.
fn deflect(orientation: Quat, optics: &LaserOptics, point: &ScanPoint) -> Vec3 {
    let x = point.x.clamp(-1.0, 1.0) * optics.scan_half_angle_x;
    let y = point.y.clamp(-1.0, 1.0) * optics.scan_half_angle_y;
    // An emitter rests aiming along local `-Y`, the same convention every other head uses. A turn
    // about local `Z` sweeps that across the figure's width and a turn about local `X` up its
    // height, which is what makes a script's `x` horizontal and its `y` vertical.
    let local = Quat::from_rotation_z(x) * Quat::from_rotation_x(y) * Vec3::NEG_Y;
    (orientation * local).normalize_or(orientation * Vec3::NEG_Y)
}

/// How far the beam travels before it lands, or the distance at which it is given up as lost.
fn throw(origin: Vec3, direction: Vec3) -> f32 {
    if direction.y >= -1e-3 {
        return LASER_THROW_METRES;
    }
    let to_floor = (FLOOR_HEIGHT - origin.y) / direction.y;
    if to_floor <= 0.0 {
        return LASER_THROW_METRES;
    }
    to_floor.clamp(0.05, LASER_THROW_METRES)
}

/// The figure itself: the path the beam draws across whatever it lands on.
fn push_run(
    out: &mut Vec<LaserInstance>,
    from: Vec3,
    to: Vec3,
    from_distance: f32,
    to_distance: f32,
    optics: &LaserOptics,
    radiance: Vec3,
) {
    out.push(LaserInstance {
        start_radius: from.extend(drawn_radius(optics, from_distance)).to_array(),
        end_radius: to.extend(drawn_radius(optics, to_distance)).to_array(),
        colour_landing: radiance.extend(1.0).to_array(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn optics() -> LaserOptics {
        LaserOptics {
            scan_half_angle_x: 0.4,
            scan_half_angle_y: 0.4,
            ..LaserOptics::default()
        }
    }

    fn scan(points: Vec<ScanPoint>) -> LaserScan {
        LaserScan {
            points,
            points_per_second: 30_000.0,
            slots: Vec::new(),
            fault: None,
        }
    }

    fn lit(x: f32, y: f32, dwell: f32) -> ScanPoint {
        ScanPoint {
            x,
            y,
            colour: [1.0, 1.0, 1.0],
            dwell,
        }
    }

    fn dark(x: f32, y: f32, dwell: f32) -> ScanPoint {
        ScanPoint {
            x,
            y,
            colour: [0.0, 0.0, 0.0],
            dwell,
        }
    }

    const WINDOW: Vec3 = Vec3::new(0.0, 6.0, 0.0);

    fn drawn(points: Vec<ScanPoint>) -> Vec<LaserInstance> {
        let mut out = Vec::new();
        push_laser(
            &mut out,
            WINDOW,
            Quat::IDENTITY,
            &optics(),
            &scan(points),
            1.0,
            Vec3::ONE,
        );
        out
    }

    /// The figure the beam draws on whatever it lands on.
    fn run(points: Vec<ScanPoint>) -> Vec<LaserInstance> {
        drawn(points)
            .into_iter()
            .filter(|instance| instance.colour_landing[3] == 1.0)
            .collect()
    }

    /// The beam on its way there, which starts at the window every time.
    fn air(points: Vec<ScanPoint>) -> Vec<LaserInstance> {
        drawn(points)
            .into_iter()
            .filter(|instance| instance.colour_landing[3] != 1.0)
            .collect()
    }

    /// A path of n points is n-1 runs, and the beam lands on the deck rather than running through
    /// it.
    #[test]
    fn a_path_becomes_one_run_between_each_pair_of_points() {
        let drawn = run(vec![
            lit(-1.0, 0.0, 0.5),
            lit(0.0, 0.0, 0.25),
            lit(1.0, 0.0, 0.25),
        ]);
        assert_eq!(drawn.len(), 2);
        for instance in &drawn {
            assert!(
                instance.end_radius[1] >= FLOOR_HEIGHT - 1e-3,
                "a run went through the floor"
            );
        }
    }

    /// The other half of the picture: what is in the air between the projector and the figure.
    /// Every ray of it leaves the window, or the fan is drawn hanging in space away from the
    /// fixture that is casting it.
    #[test]
    fn every_run_fills_the_air_between_the_window_and_where_it_lands() {
        let points = vec![lit(-1.0, 0.0, 0.5), lit(1.0, 0.0, 0.5)];
        let air = air(points.clone());
        assert!(!air.is_empty(), "the beam left nothing in the air");
        let landing = &run(points)[0];
        for ray in &air {
            let start = Vec3::from_slice(&ray.start_radius[..3]);
            let end = Vec3::from_slice(&ray.end_radius[..3]);
            assert!(
                start.abs_diff_eq(WINDOW, 1e-4),
                "a ray of the fan started at {start} rather than at the window"
            );
            // On the run it is filling: between its two ends and down on the deck with them.
            assert!((end.y - FLOOR_HEIGHT).abs() < 1e-3);
            assert!(
                end.x >= landing.start_radius[0].min(landing.end_radius[0]) - 1e-3
                    && end.x <= landing.start_radius[0].max(landing.end_radius[0]) + 1e-3,
                "a ray landed at {end}, off the run it belongs to"
            );
        }
    }

    /// How finely a script draws a figure is its own business, and it must not change how bright
    /// the sheet the figure casts comes out. A script that puts four points along a sweep instead
    /// of two is describing the same laser doing the same thing.
    #[test]
    fn filling_the_air_more_finely_does_not_make_it_brighter() {
        let brightest = |points: Vec<ScanPoint>| {
            air(points)
                .iter()
                .map(|ray| ray.colour_landing[0])
                .fold(0.0_f32, f32::max)
        };
        // The same sweep, once as one run and once as four. Kept near the middle of the scan
        // field, where the deflection is close enough to linear that the four are the same length
        // as each other: further out they are not, and that difference is real rather than an
        // artefact of the sampling.
        let coarse = brightest(vec![lit(-0.2, 0.0, 0.0), lit(0.2, 0.0, 1.0)]);
        let fine = brightest(vec![
            lit(-0.2, 0.0, 0.0),
            lit(-0.1, 0.0, 0.25),
            lit(0.0, 0.0, 0.25),
            lit(0.1, 0.0, 0.25),
            lit(0.2, 0.0, 0.25),
        ]);
        assert!(
            coarse > 0.0 && (coarse - fine).abs() < coarse * 0.02,
            "the same sweep lit the air at {coarse} drawn coarsely and {fine} drawn finely"
        );
    }

    /// A wider sweep is filled with more rays, or the sheet a big figure casts is drawn as a comb
    /// of separate shots while a small one is solid.
    #[test]
    fn a_wider_sweep_is_filled_with_more_rays() {
        let narrow = air(vec![lit(-0.05, 0.0, 0.5), lit(0.05, 0.0, 0.5)]).len();
        let wide = air(vec![lit(-0.5, 0.0, 0.5), lit(0.5, 0.0, 0.5)]).len();
        assert!(
            wide > narrow * 2,
            "a sweep ten times wider was filled with {wide} rays against {narrow}"
        );
    }

    /// The whole reason blanking exists. A scanner jumping between two figures with the light off
    /// must leave no trace, or every pattern is drawn joined to the next.
    #[test]
    fn a_blanked_run_draws_nothing() {
        let drawn = run(vec![
            lit(-1.0, 0.0, 0.4),
            dark(1.0, 0.0, 0.2),
            lit(1.0, 0.5, 0.4),
        ]);
        assert_eq!(
            drawn.len(),
            1,
            "the blanked travel between two figures was drawn"
        );
    }

    /// Why a real laser's corners are bright: the same dwell over a shorter run is more light per
    /// metre. Without this a figure reads as a uniform wireframe.
    #[test]
    fn a_run_the_scanner_lingers_on_is_brighter_per_metre() {
        let drawn = run(vec![
            lit(-1.0, 0.0, 0.0),
            // A long sweep and a short one given the same share of the scan.
            lit(1.0, 0.0, 0.5),
            lit(0.95, 0.0, 0.5),
        ]);
        assert_eq!(drawn.len(), 2);
        let long = drawn[0].colour_landing[0];
        let short = drawn[1].colour_landing[0];
        assert!(
            short > long,
            "the short run ({short}) was not brighter than the long one ({long})"
        );
    }

    /// A dark laser and an empty scan draw nothing at all.
    #[test]
    fn nothing_is_drawn_without_light_or_without_a_path() {
        let mut out = Vec::new();
        push_laser(
            &mut out,
            Vec3::new(0.0, 6.0, 0.0),
            Quat::IDENTITY,
            &optics(),
            &scan(vec![lit(-1.0, 0.0, 0.5), lit(1.0, 0.0, 0.5)]),
            0.0,
            Vec3::ONE,
        );
        assert!(out.is_empty(), "a laser at zero drew something");
        assert!(run(Vec::new()).is_empty());
    }

    /// The deflection has to turn with the fixture, or a laser rigged at an angle would draw its
    /// figure level with the world.
    #[test]
    fn the_figure_turns_with_the_fixture() {
        let points = vec![lit(-1.0, 0.0, 0.5), lit(1.0, 0.0, 0.5)];
        let figure = |orientation: Quat| {
            let mut out = Vec::new();
            push_laser(
                &mut out,
                WINDOW,
                orientation,
                &optics(),
                &scan(points.clone()),
                1.0,
                Vec3::ONE,
            );
            out.retain(|instance| instance.colour_landing[3] == 1.0);
            out
        };
        let upright = figure(Quat::IDENTITY);
        let rolled = figure(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        assert_eq!(upright.len(), 1);
        assert_eq!(rolled.len(), 1);
        // Swept across X when upright, across Z once the fixture is turned a quarter turn.
        assert!(upright[0].start_radius[0].abs() > upright[0].start_radius[2].abs());
        assert!(rolled[0].start_radius[2].abs() > rolled[0].start_radius[0].abs());
    }

    /// Divergence opens the beam up down its throw, so a long shot is drawn wider at the far end.
    #[test]
    fn a_long_shot_is_wider_where_it_lands() {
        let wide = LaserOptics {
            divergence: 0.02,
            ..optics()
        };
        let mut out = Vec::new();
        push_laser(
            &mut out,
            Vec3::new(0.0, 20.0, 0.0),
            Quat::IDENTITY,
            &wide,
            &scan(vec![lit(0.0, 0.0, 0.5), lit(0.0, 0.0, 0.5)]),
            1.0,
            Vec3::ONE,
        );
        assert!(wide.radius_at(20.0) > wide.radius_at(1.0));
        let ray = out
            .iter()
            .find(|instance| instance.colour_landing[3] == 2.0)
            .expect("a stationary beam must be drawn");
        assert!(ray.end_radius[3] > ray.start_radius[3]);
    }

    #[test]
    fn a_single_point_draws_one_beam_and_one_finite_landing_spot() {
        let instances = drawn(vec![lit(0.0, 0.0, 1.0)]);
        assert_eq!(instances.len(), 2);
        let beam = &instances[0];
        assert_eq!(beam.colour_landing[3], 2.0);
        assert!(Vec3::from_slice(&beam.start_radius).abs_diff_eq(WINDOW, 1e-6));
        assert!((beam.end_radius[1] - FLOOR_HEIGHT).abs() < 1e-3);
        let spot = &instances[1];
        assert_eq!(spot.colour_landing[3], 1.0);
        assert!((spot.start_radius[1] - FLOOR_HEIGHT).abs() < 1e-3);
        assert!((spot.end_radius[1] - FLOOR_HEIGHT).abs() < 1e-3);
        for instance in &instances {
            assert!(
                instance
                    .start_radius
                    .iter()
                    .chain(&instance.end_radius)
                    .all(|n| n.is_finite())
            );
            assert!(
                Vec3::from_slice(&instance.start_radius)
                    .distance(Vec3::from_slice(&instance.end_radius))
                    > MINIMUM_RUN_METRES
            );
        }
    }

    #[test]
    fn repeating_a_stationary_point_shares_one_beam_instead_of_duplicating_it() {
        let single = drawn(vec![lit(0.3, -0.2, 1.0)]);
        let repeated = drawn(vec![lit(0.3, -0.2, 0.25), lit(0.3, -0.2, 0.75)]);
        assert_eq!(single.len(), repeated.len());
        for (one, many) in single.iter().zip(&repeated) {
            assert_eq!(one.start_radius, many.start_radius);
            assert_eq!(one.end_radius, many.end_radius);
            assert_eq!(one.colour_landing, many.colour_landing);
        }
    }

    #[test]
    fn stationary_colour_and_blanking_are_weighted_by_the_actual_dwell() {
        let mut red = lit(0.0, 0.0, 0.25);
        red.colour = [1.0, 0.0, 0.0];
        let mut blue = lit(0.0, 0.0, 0.25);
        blue.colour = [0.0, 0.0, 1.0];
        let instances = drawn(vec![red, dark(0.0, 0.0, 0.5), blue]);
        assert_eq!(instances.len(), 2);
        let full = drawn(vec![lit(0.0, 0.0, 1.0)]);
        for (mixed, white) in instances.iter().zip(&full) {
            assert!((mixed.colour_landing[0] - white.colour_landing[0] * 0.25).abs() < 1e-6);
            assert_eq!(mixed.colour_landing[1], 0.0);
            assert!((mixed.colour_landing[2] - white.colour_landing[2] * 0.25).abs() < 1e-6);
        }
        assert!(drawn(vec![dark(0.0, 0.0, 1.0)]).is_empty());
        assert!(drawn(vec![lit(0.0, 0.0, 0.0), lit(0.0, 0.0, -1.0)]).is_empty());
        assert!(drawn(vec![lit(0.0, 0.0, f32::NAN)]).is_empty());
        let mut out = Vec::new();
        push_laser(
            &mut out,
            WINDOW,
            Quat::IDENTITY,
            &optics(),
            &scan(vec![lit(0.0, 0.0, 1.0)]),
            0.0,
            Vec3::ONE,
        );
        push_laser(
            &mut out,
            WINDOW,
            Quat::IDENTITY,
            &optics(),
            &scan(vec![lit(0.0, 0.0, 1.0)]),
            1.0,
            Vec3::ZERO,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn an_upward_stationary_beam_follows_the_fixture_without_an_airborne_landing_dot() {
        let mut out = Vec::new();
        push_laser(
            &mut out,
            WINDOW,
            Quat::from_rotation_x(std::f32::consts::PI),
            &optics(),
            &scan(vec![lit(0.0, 0.0, 1.0)]),
            1.0,
            Vec3::ONE,
        );
        assert_eq!(out.len(), 1);
        let beam = &out[0];
        assert_eq!(beam.colour_landing[3], 2.0);
        assert!((beam.end_radius[1] - (WINDOW.y + LASER_THROW_METRES)).abs() < 1e-3);
        assert!((Vec3::from_slice(&beam.end_radius) - WINDOW).length() > 59.9);
    }

    #[test]
    fn repeated_corners_do_not_add_stationary_beams_to_an_ordinary_path() {
        let plain = drawn(vec![
            lit(-0.5, 0.0, 0.0),
            lit(0.0, 0.0, 0.3),
            lit(0.5, 0.0, 0.5),
        ]);
        let repeated = drawn(vec![
            lit(-0.5, 0.0, 0.0),
            lit(0.0, 0.0, 0.3),
            lit(0.0, 0.0, 0.2),
            lit(0.5, 0.0, 0.5),
        ]);
        assert_eq!(plain.len(), repeated.len());
        for (one, many) in plain.iter().zip(&repeated) {
            assert_eq!(one.start_radius, many.start_radius);
            assert_eq!(one.end_radius, many.end_radius);
            assert_eq!(one.colour_landing, many.colour_landing);
        }
        // The first control point does not light an otherwise blanked journey to another point.
        assert!(drawn(vec![lit(-0.5, 0.0, 0.5), dark(0.5, 0.0, 0.5)]).is_empty());
    }

    #[test]
    fn blank_travel_and_lit_dwells_draw_separate_point_beams_without_connecting_fans() {
        let points = vec![
            dark(-0.5, 0.0, 0.1),
            lit(-0.5, 0.0, 0.15),
            lit(-0.5, 0.0, 0.05),
            dark(0.0, 0.0, 0.1),
            lit(0.0, 0.0, 0.2),
            dark(0.5, 0.0, 0.1),
            lit(0.5, 0.0, 0.3),
        ];
        let instances = drawn(points);
        let beams: Vec<_> = instances
            .iter()
            .filter(|i| i.colour_landing[3] == 2.0)
            .collect();
        assert_eq!(
            beams.len(),
            3,
            "each unique lit dwell needs exactly one hot beam"
        );
        assert!(
            !instances.iter().any(|i| i.colour_landing[3] == 0.0),
            "blank travel cannot produce a connecting fan"
        );
        assert_eq!(
            instances
                .iter()
                .filter(|i| i.colour_landing[3] == 1.0)
                .count(),
            3
        );
        for beam in &beams {
            assert!(Vec3::from_slice(&beam.start_radius).abs_diff_eq(WINDOW, 1e-6));
            assert!(
                Vec3::from_slice(&beam.start_radius).distance(Vec3::from_slice(&beam.end_radius))
                    > 1.0
            );
        }
        assert!(
            (beams[0].colour_landing[0] - beams[1].colour_landing[0]).abs() < 1e-6,
            "repeated same-point dwells must combine their time, not duplicate geometry"
        );
        assert!((beams[2].colour_landing[0] - beams[1].colour_landing[0] * 1.5).abs() < 1e-6);
        assert!(
            drawn(vec![
                dark(-0.5, 0.0, 0.5),
                dark(-0.5, 0.0, 0.1),
                dark(0.5, 0.0, 0.4)
            ])
            .is_empty()
        );
    }

    #[test]
    #[ignore = "requires a native GPU adapter; run explicitly for laser capture verification"]
    fn stationary_beam_reaches_the_native_pixel_pipeline() {
        use viz_scene::{
            EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, EmitterValues,
            FixtureInstance, Scene, SceneValues, ViewConfiguration,
        };
        let mut scene = Scene::default();
        scene.fixtures.push(FixtureInstance {
            position: Vec3::new(0.0, 1.0, 0.0),
            installed_colour: [1.0; 3],
            patched: true,
            ..Default::default()
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Stationary laser".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::new(180.0, 0.0, 0.0),
            pan: None,
            tilt: None,
            beam_angle_degrees: 1.0,
            field_angle_degrees: 1.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Laser,
            cells: EmitterLayoutCells::single(),
            laser: Some(optics()),
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
        scene.recompute_bounds();
        let mut values = SceneValues {
            emitters: vec![EmitterValues {
                intensity: 1.0,
                colour: [1.0; 3],
                ..Default::default()
            }],
            laser_scans: vec![scan(vec![lit(0.0, 0.0, 1.0)])],
            ..Default::default()
        };
        values.atmosphere.density = 0.16;
        let mut view = ViewConfiguration::default();
        view.camera.position = Vec3::new(8.0, 4.0, 8.0);
        view.camera.target = Vec3::new(0.0, 5.0, 0.0);
        view.ambient = 0.0;
        view.show_labels = false;
        let mut renderer = crate::Renderer::headless(512, 512).expect("native GPU adapter");
        let image = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        values.emitters[0].intensity = 0.0;
        let dark_image = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        let changed = image
            .rgba
            .chunks_exact(4)
            .zip(dark_image.rgba.chunks_exact(4))
            .filter(|(on, off)| (0..3).any(|i| i16::from(on[i]) - i16::from(off[i]) > 8))
            .count();
        eprintln!("Stationary native beam changed {changed} pixels");
        if let Some(directory) = std::env::var_os("VIZ_LASER_CAPTURE_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            let mut ppm = b"P6\n512 512\n255\n".to_vec();
            for pixel in image.rgba.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(
                std::path::Path::new(&directory).join("stationary-native.ppm"),
                ppm,
            )
            .unwrap();
        }
        assert!(
            changed > 100,
            "the native renderer lost the stationary laser despite valid scan geometry"
        );

        values.emitters[0].intensity = 1.0;
        values.atmosphere.density = 0.0;
        let clear = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        let brightness = |image: &crate::CapturedImage| {
            image
                .rgba
                .chunks_exact(4)
                .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
                .sum::<u64>()
        };
        assert!(
            brightness(&image) > brightness(&clear),
            "kind2 is airborne and must still answer haze"
        );
        values.atmosphere.density = 0.16;
        values.laser_scans[0] = scan(vec![
            dark(-0.8, 0.0, 1.0 / 6.0),
            lit(-0.8, 0.0, 1.0 / 6.0),
            dark(0.0, 0.0, 1.0 / 6.0),
            lit(0.0, 0.0, 1.0 / 6.0),
            dark(0.8, 0.0, 1.0 / 6.0),
            lit(0.8, 0.0, 1.0 / 6.0),
        ]);
        let fan = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        let fan_pixels = fan
            .rgba
            .chunks_exact(4)
            .zip(dark_image.rgba.chunks_exact(4))
            .filter(|(on, off)| (0..3).any(|i| i16::from(on[i]) - i16::from(off[i]) > 8))
            .count();
        eprintln!("Stationary point fan changed {fan_pixels} native pixels");
        assert!(
            fan_pixels > 200,
            "blank-travel point fan must reach the native pixel pipeline"
        );
        if let Some(directory) = std::env::var_os("VIZ_LASER_CAPTURE_DIR") {
            let mut ppm = b"P6\n512 512\n255\n".to_vec();
            for pixel in fan.rgba.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(
                std::path::Path::new(&directory).join("stationary-point-fan-native.ppm"),
                ppm,
            )
            .unwrap();
        }
    }
}
