//! Allocation-free bounded least squares for the visible emitter amounts of one candidate.
//!
//! Every variable is a normalized emitter amount `x ∈ [0, upper]` whose column is the emitter's
//! compiled XYZ at its declared maximum drive. The problem size is bounded by
//! [`MAX_VISIBLE_VARIABLES`]; solving never allocates.
use light_core::programming::ColorAllocation;

pub(super) const MAX_VISIBLE_VARIABLES: usize = 16;
/// Stage 1 holds luminance at the request while searching the nearest reachable chromaticity.
const LUMINANCE_HOLD_WEIGHT: f64 = 100.0;
/// Stage 2 keeps that chromaticity while approaching the requested luminance within the box.
const CHROMATICITY_HOLD_WEIGHT: f64 = 100.0;
/// Tie-break toward the allocation preference; far below one quantization step.
const ALLOCATION_WEIGHT: f64 = 1e-7;
const MAX_OUTER_STEPS: usize = 4 * MAX_VISIBLE_VARIABLES + 8;

type Matrix = [[f64; MAX_VISIBLE_VARIABLES]; MAX_VISIBLE_VARIABLES];

/// `½ xᵀHx − gᵀx` over `0 ≤ x ≤ upper`, strictly convex through the allocation term.
struct Qp {
    n: usize,
    h: Matrix,
    g: [f64; MAX_VISIBLE_VARIABLES],
    upper: f64,
}

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// `perpendicular² (I − eeᵀ) + parallel² eeᵀ`, divided by `|target|²` so weights are relative.
fn metric(target: [f64; 3], perpendicular: f64, parallel: f64) -> Option<[[f64; 3]; 3]> {
    let length = norm(target);
    if !(length.is_finite() && length > 1e-12) {
        return None;
    }
    let e = target.map(|v| v / length);
    let (p2, q2) = (perpendicular * perpendicular, parallel * parallel);
    let scale = 1.0 / (length * length);
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let identity = if i == j { 1.0 } else { 0.0 };
            scale * (p2 * (identity - e[i] * e[j]) + q2 * e[i] * e[j])
        })
    }))
}

impl Qp {
    fn new(
        columns: &[[f64; 3]],
        metric: &[[f64; 3]; 3],
        target: [f64; 3],
        upper: f64,
        weights: &[f64],
        preference: &[f64],
    ) -> Self {
        let n = columns.len();
        let mut qp = Self {
            n,
            h: [[0.0; MAX_VISIBLE_VARIABLES]; MAX_VISIBLE_VARIABLES],
            g: [0.0; MAX_VISIBLE_VARIABLES],
            upper,
        };
        let weighted = |v: [f64; 3]| -> [f64; 3] { std::array::from_fn(|r| dot(metric[r], v)) };
        let gt = weighted(target);
        for i in 0..n {
            let gi = weighted(columns[i]);
            for (j, column) in columns.iter().enumerate() {
                qp.h[i][j] = dot(gi, *column);
            }
            qp.g[i] = dot(columns[i], gt);
            qp.h[i][i] += ALLOCATION_WEIGHT * weights[i];
            qp.g[i] += ALLOCATION_WEIGHT * weights[i] * preference[i];
        }
        qp
    }

