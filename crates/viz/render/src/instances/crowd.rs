//! Deterministic, budgeted audience silhouettes for scalable Venue crowd footprints.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Quat, Vec3};
use viz_scene::{CrowdArea, CrowdPosture, RenderQuality, Scene, euler_degrees};

const PERSON_MARGIN: f32 = 0.5;

pub(super) fn push_crowds(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    let quality_budget = match style.quality {
        RenderQuality::Draft | RenderQuality::Standard => return,
        RenderQuality::High | RenderQuality::Ultra | RenderQuality::Extreme => {
            style.crowd_person_budget
        }
    };
    let amount = style.crowd_amount.clamp(0.0, 1.0);
    if amount <= 0.0 {
        frame.crowd_authored = authored_people(scene) as u32;
        return;
    }

    let mut remaining = quality_budget;
    for crowd in &scene.crowds {
        let authored = population(crowd);
        frame.crowd_authored = frame.crowd_authored.saturating_add(authored as u32);
        let requested = ((authored as f32 * amount).round() as usize).min(authored);
        frame.crowd_requested = frame.crowd_requested.saturating_add(requested as u32);
        let draw = requested.min(remaining);
        remaining = remaining.saturating_sub(draw);
        frame.crowd_drawn = frame.crowd_drawn.saturating_add(draw as u32);
        push_people(frame, crowd, draw);
        if remaining == 0 {
            break;
        }
    }
}

fn authored_people(scene: &Scene) -> usize {
    scene.crowds.iter().map(population).sum()
}

fn population(crowd: &CrowdArea) -> usize {
    let area = crowd.width_metres.max(0.0) * crowd.depth_metres.max(0.0);
    (area * crowd.density.people_per_square_metre())
        .round()
        .max(1.0) as usize
}

fn push_people(frame: &mut FrameInstances, crowd: &CrowdArea, count: usize) {
    let orientation = euler_degrees(crowd.rotation_degrees);
    let mut random = SplitMix64::new(population_seed(crowd));
    let half_width = crowd.width_metres * 0.5;
    let half_depth = crowd.depth_metres * 0.5;
    let margin_x = PERSON_MARGIN.min((half_width * 0.95).max(0.0));
    let margin_z = PERSON_MARGIN.min((half_depth * 0.95).max(0.0));
    for index in 0..count {
        let local = Vec3::new(
            random.range(-half_width + margin_x, half_width - margin_x),
            0.0,
            random.range(-half_depth + margin_z, half_depth - margin_z),
        );
        let yaw = random.range(-0.35, 0.35)
            + if crowd.posture == CrowdPosture::Dancing {
                random.range(-0.55, 0.55)
            } else {
                0.0
            };
        let height = person_height(index);
        push_person(
            frame,
            crowd.position + orientation * local,
            orientation * Quat::from_rotation_y(yaw),
            crowd.posture,
            height,
        );
    }
}

fn person_height(index: usize) -> f32 {
    1.60 + ((index.saturating_mul(73).saturating_add(41) % 251) as f32 / 1000.0)
}

fn population_seed(crowd: &CrowdArea) -> u64 {
    let posture = match crowd.posture {
        CrowdPosture::Sitting => 0x51_74_54,
        CrowdPosture::StandingStill => 0x57_41_4e_44,
        CrowdPosture::Dancing => 0x44_41_4e_43_45,
    };
    let density: u64 = match crowd.density {
        viz_scene::CrowdDensity::Sparse => 0x53_50_41_52_53_45,
        viz_scene::CrowdDensity::Medium => 0x4d_45_44_49_55_4d,
        viz_scene::CrowdDensity::Dense => 0x44_45_4e_53_45,
    };
    crowd.seed
        ^ posture
        ^ density.rotate_left(17)
        ^ u64::from(crowd.width_metres.to_bits()).rotate_left(29)
        ^ u64::from(crowd.depth_metres.to_bits()).rotate_left(43)
}

fn push_person(
    frame: &mut FrameInstances,
    floor: Vec3,
    orientation: Quat,
    posture: CrowdPosture,
    height: f32,
) {
    let posture_scale = match posture {
        CrowdPosture::Sitting => 0.72,
        CrowdPosture::StandingStill | CrowdPosture::Dancing => 1.0,
    };
    let rendered_height = height * posture_scale;
    frame.mesh(MeshKind::CrowdPerson).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(
            Vec3::new(rendered_height, rendered_height, 1.0),
            orientation,
            floor,
        ),
        Vec3::splat(0.008),
        0.92,
        Vec3::ZERO,
        0.0,
    ));
}

