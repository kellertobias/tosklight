use super::*;
/// Vertical strips subtract each aperture exactly from the wall mesh. Curves use a bounded
/// polygonal approximation; the same boundary forms the reveals and physical shadow surfaces.
pub(super) fn build(g: &mut VenueGeometry, s: [f64; 3], openings: &[WallOpening]) {
    let mut cuts = vec![-s[0] / 2.0, s[0] / 2.0];
    for o in openings {
        let start = f64::from(o.offset_millimetres) - s[0] / 2.0;
        let w = f64::from(o.width_millimetres);
        let segments = if matches!(
            o.opening,
            WallOpeningKind::Window {
                shape: VenueWindowShape::Round | VenueWindowShape::Arched
            }
        ) {
            24
        } else {
            1
        };
        for i in 0..=segments {
            cuts.push(start + w * i as f64 / segments as f64);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    for strip in cuts.windows(2) {
        let (x0, x1) = (strip[0], strip[1]);
        let xm = (x0 + x1) / 2.0;
        let active: Vec<_> = openings
            .iter()
            .filter(|o| {
                let x = f64::from(o.offset_millimetres) - s[0] / 2.0;
                xm > x && xm < x + f64::from(o.width_millimetres)
            })
            .collect();
        let mut gaps: Vec<_> = active
            .iter()
            .map(|o| {
                let a = boundary(o, x0 + s[0] / 2.0);
                let b = boundary(o, x1 + s[0] / 2.0);
                [
                    a[0] - s[1] / 2.0,
                    a[1] - s[1] / 2.0,
                    b[0] - s[1] / 2.0,
                    b[1] - s[1] / 2.0,
                ]
            })
            .collect();
        gaps.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mut low = [-s[1] / 2.0; 2];
        for gap in gaps {
            panel(g, x0, x1, low, [gap[0], gap[2]], s[2]);
            low = [gap[1], gap[3]];
        }
        panel(g, x0, x1, low, [s[1] / 2.0; 2], s[2]);
    }
    for o in openings {
        door_assembly(g, o, s);
        if matches!(o.opening, WallOpeningKind::Window { .. }) {
            let start = f64::from(o.offset_millimetres) - s[0] / 2.0;
            let end = start + f64::from(o.width_millimetres);
            // Conventional top-plan glazing: paired thin lines between the solid jambs.
            for z in [-s[2] * 0.12, s[2] * 0.12] {
                g.plan.push([[start, z], [end, z]]);
            }
        }
    }
    // Top plan retains genuine door/window gaps rather than replacing them with a solid box.
    let mut plan_cuts = vec![-s[0] / 2.0, s[0] / 2.0];
    for o in openings {
        plan_cuts.extend([
            f64::from(o.offset_millimetres) - s[0] / 2.0,
            f64::from(o.offset_millimetres + o.width_millimetres) - s[0] / 2.0,
        ]);
    }
    plan_cuts.sort_by(f64::total_cmp);
    for pair in plan_cuts.windows(2) {
        let mid = (pair[0] + pair[1]) / 2.0;
        let gap = openings.iter().any(|o| {
            let x = f64::from(o.offset_millimetres) - s[0] / 2.0;
            mid > x && mid < x + f64::from(o.width_millimetres)
        });
        if !gap {
            for z in [-s[2] / 2.0, s[2] / 2.0] {
                g.plan.push([[pair[0], z], [pair[1], z]]);
            }
        }
    }
    for x in plan_cuts {
        g.plan.push([[x, -s[2] / 2.0], [x, s[2] / 2.0]]);
    }
}
fn boundary(o: &WallOpening, x: f64) -> [f64; 2] {
    let w = f64::from(o.width_millimetres);
    let h = f64::from(o.height_millimetres);
    let sill = f64::from(o.sill_millimetres);
    let dx = (x - f64::from(o.offset_millimetres) - w / 2.0).clamp(-w / 2.0, w / 2.0);
    let arc = ((w / 2.0).powi(2) - dx * dx).max(0.0).sqrt();
    match o.opening {
        WallOpeningKind::Window {
            shape: VenueWindowShape::Round,
        } => [sill + h / 2.0 - arc, sill + h / 2.0 + arc],
        WallOpeningKind::Window {
            shape: VenueWindowShape::Arched,
        } => [sill, sill + h - w / 2.0 + arc],
        _ => [sill, sill + h],
    }
}
fn panel(g: &mut VenueGeometry, x0: f64, x1: f64, b: [f64; 2], t: [f64; 2], depth: f64) {
    if (t[0] - b[0]).max(t[1] - b[1]) < 1e-6 {
        return;
    }
    let front = [
        [x0, b[0], depth / 2.0],
        [x1, b[1], depth / 2.0],
        [x1, t[1], depth / 2.0],
        [x0, t[0], depth / 2.0],
    ];
    let back = front.map(|p| [p[0], p[1], -depth / 2.0]);
    g.quad(front);
    let mut reversed = back;
    reversed.reverse();
    g.quad(reversed);
    for i in 0..4 {
        let j = (i + 1) % 4;
        g.quad([front[i], back[i], back[j], front[j]]);
    }
}
/// The same hinge and swing author both the plan symbol and a thin, 90-degree open leaf.
fn door_assembly(g: &mut VenueGeometry, o: &WallOpening, s: [f64; 3]) {
    let WallOpeningKind::Door {
        leaves,
        swing,
        hinge,
    } = o.opening
    else {
        return;
    };
    let double = leaves == VenueDoorLeaves::Double;
    let radius = f64::from(o.width_millimetres) / if double { 2.0 } else { 1.0 };
    let left = f64::from(o.offset_millimetres) - s[0] / 2.0;
    let direction = if swing == VenueDoorSwing::Inward {
        1.0
    } else {
        -1.0
    };
    let sides: &[bool] = if double {
        &[false, true]
    } else if hinge == VenueDoorHinge::Right {
        &[true]
    } else {
        &[false]
    };
    for &right in sides {
        let x = left
            + if right {
                f64::from(o.width_millimetres)
            } else {
                0.0
            };
        let dx = if right { -1.0 } else { 1.0 };
        g.plan.push([[x, 0.0], [x, direction * radius]]);
        for i in 0..20 {
            let p = |j| {
                let a = std::f64::consts::FRAC_PI_2 * f64::from(j) / 20.0;
                [x + dx * radius * a.cos(), direction * radius * a.sin()]
            };
            g.plan.push([p(i), p(i + 1)]);
        }
        // The open leaf is real geometry, outside the aperture, with the same swing direction.
        g.cuboid(
            [
                x,
                s[1] * -0.5 + f64::from(o.height_millimetres) / 2.0,
                direction * radius / 2.0,
            ],
            [35.0, f64::from(o.height_millimetres), radius],
        );
    }
}