    /// Solves the free subsystem with the bound variables held. Cholesky on at most 16×16.
    fn subproblem(&self, x: &[f64], free: &[bool], z: &mut [f64; MAX_VISIBLE_VARIABLES]) -> bool {
        let mut index = [0usize; MAX_VISIBLE_VARIABLES];
        let mut m = 0;
        for i in (0..self.n).filter(|&i| free[i]) {
            index[m] = i;
            m += 1;
        }
        let mut a = [[0.0; MAX_VISIBLE_VARIABLES]; MAX_VISIBLE_VARIABLES];
        let mut b = [0.0; MAX_VISIBLE_VARIABLES];
        for r in 0..m {
            let i = index[r];
            b[r] = self.g[i]
                - (0..self.n)
                    .filter(|&k| !free[k])
                    .map(|k| self.h[i][k] * x[k])
                    .sum::<f64>();
            for c in 0..m {
                a[r][c] = self.h[i][index[c]];
            }
        }
        for c in 0..m {
            let pivot = a[c][..c].iter().fold(a[c][c], |p, v| p - v * v);
            if !(pivot.is_finite() && pivot > 0.0) {
                return false;
            }
            a[c][c] = pivot.sqrt();
            for r in c + 1..m {
                let value = a[r][..c]
                    .iter()
                    .zip(&a[c][..c])
                    .fold(a[r][c], |v, (x, y)| v - x * y);
                a[r][c] = value / a[c][c];
            }
        }
        for r in 0..m {
            let mut value = b[r];
            for k in 0..r {
                value -= a[r][k] * b[k];
            }
            b[r] = value / a[r][r];
        }
        for r in (0..m).rev() {
            let mut value = b[r];
            for k in r + 1..m {
                value -= a[k][r] * b[k];
            }
            b[r] = value / a[r][r];
        }
        z.fill(0.0);
        for r in 0..m {
            z[index[r]] = b[r];
        }
        true
    }

    /// Moves toward the free-set minimizer, dropping blocking variables onto their bounds.
    fn relax(&self, x: &mut [f64], free: &mut [bool]) -> bool {
        let before: [f64; MAX_VISIBLE_VARIABLES] =
            std::array::from_fn(|i| x.get(i).copied().unwrap_or(0.0));
        let mut z = [0.0; MAX_VISIBLE_VARIABLES];
        for _ in 0..=self.n {
            if !self.subproblem(x, free, &mut z) {
                break;
            }
            let mut alpha = 1.0;
            let mut blocking = None;
            for i in (0..self.n).filter(|&i| free[i]) {
                let step = if z[i] < 0.0 {
                    x[i] / (x[i] - z[i])
                } else if z[i] > self.upper {
                    (self.upper - x[i]) / (z[i] - x[i])
                } else {
                    continue;
                };
                if step < alpha {
                    alpha = step;
                    blocking = Some(i);
                }
            }
            let Some(blocking) = blocking else {
                for i in (0..self.n).filter(|&i| free[i]) {
                    x[i] = z[i];
                }
                break;
            };
            for i in 0..self.n {
                if !free[i] {
                    continue;
                }
                x[i] += alpha * (z[i] - x[i]);
                let bound = if i == blocking {
                    Some(if z[i] < 0.0 { 0.0 } else { self.upper })
                } else if x[i] <= 0.0 {
                    Some(0.0)
                } else if x[i] >= self.upper {
                    Some(self.upper)
                } else {
                    None
                };
                if let Some(bound) = bound {
                    x[i] = bound;
                    free[i] = false;
                }
            }
        }
        (0..self.n).any(|i| x[i] != before[i])
    }

    fn solve(&self, x: &mut [f64]) {
        x[..self.n].fill(0.0);
        let mut free = [false; MAX_VISIBLE_VARIABLES];
        let mut blocked = [false; MAX_VISIBLE_VARIABLES];
        let scale = self.g[..self.n]
            .iter()
            .map(|v| v.abs())
            .chain((0..self.n).map(|i| self.h[i][i]))
            .fold(1.0, f64::max);
        let tolerance = 1e-12 * scale;
        for _ in 0..MAX_OUTER_STEPS {
            let mut entering = None;
            let mut magnitude = tolerance;
            for j in (0..self.n).filter(|&j| !free[j] && !blocked[j]) {
                let slope = self.g[j] - (0..self.n).map(|k| self.h[j][k] * x[k]).sum::<f64>();
                let improves = if x[j] >= self.upper {
                    slope < -tolerance
                } else {
                    slope > tolerance
                };
                if improves && slope.abs() > magnitude {
                    magnitude = slope.abs();
                    entering = Some(j);
                }
            }
            let Some(j) = entering else { break };
            free[j] = true;
            if self.relax(x, &mut free) {
                blocked = [false; MAX_VISIBLE_VARIABLES];
            } else {
                free[j] = false;
                blocked[j] = true;
            }
        }
    }
}

