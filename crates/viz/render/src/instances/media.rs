//! Media-surface and projector geometry.
//!
//! The dynamic texture pipeline owns pixels; this layer owns physical sections, module holes,
//! bezels, emissive contribution and the separately budgeted projector body/cone.

use super::*;
use viz_scene::{MediaSectionKind, RenderQuality};

pub(super) fn push_media(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    for section in &scene.media_sections {
        let orientation = euler_degrees(section.rotation_degrees);
        if !style.fixture_models {
            super::push_box_outline(
                frame,
                Mat4::from_scale_rotation_translation(
                    section.size,
                    orientation,
                    section.position,
                ),
                style.faint_ink,
                0.8,
            );
            continue;
        }
        let source_colour = section
            .source_id
            .map(stable_source_colour)
            .unwrap_or(Vec3::ZERO);
        match &section.kind {
            MediaSectionKind::ProjectionScreen {
                colour,
                gain,
                roughness,
                ..
            } => {
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    Mat4::from_scale_rotation_translation(
                        section.size,
                        orientation,
                        section.position,
                    ),
                    Vec3::from(*colour),
                    *roughness,
                    style
                        .media_content
                        .then_some(source_colour * gain.clamp(0.0, 4.0))
                        .unwrap_or(Vec3::ZERO),
                    0.0,
                ));
            }
            MediaSectionKind::Tv {
                bezel_metres,
                spill,
            } => {
                let bezel = bezel_metres.max(0.005);
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    Mat4::from_scale_rotation_translation(
                        section.size + Vec3::new(bezel * 2.0, bezel * 2.0, bezel),
                        orientation,
                        section.position,
                    ),
                    Vec3::splat(0.018),
                    0.22,
                    Vec3::ZERO,
                    0.15,
                ));
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    Mat4::from_scale_rotation_translation(
                        section.size,
                        orientation,
                        section.position + orientation * Vec3::Z * (bezel * 0.6),
                    ),
                    Vec3::splat(0.012),
                    0.12,
                    if style.media_content {
                        source_colour * (1.2 + spill.clamp(0.0, 1.0))
                    } else {
                        Vec3::ZERO
                    },
                    0.0,
                ));
            }
            MediaSectionKind::Led {
                rows,
                columns,
                occupied_cells,
                module_size,
                module_gap,
                pixel_pitch_millimetres,
            } => {
                if style.quality < RenderQuality::High {
                    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                        Mat4::from_scale_rotation_translation(
                            section.size,
                            orientation,
                            section.position,
                        ),
                        Vec3::splat(0.02),
                        0.4,
                        if style.media_content {
                            source_colour * 1.35
                        } else {
                            Vec3::ZERO
                        },
                        0.0,
                    ));
                    continue;
                }
                let occupied: std::collections::HashSet<u32> =
                    occupied_cells.iter().copied().collect();
                let module = Vec3::new(module_size[0], module_size[1], section.size.z);
                let stride = Vec3::new(
                    module_size[0] + module_gap[0],
                    module_size[1] + module_gap[1],
                    0.0,
                );
                let origin = Vec3::new(
                    -(f32::from(*columns) - 1.0) * stride.x * 0.5,
                    (f32::from(*rows) - 1.0) * stride.y * 0.5,
                    0.0,
                );
                for row in 0..*rows {
                    for column in 0..*columns {
                        let index = u32::from(row) * u32::from(*columns) + u32::from(column);
                        if !occupied.contains(&index) {
                            continue;
                        }
                        let local = origin
                            + Vec3::new(
                                f32::from(column) * stride.x,
                                -f32::from(row) * stride.y,
                                0.0,
                            );
                        let diode_gain =
                            (4.0_f32 / (*pixel_pitch_millimetres).max(1.0_f32)).clamp(0.65, 2.0);
                        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                            Mat4::from_scale_rotation_translation(
                                module,
                                orientation,
                                section.position + orientation * local,
                            ),
                            Vec3::splat(0.012),
                            0.5,
                            if style.media_content {
                                source_colour * 1.5 * diode_gain
                            } else {
                                Vec3::ZERO
                            },
                            0.0,
                        ));
                    }
                }
            }
        }
    }

    for projector in &scene.media_projectors {
        push_projector(frame, projector, style);
    }
}

fn stable_source_colour(id: viz_scene::uuid::Uuid) -> Vec3 {
    let bytes = id.as_bytes();
    Vec3::new(
        0.15 + f32::from(bytes[0]) / 340.0,
        0.12 + f32::from(bytes[5]) / 370.0,
        0.18 + f32::from(bytes[10]) / 330.0,
    )
}

