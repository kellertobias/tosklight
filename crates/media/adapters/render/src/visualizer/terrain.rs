//! The original terrain samples coherent noise once per mesh vertex, not per output pixel.
//! Bake the 50x50 height grid into spare analysis rows for the projected mesh shader.

fn gradient(x: i32, y: i32, z: i32, offset: [f32; 3]) -> f32 {
    let mut hash = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ (z as u32).wrapping_mul(2_147_483_647);
    hash = (hash ^ (hash >> 13)).wrapping_mul(1_274_126_177);
    hash ^= hash >> 16;
    let h = hash & 15;
    let u = if h < 8 { offset[0] } else { offset[1] };
    let v = if h < 4 {
        offset[1]
    } else if h == 12 || h == 14 {
        offset[0]
    } else {
        offset[2]
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

fn noise(point: [f32; 3]) -> f32 {
    let cell = point.map(|v| v.floor() as i32);
    let local = std::array::from_fn::<_, 3, _>(|i| point[i] - cell[i] as f32);
    let fade = local.map(|v| v * v * v * (v * (v * 6.0 - 15.0) + 10.0));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let mut levels = [0.0; 2];
    for (z, level) in levels.iter_mut().enumerate() {
        let mut rows = [0.0; 2];
        for (y, row) in rows.iter_mut().enumerate() {
            let sample = |x: i32| {
                gradient(
                    cell[0] + x,
                    cell[1] + y as i32,
                    cell[2] + z as i32,
                    [
                        local[0] - x as f32,
                        local[1] - y as f32,
                        local[2] - z as f32,
                    ],
                )
            };
            *row = mix(sample(0), sample(1), fade[0]);
        }
        *level = mix(rows[0], rows[1], fade[1]);
    }
    (0.5 + mix(levels[0], levels[1], fade[2]) * 0.5).clamp(0.0, 1.0)
}

pub(super) fn heights(clock: f32, bass: f32) -> [f32; 2500] {
    std::array::from_fn(|i| {
        noise([
            (i % 50) as f32 * 0.1,
            (i / 50) as f32 * 0.1 - clock * 0.5,
            bass * 0.4,
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_is_bounded_and_moves_in_time_and_the_audio_dimension() {
        let initial = heights(0.0, 0.0);
        assert!(initial.iter().all(|v| (0.0..=1.0).contains(v)));
        assert_ne!(initial, heights(0.1, 0.0));
        assert_ne!(initial, heights(0.0, 0.8));
        assert_eq!(initial, heights(0.0, 0.0));
    }
}