/// Small deterministic generator with stable output across platforms and renderer restarts.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        if max <= min {
            return (min + max) * 0.5;
        }
        let unit = (self.next() >> 40) as f32 / (1_u32 << 24) as f32;
        min + (max - min) * unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::{CrowdDensity, Scene};

    fn area() -> CrowdArea {
        CrowdArea {
            id: viz_scene::uuid::Uuid::nil(),
            name: "Audience".into(),
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            width_metres: 20.0,
            depth_metres: 20.0,
            posture: CrowdPosture::StandingStill,
            density: CrowdDensity::Dense,
            seed: 42,
        }
    }

    #[test]
    fn quality_and_amount_apply_stable_bounded_subsets() {
        let scene = Scene {
            crowds: vec![area()],
            ..Scene::default()
        };
        let omitted = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::Standard,
                ..FrameStyle::default()
            },
        );
        assert_eq!(omitted.crowd_drawn, 0);

        let high = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::High,
                crowd_amount: 1.0,
                ..FrameStyle::default()
            },
        );
        assert_eq!(high.crowd_drawn, 384);
        assert!(high.crowd_authored > high.crowd_drawn);
        let again = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::High,
                crowd_amount: 1.0,
                ..FrameStyle::default()
            },
        );
        assert_eq!(high.meshes[0].1[0].model, again.meshes[0].1[0].model);

        let none = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::Ultra,
                crowd_amount: 0.0,
                ..FrameStyle::default()
            },
        );
        assert_eq!(none.crowd_drawn, 0);
        assert_eq!(none.crowd_authored, high.crowd_authored);
    }

    #[test]
    fn complete_generated_bodies_remain_inside_the_footprint() {
        let crowd = CrowdArea {
            width_metres: 2.0,
            depth_metres: 1.0,
            ..area()
        };
        let scene = Scene {
            crowds: vec![crowd],
            ..Scene::default()
        };
        let frame = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::Ultra,
                ..FrameStyle::default()
            },
        );
        let silhouette = crate::mesh::unit_crowd_person();
        for (_, instances) in frame.meshes {
            for instance in instances {
                let model = Mat4::from_cols_array_2d(&instance.model);
                for vertex in &silhouette.vertices {
                    let world = model.transform_point3(Vec3::from_array(vertex.position));
                    assert!((-1.0..=1.0).contains(&world.x), "x={}", world.x);
                    assert!((0.0..=1.85).contains(&world.y), "y={}", world.y);
                    assert!((-0.5..=0.5).contains(&world.z), "z={}", world.z);
                }
            }
        }
    }

    #[test]
    fn mode_and_footprint_are_deterministic_inputs_without_changing_person_height() {
        let base = area();
        let render = |crowd: CrowdArea| {
            let scene = Scene {
                crowds: vec![crowd],
                ..Scene::default()
            };
            super::super::build(
                &scene,
                &viz_scene::SceneValues::default(),
                &FrameStyle {
                    quality: RenderQuality::High,
                    ..FrameStyle::default()
                },
            )
        };
        let first = render(base.clone());
        let restarted = render(base.clone());
        assert_eq!(first.meshes[0].1[0].model, restarted.meshes[0].1[0].model);

        let resized = render(CrowdArea {
            width_metres: 30.0,
            depth_metres: 12.0,
            ..base.clone()
        });
        assert_ne!(first.meshes[0].1[0].model, resized.meshes[0].1[0].model);
        assert_eq!(
            first.meshes[0].1[0].model[1][1], resized.meshes[0].1[0].model[1][1],
            "footprint size must not scale a person's height"
        );

        let sitting = render(CrowdArea {
            posture: CrowdPosture::Sitting,
            ..base
        });
        assert_ne!(first.meshes[0].1[0].model, sitting.meshes[0].1[0].model);
        assert!(sitting.meshes[0].1[0].model[1][1] < first.meshes[0].1[0].model[1][1]);
    }

    #[test]
    fn every_drawn_person_is_one_black_flat_silhouette() {
        let scene = Scene {
            crowds: vec![area()],
            ..Scene::default()
        };
        let frame = super::super::build(
            &scene,
            &viz_scene::SceneValues::default(),
            &FrameStyle {
                quality: RenderQuality::High,
                crowd_amount: 0.1,
                ..FrameStyle::default()
            },
        );
        let silhouettes = frame
            .meshes
            .iter()
            .find(|(kind, _)| *kind == MeshKind::CrowdPerson)
            .expect("crowd silhouette mesh");
        assert_eq!(silhouettes.1.len(), frame.crowd_drawn as usize);
        assert!(
            silhouettes
                .1
                .iter()
                .all(|person| person.base_colour[..3] == [0.008; 3])
        );
        assert!(
            frame
                .meshes
                .iter()
                .all(|(kind, _)| !matches!(kind, MeshKind::Sphere | MeshKind::Cylinder))
        );
        for person in &silhouettes.1 {
            let height = person.model[1][1].abs();
            assert!((1.60..=1.85).contains(&height), "height={height}");
        }
    }
}
