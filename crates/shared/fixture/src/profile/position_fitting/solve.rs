use super::*;
use crate::forward::Driver;

// Bounded numerical work. A previous valid branch normally converges in a few iterations;
// all phase seeds are considered unless an unchanged encoded pair is already optimal. Authored
// function combinations stay explicitly unsupported, never an unbounded per-frame search.
const MAX_FUNCTION_PAIRS: usize = 64;
const ITERATIONS: usize = 48;
const MAX_EVALUATIONS_PER_FIT: usize = 4096;
const ENCODED_MATCH_DEGREES: f64 = 0.05;
const RESIDUAL_TOLERANCE: f64 = 1e-5;

#[derive(Clone, Copy)]
pub(super) struct TargetGoal {
    pub anchor: [f64; 2],
    pub target: [f64; 3],
}
#[derive(Clone, Copy)]
struct WorldRay {
    target: [f64; 3],
    mount: R,
    /// False for a fixed axis: the minimizer never moves it.
    free: [bool; 2],
}
/// One verified native proposal of a Target solve.
#[derive(Clone, Copy)]
struct Candidate {
    writes: [Option<PositionControlWrite>; 2],
    error: f64,
    distance: f64,
}
impl Candidate {
    /// Within the encoded tolerance, continuity decides; otherwise the smaller ray error.
    fn improves(&self, best: &Self) -> bool {
        if self.error <= ENCODED_MATCH_DEGREES && best.error <= ENCODED_MATCH_DEGREES {
            self.distance < best.distance
        } else {
            self.error < best.error
        }
    }
}
fn seed_angles(seed: usize, first: [f64; 2], anchor: [f64; 2], bounds: [[f64; 2]; 2]) -> [f64; 2] {
    match seed {
        0 => first,
        1 => std::array::from_fn(|j| (bounds[j][0] + bounds[j][1]) * 0.5),
        _ => {
            let n = seed - 2;
            std::array::from_fn(|j| {
                let phase = if j == 0 { n / 4 } else { n % 4 } as f64;
                let span = (bounds[j][1] - bounds[j][0]).min(360.);
                nearest_equivalent(bounds[j][0] + span * phase / 4., anchor[j], bounds[j])
            })
        }
    }
}

