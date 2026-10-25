//! CPU sampling of common.wgsl::beam_profile. Keep native gate transforms and constants together.
use glam::Vec2;
use viz_scene::GoboArtwork;
pub(super) const PEAK: f32 = 2.75;
#[derive(Clone)]
pub(super) struct Gate {
    pub cos_outer: f32,
    pub params: [f32; 4],
    pub frost: f32,
    pub shapers: [f32; 4],
    pub angles: [f32; 4],
    /// Private CPU-baking input, not serialized in ImageManifest. Zero is legacy dynamic softness.
    pub shaper_params: [f32; 4],
    pub gobos: [[f32; 4]; 8],
    pub prisms: [[f32; 4]; 8],
    pub gobo_count: usize,
    pub prism_count: usize,
}
impl Gate {
    pub fn key(&self) -> Vec<u32> {
        [self.cos_outer, self.frost]
            .into_iter()
            .chain(self.params)
            .chain(self.shapers)
            .chain(self.angles)
            .chain(self.shaper_params)
            .chain(self.gobos.iter().flatten().copied())
            .chain(self.prisms.iter().flatten().copied())
            .map(f32::to_bits)
            .chain([self.gobo_count as u32, self.prism_count as u32])
            .collect()
    }
    pub fn invalid_reason(&self, artwork: &[GoboArtwork]) -> Option<String> {
        let key = self.key();
        // Exclude only the two integer wheel counts; every float, including reserved slots,
        // participates in finiteness checks when the native storage contract grows.
        if key[..key.len() - 2]
            .iter()
            .any(|bits| !f32::from_bits(*bits).is_finite())
            || self.gobo_count > 8
            || self.prism_count > 8
            || !(0.0..1.0).contains(&self.cos_outer)
        {
            return Some("Frozen optical coordinates are outside the supported finite cone".into());
        }
        let manual = self.shaper_params[0] > 0.0;
        let maximum_stroke = if manual { 2.0 } else { 1.0 };
        if !(0.0..=1.0).contains(&self.shaper_params[0])
            || self.shaper_params[1..].iter().any(|value| *value != 0.0)
            || self
                .shapers
                .iter()
                .any(|blade| !(0.0..=maximum_stroke).contains(blade))
        {
            return Some("Frozen shutter coordinates exceed their dynamic/manual stroke or softness contract".into());
        }
        for gobo in self
            .gobos
            .iter()
            .take(self.gobo_count)
            .filter(|gobo| gobo[0] > 0.0 && gobo[2] >= 0.0)
        {
            let Some(image) = artwork.get(gobo[2] as usize) else {
                return Some("Active gobo references missing native artwork".into());
            };
            if image.edge == 0
                || (image.edge as usize).checked_mul(image.edge as usize) != Some(image.mask.len())
            {
                return Some("Active native gobo artwork has invalid dimensions".into());
            }
        }
        None
    }
    pub fn transmission(&self, mut gate: Vec2, artwork: &[GoboArtwork]) -> f32 {
        let tangent =
            (1.0 - self.cos_outer * self.cos_outer).max(1e-6).sqrt() / self.cos_outer.max(1e-3);
        let cosine = 1.0 / (1.0 + gate.length_squared() * tangent * tangent).sqrt();
        if cosine <= self.cos_outer {
            return 0.0;
        }
        let radial = ((1.0 - cosine) / (1.0 - self.cos_outer).max(1e-5)).clamp(0.0, 1.0);
        let hotspot = 0.35 + 2.4 * (1.0 - radial).powf(2.5);
        let mut profile = smooth(
            self.cos_outer,
            self.params[0].max(self.cos_outer + 0.0005),
            cosine,
        ) * mix(hotspot, 1.0, self.params[2].clamp(0.0, 1.0));
        let shaped = self.shapers.iter().sum::<f32>() > 0.001;
        let active = shaped
            || self
                .gobos
                .iter()
                .take(self.gobo_count)
                .any(|gobo| gobo[0] > 0.0)
            || self
                .prisms
                .iter()
                .take(self.prism_count)
                .any(|prism| prism[0] >= 2.0);
        if !active {
            return profile;
        }
        let mut aperture = 1.0;
        for prism in self
            .prisms
            .iter()
            .take(self.prism_count)
            .filter(|prism| prism[0] >= 2.0)
        {
            let rotated = rotate(gate, prism[1]);
            let copy = if prism[2] > 0.5 {
                let spacing = (2.0 * prism[3] / (prism[0] - 1.0).max(1.0)).max(0.0001);
                let index = (rotated.x / spacing + (prism[0] - 1.0) * 0.5)
                    .round()
                    .clamp(0.0, prism[0] - 1.0);
                let centre = (index - (prism[0] - 1.0) * 0.5) * spacing;
                Vec2::new(rotated.x - centre, rotated.y) / (spacing * 0.42).clamp(0.025, 0.35)
            } else {
                let wedge = std::f32::consts::TAU / prism[0];
                let heading = rotated.y.atan2(rotated.x);
                let folded = heading - (heading / wedge).floor() * wedge - wedge * 0.5;
                let radius =
                    (prism[3] * (std::f32::consts::PI / prism[0]).sin() * 0.85).clamp(0.025, 0.35);
                (Vec2::new(folded.cos(), folded.sin()) * rotated.length()
                    - Vec2::new(prism[3], 0.0))
                    / radius
            };
            gate = rotate(copy, -prism[1]);
            aperture *= 1.0 - smooth(0.82, 1.0, copy.length());
        }
        let mut transmission = 1.0;
        for gobo in self
            .gobos
            .iter()
            .take(self.gobo_count)
            .filter(|gobo| gobo[0] > 0.0)
        {
            let point = rotate(gate, gobo[1]);
            transmission *= if gobo[2] >= 0.0 {
                sample(&artwork[gobo[2] as usize], point)
            } else {
                native_pattern(gobo[0] as u32, point)
            };
        }
        let frost = (self.frost * 1.4).clamp(0.0, 0.95);
        profile *= mix(transmission, 1.0, frost);
        if shaped {
            profile *= self.shapers(gate);
        }
        profile * mix(aperture, 1.0, frost)
    }
    fn shapers(&self, gate: Vec2) -> f32 {
        let edge = if self.shaper_params[0] > 0.0 {
            self.shaper_params[0]
        } else {
            (self.params[1] + self.frost).max(0.02)
        };
        let coordinates = [
            rotate(gate, self.angles[0]).x,
            -rotate(gate, self.angles[1]).x,
            rotate(gate, self.angles[2]).y,
            -rotate(gate, self.angles[3]).y,
        ];
        self.shapers
            .iter()
            .zip(coordinates)
            .map(|(blade, position)| {
                if self.shaper_params[0] > 0.0 {
                    if *blade <= 0.0 {
                        return 1.0;
                    }
                    if *blade >= 2.0 {
                        return 0.0;
                    }
                }
                smooth(1.0 - blade + edge, 1.0 - blade - edge, position)
            })
            .product()
    }
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn rotate(point: Vec2, angle: f32) -> Vec2 {
    Vec2::new(
        point.x * angle.cos() - point.y * angle.sin(),
        point.x * angle.sin() + point.y * angle.cos(),
    )
}
fn sample(image: &GoboArtwork, point: Vec2) -> f32 {
    let uv = point * 0.5 + Vec2::splat(0.5);
    if uv.min_element() < 0.0 || uv.max_element() > 1.0 {
        return 0.0;
    }
    let pixel = uv * image.edge as f32 - Vec2::splat(0.5);
    let base = pixel.floor();
    let fraction = pixel - base;
    let fetch = |x: f32, y: f32| {
        let x = x.clamp(0.0, image.edge as f32 - 1.0) as usize;
        let y = y.clamp(0.0, image.edge as f32 - 1.0) as usize;
        image.mask[y * image.edge as usize + x] as f32 / 255.0
    };
    mix(
        mix(
            fetch(base.x, base.y),
            fetch(base.x + 1.0, base.y),
            fraction.x,
        ),
        mix(
            fetch(base.x, base.y + 1.0),
            fetch(base.x + 1.0, base.y + 1.0),
            fraction.x,
        ),
        fraction.y,
    )
}
/// These are the native renderer's declared generic fallback slots, never substituted for artwork.
fn native_pattern(slot: u32, p: Vec2) -> f32 {
    let radius = p.length();
    let angle = p.y.atan2(p.x);
    match slot {
        0 => 1.0,
        1 => smooth(
            -0.25,
            0.35,
            (p.x * 9.0).sin() * (p.y * 7.3).sin()
                + 0.6 * (p.x * 17.0 + 1.7).sin() * (p.y * 13.0 - 0.9).sin(),
        ),
        2 => {
            let q = p * 3.0;
            let grid = q - q.floor() - Vec2::splat(0.5);
            1.0 - smooth(0.16, 0.26, grid.length())
        }
        3 => smooth(0.1, 0.45, (angle * 6.0).sin().abs()),
        4 => smooth(0.15, 0.4, (radius * 14.0).sin().abs()),
        5 => 1.0 - smooth(0.42, 0.5, p.x.abs() + p.y.abs() * 0.8),
        6 => smooth(0.2, 0.5, (p.y * 8.0).sin().abs()),
        _ => 1.0 - smooth(0.35, 0.5, radius),
    }
}
