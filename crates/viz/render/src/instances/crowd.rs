//! Deterministic, budgeted audience silhouettes for scalable Venue crowd footprints.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Quat, Vec3};
use viz_scene::{CrowdArea, CrowdPosture, RenderQuality, Scene, euler_degrees};

const PERSON_MARGIN: f32 = 0.5;

pub(super) fn push_crowds(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    let quality_budget = match style.quality {
        RenderQuality::Draft | RenderQuality::Standard => return,
        RenderQuality::High | RenderQuality::Ultra => style.crowd_person_budget,
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
        push_person(
            frame,
            crowd.position + orientation * local,
            orientation * Quat::from_rotation_y(yaw),
            crowd.posture,
            person_size(crowd.seed, index),
        );
    }
}

/// Height of an average person in the crowd, in metres.
const PERSON_HEIGHT: f32 = 1.72;

/// How tall and how wide one person is drawn: each scaled on its own between 0.85 and 1.15 of an
/// average person. It follows only the crowd's seed and the person's index, so a person keeps
/// their size from frame to frame and when the footprint, posture or amount drawn changes.
fn person_size(seed: u64, index: usize) -> PersonSize {
    let mut random = SplitMix64::new(
        seed ^ (index as u64)
            .wrapping_add(1)
            .wrapping_mul(0xd1b5_4a32_d192_ed03),
    );
    PersonSize {
        height: PERSON_HEIGHT * random.range(0.85, 1.15),
        width: random.range(0.85, 1.15),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PersonSize {
    /// Standing height in metres.
    height: f32,
    /// Width against a person of that height with average proportions.
    width: f32,
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
    size: PersonSize,
) {
    let posture_scale = match posture {
        CrowdPosture::Sitting => 0.72,
        CrowdPosture::StandingStill | CrowdPosture::Dancing => 1.0,
    };
    let rendered_height = size.height * posture_scale;
    let scale = Vec3::new(rendered_height * size.width, rendered_height, 1.0);
    frame.mesh(MeshKind::CrowdPerson).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(scale, orientation, floor),
        Vec3::splat(0.008),
        0.92,
        Vec3::ZERO,
        0.0,
    ));
    frame
        .mesh(MeshKind::CrowdPersonOutline)
        .push(MeshInstance::new(
            Mat4::from_scale_rotation_translation(scale, orientation, floor),
            Vec3::splat(0.42),
            0.9,
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
                    assert!((0.0..=2.0).contains(&world.y), "y={}", world.y);
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
    fn every_drawn_person_is_one_black_flat_silhouette_with_authored_outlines() {
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
        let outlines = frame
            .meshes
            .iter()
            .find(|(kind, _)| *kind == MeshKind::CrowdPersonOutline)
            .expect("crowd outline mesh");
        assert_eq!(outlines.1.len(), frame.crowd_drawn as usize);
        assert!(
            silhouettes
                .1
                .iter()
                .all(|person| person.base_colour[..3] == [0.008; 3])
        );
        assert!(
            outlines
                .1
                .iter()
                .all(|person| person.base_colour[..3] == [0.42; 3])
        );
        assert!(
            frame
                .meshes
                .iter()
                .all(|(kind, _)| !matches!(kind, MeshKind::Sphere | MeshKind::Cylinder))
        );
        for person in &silhouettes.1 {
            let height = person.model[1][1].abs();
            assert!((1.46..=1.98).contains(&height), "height={height}");
        }
    }

    /// People differ in height and build, and each keeps their own size from frame to frame.
    #[test]
    fn each_person_has_their_own_stable_height_and_width() {
        let scene = Scene {
            crowds: vec![area()],
            ..Scene::default()
        };
        let style = FrameStyle {
            quality: RenderQuality::High,
            crowd_amount: 1.0,
            ..FrameStyle::default()
        };
        let first = super::super::build(&scene, &viz_scene::SceneValues::default(), &style);
        let again = super::super::build(&scene, &viz_scene::SceneValues::default(), &style);
        let people = |frame: &super::super::FrameInstances| {
            frame
                .meshes
                .iter()
                .find(|(kind, _)| *kind == MeshKind::CrowdPerson)
                .map(|(_, people)| people.iter().map(|person| person.model).collect::<Vec<_>>())
                .unwrap()
        };
        let (first, again) = (people(&first), people(&again));
        assert_eq!(first, again, "the same people at the same sizes");

        let heights: Vec<f32> = first.iter().map(|model| model[1][1]).collect();
        let builds: Vec<f32> = first
            .iter()
            .map(|model| model[0][0].hypot(model[0][2]) / model[1][1])
            .collect();
        let spread = |values: &[f32]| {
            let low = values.iter().copied().fold(f32::MAX, f32::min);
            let high = values.iter().copied().fold(f32::MIN, f32::max);
            (low, high)
        };
        let (short, tall) = spread(&heights);
        assert!(short >= PERSON_HEIGHT * 0.85 - 1e-3 && tall <= PERSON_HEIGHT * 1.15 + 1e-3);
        assert!(tall - short > 0.3, "heights {short}..{tall}");
        let (narrow, wide) = spread(&builds);
        assert!(narrow >= 0.85 - 1e-3 && wide <= 1.15 + 1e-3);
        assert!(wide - narrow > 0.2, "widths {narrow}..{wide}");

        assert_eq!(person_size(42, 7), person_size(42, 7));
        assert_ne!(person_size(42, 7), person_size(43, 7));
        assert_ne!(person_size(42, 7), person_size(42, 8));
    }
}
