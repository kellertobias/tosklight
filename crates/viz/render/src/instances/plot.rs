//! The plan views: outlines, symbols and aim lines on a plain page.
//!
//! A plan is drawn from the same scene and the same live values as the shaded picture, so a lit
//! fixture takes the colour it is emitting and a symbol sits exactly where the lantern is.

use super::*;

/// A drawn plan: outlines, symbols and aim lines on a plain page.
pub(super) fn push_plot(
    frame: &mut FrameInstances,
    scene: &Scene,
    values: &SceneValues,
    head_angles: &[(f32, f32)],
    style: &FrameStyle,
) {
    for object in &scene.scenery {
        push_box_outline(
            frame,
            object.position,
            object.size,
            euler_degrees(object.rotation_degrees),
            style.faint_ink,
            1.0,
        );
    }

    // A drawn plan is read, not admired: a fixture that makes light is drawn in the page's ink,
    // one that does not is drawn faint, and the light itself is the only thing with a colour.
    let fallback = EmitterValues::default();
    let mut makes_light = vec![false; scene.fixtures.len()];
    for emitter in &scene.emitters {
        if matches!(emitter.kind, EmitterKind::Beam | EmitterKind::Emissive)
            && let Some(slot) = makes_light.get_mut(emitter.fixture_index as usize)
        {
            *slot = true;
        }
    }

    for (index, fixture) in scene.fixtures.iter().enumerate() {
        let lights = makes_light.get(index).copied().unwrap_or(false);
        let ink = if lights { style.ink } else { style.faint_ink };
        push_symbol(frame, fixture, style, ink, if lights { 1.0 } else { 0.75 });
    }

    for (index, emitter) in scene.emitters.iter().enumerate() {
        let Some(fixture) = scene.fixtures.get(emitter.fixture_index as usize) else {
            continue;
        };
        if emitter.kind != EmitterKind::Beam {
            continue;
        }
        let value = values.emitters.get(index).unwrap_or(&fallback);
        let intensity = value.visible_intensity();
        if intensity <= 0.004 {
            continue;
        }
        let (pan, tilt) = head_angles.get(index).copied().unwrap_or((0.0, 0.0));
        let pose = emitter_pose(fixture, emitter, pan, tilt, value.zoom);
        // Every beam on a plan is the same colour, so the eye reads them as beams rather than
        // trying to read a colour off a line. The lamp's real colour is shown beside the symbol.
        push_aim_line(frame, pose.origin, pose, intensity, style.beam_ink);
    }
}

/// The plan symbol for one fixture, drawn flat on the page at its true body size.
fn push_symbol(
    frame: &mut FrameInstances,
    fixture: &FixtureInstance,
    style: &FrameStyle,
    ink: Vec3,
    alpha: f32,
) {
    let right = style.plot_right;
    let up = style.plot_up;
    // A plan symbol is a drawing convention, not a scale model: it keeps its screen size so a
    // wide-out plan stays readable, while a long body still reads as a bar.
    let size = fixture.body.size;
    let elongation = (size.x.max(size.z) / size.y.max(size.z).max(0.001)).clamp(1.0, 2.5);
    let half_height = style.symbol_metres;
    let half_width = half_height * elongation;
    let centre = fixture.position;
    let colour = ink.extend(alpha).to_array();
    let mut segment = |from: Vec3, to: Vec3| {
        for point in [from, to] {
            frame.lines.push(LineVertex {
                position: point.to_array(),
                _pad: 0.0,
                colour,
            });
        }
    };

    match fixture.body.kind {
        BodyKind::MovingHead => {
            // A circle with a stalk: the operator's shorthand for a moving head.
            let radius = half_width;
            let steps = 14;
            let mut previous = centre + right * radius;
            for step in 1..=steps {
                let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
                let point = centre + right * (angle.cos() * radius) + up * (angle.sin() * radius);
                segment(previous, point);
                previous = point;
            }
            segment(centre, centre - up * radius * 1.6);
        }
        BodyKind::Bar | BodyKind::Matrix => {
            push_box_outline(frame, centre, size, fixture.orientation(), ink, alpha);
        }
        BodyKind::Lantern => {
            // A body with one diagonal, the way a profile is plotted.
            let corners = page_rectangle(centre, right, up, half_width, half_height);
            for index in 0..4 {
                segment(corners[index], corners[(index + 1) % 4]);
            }
            segment(corners[0], corners[2]);
        }
        BodyKind::Machine | BodyKind::Generic => {
            let corners = page_rectangle(centre, right, up, half_width, half_height);
            for index in 0..4 {
                segment(corners[index], corners[(index + 1) % 4]);
            }
            segment(corners[0], corners[2]);
            segment(corners[1], corners[3]);
        }
    }
}

fn page_rectangle(
    centre: Vec3,
    right: Vec3,
    up: Vec3,
    half_width: f32,
    half_height: f32,
) -> [Vec3; 4] {
    [
        centre - right * half_width - up * half_height,
        centre + right * half_width - up * half_height,
        centre + right * half_width + up * half_height,
        centre - right * half_width + up * half_height,
    ]
}

/// The twelve edges of an oriented box, as lines.
fn push_box_outline(
    frame: &mut FrameInstances,
    centre: Vec3,
    size: Vec3,
    rotation: Quat,
    ink: Vec3,
    alpha: f32,
) {
    let half = size * 0.5;
    let corner = |x: f32, y: f32, z: f32| centre + rotation * (Vec3::new(x, y, z) * half);
    let corners = [
        corner(-1.0, -1.0, -1.0),
        corner(1.0, -1.0, -1.0),
        corner(1.0, -1.0, 1.0),
        corner(-1.0, -1.0, 1.0),
        corner(-1.0, 1.0, -1.0),
        corner(1.0, 1.0, -1.0),
        corner(1.0, 1.0, 1.0),
        corner(-1.0, 1.0, 1.0),
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let colour = ink.extend(alpha).to_array();
    for (from, to) in EDGES {
        for point in [corners[from], corners[to]] {
            frame.lines.push(LineVertex {
                position: point.to_array(),
                _pad: 0.0,
                colour,
            });
        }
    }
}