fn push_projector(
    frame: &mut FrameInstances,
    projector: &viz_scene::MediaProjector,
    style: &FrameStyle,
) {
    let orientation = euler_degrees(projector.rotation_degrees);
    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.48, 0.2, 0.56),
            orientation,
            projector.position,
        ),
        Vec3::splat(0.045),
        0.32,
        Vec3::ZERO,
        0.15,
    ));
    let direction = (orientation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
    let origin = projector.position + direction * 0.3;
    let colour = stable_source_colour(projector.surface_id);
    frame.mesh(MeshKind::Lens).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.1, 0.1, 0.025),
            Quat::from_rotation_arc(Vec3::Z, direction),
            origin,
        ),
        Vec3::splat(0.04),
        0.18,
        if style.media_content {
            colour * 1.8
        } else {
            Vec3::ZERO
        },
        0.0,
    ));
    if !style.media_content || style.quality < RenderQuality::High {
        return;
    }
    let half_angle = 0.24_f32;
    let light_index = frame.lights.len() as u32;
    frame.lights.push(GpuLight {
        position_range: origin.extend(projector.cone_length_metres).to_array(),
        direction_cos_outer: direction.extend(half_angle.cos()).to_array(),
        colour_intensity: (colour * projector.spill.clamp(0.0, 1.0))
            .extend(projector.spill.clamp(0.0, 1.0))
            .to_array(),
        params: [(half_angle * 0.75).cos(), 0.25, 0.9, 0.0],
        tangent_frost: (orientation * Vec3::X).extend(0.15).to_array(),
        optics: [0.0; 4],
        shapers: [0.0; 4],
        shaper_angles: [0.0; 4],
        gate: [-1.0, 0.0, 0.0, 0.0],
        shadow: [-1.0, 0.0, 0.0, 0.0],
    });
    let total = projector.cone_length_metres;
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(half_angle.tan() * total, half_angle.tan() * total, total),
        Quat::from_rotation_arc(Vec3::Z, direction),
        origin,
    );
    frame.beams.push(BeamInstance {
        model: model.to_cols_array_2d(),
        colour: colour.extend(0.32).to_array(),
        params: [light_index as f32, total, 0.0, 0.0],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_source_is_drawn_once_per_authored_consumer_without_filling_sparse_led_holes() {
        let source = viz_scene::uuid::Uuid::new_v4();
        let mut scene = Scene::default();
        scene.media_sections.push(viz_scene::MediaSection {
            id: viz_scene::uuid::Uuid::new_v4(),
            surface_id: viz_scene::uuid::Uuid::new_v4(),
            name: "Sparse wall".into(),
            source_id: Some(source),
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            size: Vec3::new(2.0, 1.0, 0.04),
            crop: viz_scene::MediaCrop {
                left: 0.0,
                top: 0.0,
                width: 1.0,
                height: 1.0,
            },
            kind: MediaSectionKind::Led {
                rows: 2,
                columns: 3,
                occupied_cells: vec![0, 2, 4],
                module_size: [0.5, 0.5],
                module_gap: [0.01, 0.01],
                pixel_pitch_millimetres: 3.9,
            },
        });
        let mut frame = FrameInstances::default();
        push_media(
            &mut frame,
            &scene,
            &FrameStyle {
                quality: RenderQuality::High,
                ..FrameStyle::default()
            },
        );
        let modules = frame
            .meshes
            .iter()
            .find(|(kind, _)| *kind == MeshKind::Cube)
            .map(|(_, instances)| instances.len());
        assert_eq!(modules, Some(3));
    }

    #[test]
    fn helper_mode_keeps_geometry_but_removes_emission_and_projector_cones() {
        let mut scene = Scene::default();
        scene.media_projectors.push(viz_scene::MediaProjector {
            id: viz_scene::uuid::Uuid::new_v4(),
            surface_id: viz_scene::uuid::Uuid::new_v4(),
            name: "Projector".into(),
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            cone_length_metres: 10.0,
            spill: 0.4,
        });
        let mut frame = FrameInstances::default();
        push_media(
            &mut frame,
            &scene,
            &FrameStyle {
                media_content: false,
                quality: RenderQuality::Ultra,
                ..FrameStyle::default()
            },
        );
        assert!(!frame.meshes.is_empty());
        assert!(frame.lights.is_empty());
        assert!(frame.beams.is_empty());
    }
}