/// Inputs for one candidate's visible amounts.
pub(super) struct VisibleProblem<'a> {
    /// XYZ at each variable's maximum declared drive, through the candidate's filters.
    pub columns: &'a [[f64; 3]],
    pub white: &'a [bool],
    /// Requested visible XYZ: the total the path should emit, fixed contributions included.
    pub target: [f64; 3],
    /// Fixed known contributions emitted regardless of the visible amounts (fixed source,
    /// frozen UV leakage). Chromaticity is fitted on `offset + columns·x`, never on a residual.
    pub offset: [f64; 3],
    pub colored_part: [f64; 3],
    pub white_part: [f64; 3],
    pub allocation: ColorAllocation,
}

fn preference(problem: &VisibleProblem<'_>, weights: &mut [f64], preferred: &mut [f64]) {
    let n = problem.columns.len();
    weights[..n].fill(1.0);
    preferred[..n].fill(0.0);
    match problem.allocation {
        ColorAllocation::PreferWhite => {
            for (w, white) in weights.iter_mut().zip(problem.white) {
                if *white {
                    *w = 0.01;
                }
            }
        }
        ColorAllocation::PreferColoredEmitters => {
            for (w, white) in weights.iter_mut().zip(problem.white) {
                if *white {
                    *w = 100.0;
                }
            }
        }
        ColorAllocation::PreserveRecipe => {
            // The base recipe is carried by colored emitters and the white target by dedicated
            // white emitters where the head has them; exact fits still use every emitter.
            if !problem.white.iter().any(|w| *w) || problem.white.iter().all(|w| *w) {
                return;
            }
            for (part, want_white) in [(problem.colored_part, false), (problem.white_part, true)] {
                let Some(m) = metric(part, 1.0, 1.0) else {
                    continue;
                };
                let mut columns = [[0.0; 3]; MAX_VISIBLE_VARIABLES];
                let mut owners = [0usize; MAX_VISIBLE_VARIABLES];
                let mut count = 0;
                for i in (0..n).filter(|&i| problem.white[i] == want_white) {
                    columns[count] = problem.columns[i];
                    owners[count] = i;
                    count += 1;
                }
                let ones = [1.0; MAX_VISIBLE_VARIABLES];
                let zeros = [0.0; MAX_VISIBLE_VARIABLES];
                let qp = Qp::new(&columns[..count], &m, part, 1.0, &ones, &zeros);
                let mut x = [0.0; MAX_VISIBLE_VARIABLES];
                qp.solve(&mut x);
                for k in 0..count {
                    preferred[owners[k]] = x[k];
                }
            }
        }
    }
}

/// CIE 1976 u'v' of an XYZ triple, `None` for black.
pub(super) fn chromaticity(v: [f64; 3]) -> Option<(f64, f64)> {
    let d = v[0] + 15. * v[1] + 3. * v[2];
    (d.is_finite() && d > 1e-12).then(|| (4. * v[0] / d, 9. * v[1] / d))
}

pub(super) fn delta_uv(achieved: [f64; 3], target: [f64; 3]) -> Option<f64> {
    let (a, t) = (chromaticity(achieved)?, chromaticity(target)?);
    Some((a.0 - t.0).hypot(a.1 - t.1))
}

/// Chromaticity match of a COMPLETE forward-evaluated output against a target: a black target
/// is matched only by black luminance; otherwise Δu′v′ within EXACT/APPROXIMATE, and an
/// achromatic (black) output or one without a finite distance is out of gamut.
pub(super) fn color_match(achieved: [f64; 3], target: [f64; 3]) -> super::ColorMatch {
    use super::ColorMatch;
    use light_core::color_intent::{APPROXIMATE_DELTA_UV, EXACT_DELTA_UV};
    if chromaticity(target).is_none() {
        return if achieved[1].abs() <= super::COLOR_MATCH_BLACK_Y {
            ColorMatch::Exact
        } else {
            ColorMatch::OutOfGamut
        };
    }
    match delta_uv(achieved, target) {
        Some(d) if d <= f64::from(EXACT_DELTA_UV) => ColorMatch::Exact,
        Some(d) if d <= f64::from(APPROXIMATE_DELTA_UV) => ColorMatch::Approximate,
        _ => ColorMatch::OutOfGamut,
    }
}

