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
        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
            Mat4::from_rotation_translation(
                euler_degrees(object.rotation_degrees),
                object.position,
            ) * Mat4::from_scale(object.size),
            Vec3::ZERO,
            1.0,
            style.faint_ink * 0.12,
            0.0,
        ));
        push_box_outline(
            frame,
            object.position,
            object.size,
            euler_degrees(object.rotation_degrees),
            style.faint_ink,
            1.0,
        );
    }

    // Crowd Areas remain rectangular authored footprints on every plan. Individual people are a
    // quality-dependent 3D presentation and would make a plot unstable; the footprint is the
    // portable show intent the operator positions and scales.
    for crowd in &scene.crowds {
        let size = Vec3::new(crowd.width_metres, 0.02, crowd.depth_metres);
        let orientation = euler_degrees(crowd.rotation_degrees);
        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
            Mat4::from_rotation_translation(orientation, crowd.position + Vec3::Y * 0.01)
                * Mat4::from_scale(size),
            Vec3::ZERO,
            1.0,
            style.faint_ink * 0.1,
            0.0,
        ));
        push_box_outline(
            frame,
            crowd.position + Vec3::Y * 0.01,
            size,
            orientation,
            style.faint_ink,
            1.0,
        );
    }

    // A drawn plan is read, not admired. The symbols are the quietest thing on it: they say where
    // the lanterns are, and an operator looking at a plan is looking at where the light is going.
    // Drawn in the page's full ink they compete with the beams for the eye and win, because there
    // are far more of them.
    //
    // A selected fixture is the exception, and the only one. Selection is the question an operator
    // asks the plan most often — which of these am I about to change — so it is the one thing
    // allowed to stand out.
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
        let selected = values.selected_fixtures.contains(&fixture.fixture_id);
        let (ink, opacity) = if selected {
            (style.selected_ink, 1.0)
        } else if lights {
            (style.symbol_ink, 0.9)
        } else {
            (style.faint_ink * 0.7, 0.7)
        };
        let plan = scene
            .fixture_plan
            .iter()
            .find(|binding| binding.fixture_id == fixture.fixture_id);
        let packaged = plan.and_then(|binding| binding.artwork[style.projection_view.index()]);
        let transform = Mat4::from_rotation_translation(fixture.orientation(), fixture.position);
        if let Some(artwork) = packaged {
            frame
                .mesh(MeshKind::PlanArtwork(artwork))
                .push(MeshInstance::new(
                    transform,
                    Vec3::ZERO,
                    1.0,
                    ink * 0.72,
                    0.0,
                ));
            if selected {
                push_symbol(frame, fixture, style, style.selected_ink, 1.0);
            }
        } else {
            // Both renderer fallbacks are explicit opaque regions, so their depth can hide truss
            // and scenery. Recognized types add the renderer-owned vector convention; a truly
            // unknown declaration deliberately remains the final simple box.
            frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                transform * Mat4::from_scale(fixture.body.size),
                Vec3::ZERO,
                1.0,
                ink * 0.18,
                0.0,
            ));
            if plan.is_some_and(|binding| binding.fallback == viz_scene::PlanFallback::GenericType)
            {
                push_symbol(frame, fixture, style, ink, opacity);
            } else {
                push_box_outline(
                    frame,
                    fixture.position,
                    fixture.body.size,
                    fixture.orientation(),
                    ink,
                    opacity,
                );
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::{CrowdArea, CrowdDensity, CrowdPosture};

    #[test]
    fn plots_draw_the_authored_crowd_rectangle_without_people() {
        let scene = Scene {
            crowds: vec![CrowdArea {
                id: viz_scene::uuid::Uuid::nil(),
                name: "Audience".into(),
                position: Vec3::new(2.0, 0.0, -3.0),
                rotation_degrees: Vec3::new(0.0, 25.0, 0.0),
                width_metres: 12.0,
                depth_metres: 7.0,
                posture: CrowdPosture::Dancing,
                density: CrowdDensity::Dense,
                seed: 108,
            }],
            ..Scene::default()
        };
        let frame = super::super::build(
            &scene,
            &SceneValues::default(),
            &FrameStyle {
                plot: true,
                quality: viz_scene::RenderQuality::Ultra,
                ..FrameStyle::default()
            },
        );

        assert_eq!(frame.meshes.len(), 1);
        assert_eq!(frame.meshes[0].0, MeshKind::Cube);
        assert_eq!(
            frame.meshes[0].1.len(),
            1,
            "only the footprint fill is a mesh"
        );
        assert_eq!(
            frame.lines.len(),
            24,
            "the twelve rectangle edges are retained"
        );
        assert_eq!(frame.crowd_drawn, 0, "plots never expand people");
    }
}