impl CompiledPositionFitting {
    pub(super) fn solve_target(
        &self,
        index: usize,
        pair: [usize; 2],
        goal: TargetGoal,
        context: FitContext<'_>,
        ws: &mut PositionFitWorkspace,
        out: &mut PositionFitResult,
    ) {
        let TargetGoal { anchor, target } = goal;
        let FitContext { mount, .. } = context;
        let models = self.forward.fitting_axes();
        // Every absolute driver of an axis; a fixed axis offers only its held angle (no write).
        let options: [Vec<Option<&Driver>>; 2] = pair.map(|i| {
            let axis = &models[i];
            if axis.fixed.is_some() {
                vec![None]
            } else {
                axis.drivers
                    .iter()
                    .filter(|d| !d.velocity)
                    .map(Some)
                    .collect()
            }
        });
        if options[0].len().saturating_mul(options[1].len()) > MAX_FUNCTION_PAIRS {
            out.status = PositionFitStatus::SolverCapacity;
            return;
        }
        // With a fixed axis the beam sweeps one cone: aim as closely as it allows and report the
        // remaining error, instead of requiring an exact ray.
        let free = pair.map(|i| models[i].fixed.is_none());
        ws.trial_axes.copy_from_slice(&ws.axes);
        for j in 0..2 {
            ws.trial_axes[pair[j]] = Some(if free[j] {
                anchor[j]
            } else {
                ws.axes[pair[j]].unwrap_or(anchor[j])
            });
        }
        self.forward
            .evaluate_pose(&ws.trial_axes, mount, &mut ws.geometry, &mut ws.poses)
            .unwrap();
        if let Some(pose) = ws.poses[index].world {
            if unit(std::array::from_fn(|i| target[i] - pose.point([0.; 3])[i])).is_none() {
                out.status = PositionFitStatus::CoincidentTarget;
                return;
            }
        } else {
            out.status = PositionFitStatus::UnsupportedGeometry;
            return;
        }
        let mut best: Option<Candidate> = None;
        let mut failed_verification = PositionFitStatus::UnreachableTarget;
        'functions: for pd in &options[0] {
            for td in &options[1] {
                let drivers = [*pd, *td];
                let bounds: [[f64; 2]; 2] =
                    std::array::from_fn(|j| self.axis_bounds(pair[j], drivers[j]));
                let first = std::array::from_fn(|j| anchor[j].clamp(bounds[j][0], bounds[j][1]));
                // Include an alternate head branch and boundary phases, not only a turn-lift of
                // one atan2 result. Arbitrary pivots/neutral transforms need full ray fitting.
                for seed in 0..18 {
                    if ws.evaluations >= MAX_EVALUATIONS_PER_FIT {
                        out.search_limited = true;
                        break 'functions;
                    }
                    let initial = seed_angles(seed, first, anchor, bounds);
                    let ray = WorldRay {
                        target,
                        mount,
                        free,
                    };
                    let Some(mut angles) = self.minimize(index, pair, initial, bounds, ray, ws)
                    else {
                        continue;
                    };
                    for j in 0..2 {
                        angles[j] = nearest_equivalent(angles[j], anchor[j], bounds[j]);
                    }
                    let Some((residual, _)) = self.residual(index, pair, angles, target, mount, ws)
                    else {
                        continue;
                    };
                    if free == [true; 2] && dot(residual, residual).sqrt() > RESIDUAL_TOLERANCE {
                        continue;
                    }
                    let writes = std::array::from_fn(|j| {
                        let driver = drivers[j]?;
                        let axis = &models[pair[j]];
                        let mapped = driver
                            .mapping
                            .raw_for_physical(axis.calibration.calibrated_to_physical(angles[j]))
                            .unwrap();
                        Some(self.write(driver.channel, driver.mapping.function_id, mapped.raw))
                    });
                    let mut verified = out.clone();
                    if !self.verify_pair(index, pair, writes, context, ws, &mut verified) {
                        failed_verification = verified.status;
                        continue;
                    }
                    let Some(encoded_error) =
                        ws.poses[index].world.and_then(|p| ray_error(p, target))
                    else {
                        continue;
                    };
                    let achieved = pair.map(|i| ws.commands[i].absolute_degrees().unwrap());
                    let distance = (achieved[0] - anchor[0]).hypot(achieved[1] - anchor[1]);
                    let candidate = Candidate {
                        writes,
                        error: encoded_error,
                        distance,
                    };
                    if best.is_none_or(|b| candidate.improves(&b)) {
                        best = Some(candidate);
                    }
                    // Only an unchanged verified joint pair is an unconditional continuity optimum.
                    if distance < 1e-8 && encoded_error <= ENCODED_MATCH_DEGREES {
                        break;
                    }
                }
            }
        }
        out.search_limited |= ws.evaluations >= MAX_EVALUATIONS_PER_FIT;
        let Some(best) = best else {
            out.status = if out.search_limited {
                PositionFitStatus::SolverCapacity
            } else {
                failed_verification
            };
            return;
        };
        // Quantization is reported by the final achieved ray, not hidden by continuous solver
        // accuracy. A coarse native channel can have a larger error even for a reachable target.
        // A single moving axis that cannot reach the ray reports the shortfall as clipping.
        out.clipped |= free != [true; 2] && best.error > ENCODED_MATCH_DEGREES;
        self.verify_pair(index, pair, best.writes, context, ws, out);
    }
    /// Calibrated travel of one driver, or the held angle of a fixed axis.
    fn axis_bounds(&self, axis: usize, driver: Option<&Driver>) -> [f64; 2] {
        let axis = &self.forward.fitting_axes()[axis];
        let Some(d) = driver else {
            let held = axis.fixed.unwrap_or(0.);
            return [held, held];
        };
        let a = axis
            .calibration
            .physical_to_calibrated(d.mapping.physical_for_raw(d.from).physical);
        let b = axis
            .calibration
            .physical_to_calibrated(d.mapping.physical_for_raw(d.to).physical);
        [a.min(b), a.max(b)]
    }
    fn residual(
        &self,
        index: usize,
        pair: [usize; 2],
        angles: [f64; 2],
        target: [f64; 3],
        mount: R,
        ws: &mut PositionFitWorkspace,
    ) -> Option<([f64; 3], [[f64; 3]; 2])> {
        if ws.evaluations >= MAX_EVALUATIONS_PER_FIT {
            return None;
        }
        ws.evaluations += 1;
        for j in 0..2 {
            ws.trial_axes[pair[j]] = Some(angles[j]);
        }
        let (pose, tangents) = self.forward.fitting_lens_geometry(
            index,
            &self.lenses[index].ancestry,
            &ws.trial_axes,
            mount,
            pair,
        )?;
        let origin = pose.point([0.; 3]);
        let toward: [f64; 3] = std::array::from_fn(|i| target[i] - origin[i]);
        let distance = toward[0].hypot(toward[1]).hypot(toward[2]);
        let wanted = unit(toward)?;
        let direction = unit(pose.direction([0., -1., 0.]))?;
        let jacobian = tangents.map(|axis| {
            let projection = dot(wanted, axis.origin);
            std::array::from_fn(|i| {
                axis.direction[i] + (axis.origin[i] - wanted[i] * projection) / distance
            })
        });
        // Full direction difference rejects antiparallel rays. The derivative includes moving
        // lens origin as well as beam direction (rigid or reflected by a mirror); an origin at
        // the mounting point is not assumed.
        Some((std::array::from_fn(|i| direction[i] - wanted[i]), jacobian))
    }
    fn minimize(
        &self,
        index: usize,
        pair: [usize; 2],
        mut angles: [f64; 2],
        bounds: [[f64; 2]; 2],
        world: WorldRay,
        ws: &mut PositionFitWorkspace,
    ) -> Option<[f64; 2]> {
        let WorldRay {
            target,
            mount,
            free,
        } = world;
        let held = |(r, mut jacobian): ([f64; 3], [[f64; 3]; 2])| {
            for j in 0..2 {
                if !free[j] {
                    jacobian[j] = [0.; 3];
                }
            }
            (r, jacobian)
        };
        let (mut r, mut jacobian) = held(self.residual(index, pair, angles, target, mount, ws)?);
        let mut lambda = 1e-7;
        for _ in 0..ITERATIONS {
            if dot(r, r) < 1e-18 {
                break;
            }
            let a = dot(jacobian[0], jacobian[0]) + lambda;
            let b = dot(jacobian[0], jacobian[1]);
            let c = dot(jacobian[1], jacobian[1]) + lambda;
            let g = [dot(jacobian[0], r), dot(jacobian[1], r)];
            let det = a * c - b * b;
            if !det.is_finite() || det <= 0. {
                return None;
            }
            let mut step = [(-c * g[0] + b * g[1]) / det, (b * g[0] - a * g[1]) / det];
            let length = step[0].hypot(step[1]);
            if length > 40. {
                step = step.map(|v| v * 40. / length);
            }
            let next =
                std::array::from_fn(|j| (angles[j] + step[j]).clamp(bounds[j][0], bounds[j][1]));
            let (next_r, next_jacobian) =
                held(self.residual(index, pair, next, target, mount, ws)?);
            if dot(next_r, next_r) < dot(r, r) {
                angles = next;
                r = next_r;
                jacobian = next_jacobian;
                lambda = (lambda * 0.3).max(1e-12);
                if length < 1e-8 {
                    break;
                }
            } else {
                lambda *= 10.;
                if lambda > 1e4 {
                    break;
                }
            }
        }
        Some(angles)
    }
}
fn nearest_equivalent(value: f64, previous: f64, bounds: [f64; 2]) -> f64 {
    let min_turn = ((bounds[0] - value) / 360.).ceil();
    let max_turn = ((bounds[1] - value) / 360.).floor();
    if min_turn > max_turn {
        return value.clamp(bounds[0], bounds[1]);
    }
    let turn = ((previous - value) / 360.)
        .round()
        .clamp(min_turn, max_turn);
    (value + turn * 360.).clamp(bounds[0], bounds[1])
}