/// Luminance scales of the requested target sampled before refinement when a fixed offset
/// makes the reachable chromaticity depend on the output level.
const SCALE_GRID: usize = 16;
/// Golden-section refinements around the best grid scale; shrinks the bracket below 1e-5.
const SCALE_REFINEMENTS: usize = 24;
/// Relative luminance error traded per unit of Δu'v' while ranking scales: far below one
/// quantization step of chromaticity, so luminance only separates equally good hues.
const SCALE_LUMINANCE_WEIGHT: f64 = 1e-5;

/// Chromaticity first, then luminance, then allocation. Writes normalized amounts into `x`.
///
/// The fitted quantity is always the total `offset + columns·x`. Without an offset its
/// chromaticity is independent of the output level, so one solve at the requested luminance
/// finds it. With an offset the chromaticity depends on the level, so a bounded search over
/// the level `s·target` (grid plus golden-section refinement) keeps the level whose final
/// chromaticity is best, and among equally good ones the level closest to the request.
///
/// Returns the measured work: chromaticity-first level solves (two bounded QPs each) and whether
/// a fixed offset forced the bounded level search. Counters only; they imply no budget.
pub(super) fn solve_visible(problem: &VisibleProblem<'_>, x: &mut [f64]) -> VisibleSolveWork {
    let n = problem.columns.len();
    x[..n].fill(0.0);
    let mut weights = [1.0; MAX_VISIBLE_VARIABLES];
    let mut preferred = [0.0; MAX_VISIBLE_VARIABLES];
    preference(problem, &mut weights, &mut preferred);
    // Black keeps every visible emitter off.
    let mut work = VisibleSolveWork::default();
    let Some(hold_luminance) = metric(problem.target, 1.0, LUMINANCE_HOLD_WEIGHT) else {
        return work;
    };
    let fit = Level {
        problem,
        hold_luminance,
        weights: &weights[..n],
        preferred: &preferred[..n],
    };
    let offset = problem.offset;
    work.levels = 1;
    if norm(offset) <= 1e-12 * norm(problem.target) {
        fit.solve(1.0, x);
        return work;
    }
    work.fixed_offset = true;
    // Scales at which the total projected onto the request is the offset alone and the offset
    // plus every emitter at its maximum; the reachable levels lie between them.
    let length2 = dot(problem.target, problem.target);
    let mut full = offset;
    for column in problem.columns {
        for c in 0..3 {
            full[c] += column[c];
        }
    }
    let low = (dot(offset, problem.target) / length2).max(0.0);
    let high = (dot(full, problem.target) / length2).max(low);
    let mut amounts = [0.0; MAX_VISIBLE_VARIABLES];
    let mut best = (fit.cost(1.0, &mut amounts), 1.0);
    x[..n].copy_from_slice(&amounts[..n]);
    let step = (high - low) / (SCALE_GRID - 1) as f64;
    if !(step.is_finite() && step > 0.0) {
        return work;
    }
    let mut keep = |scale: f64, x: &mut [f64], best: &mut (f64, f64)| {
        work.levels += 1;
        let cost = fit.cost(scale, &mut amounts);
        if cost < best.0 {
            *best = (cost, scale);
            x[..n].copy_from_slice(&amounts[..n]);
        }
        cost
    };
    for i in 0..SCALE_GRID {
        keep(low + step * i as f64, x, &mut best);
    }
    // Refine inside the grid cells around the best sampled scale.
    let (mut a, mut b) = ((best.1 - step).max(low), (best.1 + step).min(high));
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let (mut c, mut d) = (b - ratio * (b - a), a + ratio * (b - a));
    let (mut fc, mut fd) = (keep(c, x, &mut best), keep(d, x, &mut best));
    for _ in 0..SCALE_REFINEMENTS {
        if fc <= fd {
            (b, d, fd) = (d, c, fc);
            c = b - ratio * (b - a);
            fc = keep(c, x, &mut best);
        } else {
            (a, c, fc) = (c, d, fd);
            d = a + ratio * (b - a);
            fd = keep(d, x, &mut best);
        }
    }
    work
}

/// Work of one visible solve; see [`solve_visible`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct VisibleSolveWork {
    pub levels: u32,
    pub fixed_offset: bool,
}

