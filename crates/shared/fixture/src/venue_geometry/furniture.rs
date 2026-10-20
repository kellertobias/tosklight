use super::*;
pub(super) fn table(g: &mut VenueGeometry, s: [f64; 3], shape: VenueTableShape) {
    let thickness = 50.0_f64.min(s[1] * 0.1);
    if shape == VenueTableShape::Round {
        g.cylinder(
            [0.0, (s[1] - thickness) / 2.0, 0.0],
            [s[0], thickness, s[2]],
        );
        g.cylinder(
            [0.0, -thickness / 2.0, 0.0],
            [s[0] * 0.12, s[1] - thickness, s[2] * 0.12],
        );
        g.cylinder(
            [0.0, -s[1] / 2.0 + 15.0, 0.0],
            [s[0] * 0.6, 30.0, s[2] * 0.6],
        );
    } else {
        g.cuboid(
            [0.0, (s[1] - thickness) / 2.0, 0.0],
            [s[0], thickness, s[2]],
        );
        legs(g, s, s[1] - thickness, -thickness / 2.0, false);
    }
    g.footprint(s, shape == VenueTableShape::Round);
}
fn legs(g: &mut VenueGeometry, s: [f64; 3], height: f64, y: f64, metal: bool) {
    let first = g.triangles.len();
    for x in [-0.4, 0.4] {
        for z in [-0.4, 0.4] {
            g.cuboid([s[0] * x, y, s[2] * z], [s[0] * 0.06, height, s[2] * 0.06]);
        }
    }
    if metal {
        g.metallic[first..].fill(true);
    }
}
pub(super) fn chair(g: &mut VenueGeometry, s: [f64; 3], style: VenueChairStyle) {
    let stool = style == VenueChairStyle::BarStool;
    let seat_y = if stool { s[1] * 0.25 } else { 0.0 };
    let seat = s[1] * 0.055;
    if stool {
        g.cylinder([0.0, seat_y, 0.0], [s[0] * 0.9, seat, s[2] * 0.9]);
    } else {
        g.cuboid([0.0, seat_y, 0.0], [s[0], seat, s[2]]);
    }
    let leg_height = seat_y + s[1] / 2.0;
    legs(
        g,
        s,
        leg_height,
        -s[1] / 2.0 + leg_height / 2.0,
        style != VenueChairStyle::White,
    );
    if !stool {
        g.cuboid(
            [0.0, s[1] * 0.25, s[2] * 0.44],
            [s[0], s[1] * 0.5, s[2] * 0.08],
        );
    }
    if style == VenueChairStyle::White {
        for x in [-0.4, 0.4] {
            g.cuboid(
                [s[0] * x, s[1] * 0.17, 0.0],
                [s[0] * 0.06, s[1] * 0.2, s[2] * 0.8],
            );
        }
    }
    if stool {
        g.cuboid(
            [0.0, -s[1] * 0.15, s[2] * -0.4],
            [s[0] * 0.85, s[1] * 0.025, s[2] * 0.04],
        );
    }
    g.footprint(s, false);
}
