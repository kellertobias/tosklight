//! Semantic elevation linework, independent of mesh triangulation and wall subtraction strips.
use crate::{VenueWindowShape, WallOpening, WallOpeningKind};

type Point = [f64; 3];
type Line = [Point; 2];

/// Centred model coordinates, as for `venue_geometry`. Frames are drawing symbols, not glass
/// occluders: an aperture remains physically open to light and picking.
pub fn venue_wall_drawing(size: [f64; 3], openings: &[WallOpening]) -> Vec<Line> {
    let mut lines = Vec::new();
    let [width, height, _] = size;
    contour(
        &mut lines,
        &rectangle(-width / 2.0, -height / 2.0, width, height),
    );
    for opening in openings {
        let x = f64::from(opening.offset_millimetres) - width / 2.0;
        let y = f64::from(opening.sill_millimetres) - height / 2.0;
        let w = f64::from(opening.width_millimetres);
        let h = f64::from(opening.height_millimetres);
        if let WallOpeningKind::Window { shape } = opening.opening {
            // Two outlines clearly distinguish the frame from an empty hole in an elevation.
            contour(&mut lines, &window(shape, x, y, w, h));
            let inset = 50.0_f64.min(w / 8.0).min(h / 8.0);
            contour(
                &mut lines,
                &window(
                    shape,
                    x + inset,
                    y + inset,
                    w - inset * 2.0,
                    h - inset * 2.0,
                ),
            );
        } else {
            // The open leaf is represented by the plan swing; the elevation shows the doorway.
            lines.extend([
                [[x, y, 0.0], [x, y + h, 0.0]],
                [[x, y + h, 0.0], [x + w, y + h, 0.0]],
                [[x + w, y + h, 0.0], [x + w, y, 0.0]],
            ]);
        }
    }
    lines
}

fn contour(lines: &mut Vec<Line>, points: &[Point]) {
    for i in 0..points.len() {
        lines.push([points[i], points[(i + 1) % points.len()]]);
    }
}
fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![
        [x, y, 0.0],
        [x + w, y, 0.0],
        [x + w, y + h, 0.0],
        [x, y + h, 0.0],
    ]
}
fn window(shape: VenueWindowShape, x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    match shape {
        VenueWindowShape::Rectangular => rectangle(x, y, w, h),
        VenueWindowShape::Round => (0..64)
            .map(|i| {
                let angle = f64::from(i) * std::f64::consts::TAU / 64.0;
                [
                    x + w / 2.0 + w / 2.0 * angle.cos(),
                    y + h / 2.0 + h / 2.0 * angle.sin(),
                    0.0,
                ]
            })
            .collect(),
        VenueWindowShape::Arched => {
            let mut points = vec![[x, y, 0.0], [x + w, y, 0.0]];
            points.extend((0..=32).map(|i| {
                let angle = f64::from(i) * std::f64::consts::PI / 32.0;
                [
                    x + w / 2.0 + w / 2.0 * angle.cos(),
                    y + h - w / 2.0 + w / 2.0 * angle.sin(),
                    0.0,
                ]
            }));
            points
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_elevations_have_two_clean_contours_at_the_correct_sill() {
        for (shape, count) in [
            (VenueWindowShape::Rectangular, 12),
            (VenueWindowShape::Round, 132),
            (VenueWindowShape::Arched, 74),
        ] {
            let opening = WallOpening {
                id: uuid::Uuid::nil(),
                offset_millimetres: 1000,
                sill_millimetres: 800,
                width_millimetres: 1200,
                height_millimetres: 1600,
                opening: WallOpeningKind::Window { shape },
            };
            let lines = venue_wall_drawing([5000.0, 3000.0, 200.0], &[opening]);
            assert_eq!(lines.len(), count);
            let points: Vec<_> = lines[4..].iter().flatten().collect();
            assert_eq!(
                points.iter().map(|p| p[0]).min_by(f64::total_cmp),
                Some(-1500.0)
            );
            assert_eq!(
                points.iter().map(|p| p[1]).min_by(f64::total_cmp),
                Some(-700.0)
            );
            assert!(points.iter().all(|p| p[2] == 0.0));
        }
    }
}
