//! Deterministic, budgeted audience silhouettes for scalable Venue crowd footprints.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Quat, Vec3};
use viz_scene::{CrowdArea, CrowdPosture, RenderQuality, Scene, euler_degrees};

const PERSON_RADIUS: f32 = 0.16;
const HIGH_PERSON_BUDGET: usize = 384;
const ULTRA_PERSON_BUDGET: usize = 1_024;

pub(super) fn push_crowds(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    let quality_budget = match style.quality {
        RenderQuality::Draft | RenderQuality::Standard => return,
        RenderQuality::High => HIGH_PERSON_BUDGET,
        RenderQuality::Ultra => ULTRA_PERSON_BUDGET,
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
    let mut random = SplitMix64::new(crowd.seed);
    let half_width = crowd.width_metres * 0.5;
    let half_depth = crowd.depth_metres * 0.5;
    let margin_x = PERSON_RADIUS.min((half_width * 0.95).max(0.0));
    let margin_z = PERSON_RADIUS.min((half_depth * 0.95).max(0.0));
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
        let shade = 0.035 + (index % 7) as f32 * 0.006;
        push_person(
            frame,
            crowd.position + orientation * local,
            orientation * Quat::from_rotation_y(yaw),
            crowd.posture,
            Vec3::splat(shade),
            &mut random,
        );
    }
}

fn push_person(
    frame: &mut FrameInstances,
    floor: Vec3,
    orientation: Quat,
    posture: CrowdPosture,
    colour: Vec3,
    random: &mut SplitMix64,
) {
    let (body_height, body_y, head_y, leg_height) = match posture {
        CrowdPosture::Sitting => (0.54, 0.72, 1.12, 0.42),
        CrowdPosture::StandingStill | CrowdPosture::Dancing => (0.78, 1.02, 1.57, 0.76),
    };
    push_part(
        frame,
        MeshKind::Cylinder,
        Vec3::new(0.32, body_height, 0.22),
        orientation,
        floor + Vec3::Y * body_y,
        colour,
    );
    push_part(
        frame,
        MeshKind::Sphere,
        Vec3::splat(0.28),
        orientation,
        floor + Vec3::Y * head_y,
        colour * 1.08,
    );

    let dance = posture == CrowdPosture::Dancing;
    for side in [-1.0_f32, 1.0] {
        let leg_origin = if posture == CrowdPosture::Sitting {
            floor + orientation * Vec3::new(side * 0.10, 0.30, 0.16)
        } else {
            floor + orientation * Vec3::new(side * 0.09, leg_height * 0.5, 0.0)
        };
        let leg_tilt = if posture == CrowdPosture::Sitting {
            Quat::from_rotation_x(-0.48)
        } else {
            Quat::IDENTITY
        };
        push_part(
            frame,
            MeshKind::Cylinder,
            Vec3::new(0.11, leg_height, 0.11),
            orientation * leg_tilt,
            leg_origin,
            colour,
        );

        let arm_angle = if dance {
            random.range(-1.15, 1.15)
        } else if posture == CrowdPosture::Sitting {
            side * 0.18
        } else {
            side * 0.08
        };
        let arm_orientation = orientation * Quat::from_rotation_z(arm_angle);
        push_part(
            frame,
            MeshKind::Cylinder,
            Vec3::new(0.09, 0.58, 0.09),
            arm_orientation,
            floor
                + Vec3::Y * (body_y + 0.08)
                + arm_orientation * Vec3::Y * -0.18
                + orientation * Vec3::X * side * 0.20,
            colour,
        );
    }
}

fn push_part(
    frame: &mut FrameInstances,
    kind: MeshKind,
    size: Vec3,
    orientation: Quat,
    centre: Vec3,
    colour: Vec3,
) {
    frame.mesh(kind).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(size, orientation, centre),
        colour,
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
        assert_eq!(high.crowd_drawn, HIGH_PERSON_BUDGET as u32);
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
    fn generated_body_centres_remain_inside_the_footprint() {
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
        for (_, instances) in frame.meshes {
            for instance in instances {
                let x = instance.model[3][0];
                let y = instance.model[3][1];
                let z = instance.model[3][2];
                assert!((-1.0..=1.0).contains(&x), "x={x}");
                assert!((0.0..=1.8).contains(&y), "y={y}");
                assert!((-0.5..=0.5).contains(&z), "z={z}");
            }
        }
    }
}