/// One level of the chromaticity-first solve for a fixed candidate.
struct Level<'a> {
    problem: &'a VisibleProblem<'a>,
    hold_luminance: [[f64; 3]; 3],
    weights: &'a [f64],
    preferred: &'a [f64],
}

impl Level<'_> {
    /// Stage 1 holds the total at `scale × target` while searching the nearest reachable
    /// chromaticity; stage 2 keeps that total chromaticity while approaching its luminance
    /// within the drive box.
    fn solve(&self, scale: f64, x: &mut [f64]) {
        let problem = self.problem;
        let (n, offset) = (problem.columns.len(), problem.offset);
        x[..n].fill(0.0);
        let stage1 = Qp::new(
            problem.columns,
            &self.hold_luminance,
            std::array::from_fn(|c| scale * problem.target[c] - offset[c]),
            f64::INFINITY,
            self.weights,
            self.preferred,
        );
        let mut direction_amounts = [0.0; MAX_VISIBLE_VARIABLES];
        stage1.solve(&mut direction_amounts);
        let reachable = total(problem, &direction_amounts);
        let Some(hold_chromaticity) = metric(reachable, CHROMATICITY_HOLD_WEIGHT, 1.0) else {
            return;
        };
        Qp::new(
            problem.columns,
            &hold_chromaticity,
            std::array::from_fn(|c| reachable[c] - offset[c]),
            1.0,
            self.weights,
            self.preferred,
        )
        .solve(x);
    }

    /// Final Δu'v' of the boxed total, plus a small luminance term that separates equal hues.
    fn cost(&self, scale: f64, x: &mut [f64]) -> f64 {
        self.solve(scale, x);
        let target = self.problem.target;
        let achieved = total(self.problem, x);
        let chroma = delta_uv(achieved, target).unwrap_or(1.0);
        let level = dot(achieved, target) / dot(target, target);
        chroma + SCALE_LUMINANCE_WEIGHT * (level - 1.0).abs()
    }
}

/// The total `offset + columns·x` a set of amounts emits.
fn total(problem: &VisibleProblem<'_>, x: &[f64]) -> [f64; 3] {
    let mut sum = problem.offset;
    for (column, amount) in problem.columns.iter().zip(x) {
        let amount = if amount.is_finite() { *amount } else { 0.0 };
        for c in 0..3 {
            sum[c] += column[c] * amount;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_bounded_least_squares_matches_hand_solutions() {
        // Three independent axes: exact inside the box, clipped along the ray outside it.
        let columns = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let white = [false; 3];
        let mut x = [0.0; MAX_VISIBLE_VARIABLES];
        for (target, expected) in [
            ([0.25, 0.5, 1.0], [0.25, 0.5, 1.0]),
            ([1.0, 2.0, 4.0], [0.25, 0.5, 1.0]),
            ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        ] {
            solve_visible(
                &VisibleProblem {
                    columns: &columns,
                    white: &white,
                    target,
                    offset: [0.0; 3],
                    colored_part: target,
                    white_part: [0.0; 3],
                    allocation: ColorAllocation::PreserveRecipe,
                },
                &mut x,
            );
            // Clipping trades 1/CHROMATICITY_HOLD_WEIGHT² of chromaticity for luminance: far
            // below one 16-bit quantization step.
            for i in 0..3 {
                assert!((x[i] - expected[i]).abs() < 2e-4, "{target:?}: {x:?}");
            }
        }
    }

    #[test]
    fn out_of_gamut_direction_is_not_collapsed_to_black() {
        // Only +X and +Y exist; a request with negative-free Z is matched in the XY plane.
        let columns = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut x = [0.0; MAX_VISIBLE_VARIABLES];
        solve_visible(
            &VisibleProblem {
                columns: &columns,
                white: &[false; 2],
                target: [0.5, 0.5, 0.2],
                offset: [0.0; 3],
                colored_part: [0.5, 0.5, 0.2],
                white_part: [0.0; 3],
                allocation: ColorAllocation::PreserveRecipe,
            },
            &mut x,
        );
        assert!(x[0] > 0.4 && x[1] > 0.4, "{x:?}");
    }
}
