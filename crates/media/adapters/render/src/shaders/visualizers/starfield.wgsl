// A perspective starfield drifting toward the viewer; hi-hats make it sparkle. Louder music speeds
// the drift up, gliding rather than jumping. On beat, every landed beat launches a fresh volley of
// stars from the vanishing point over a sparse field that keeps drifting.

/// Depth covered per second of flow at speed one: a star takes around ten seconds from the
/// vanishing point to the edge of the picture.
const DRIFT: f32 = 0.05;

/// One star at `depth` along `direction`, `0` far away and `1` passing the viewer.
fn star(p: vec2<f32>, direction: vec2<f32>, depth: f32) -> f32 {
    let position = direction * depth * depth * 2.0;
    let brightness = depth * depth;
    let point_size = 0.004 + depth * 0.014;
    return brightness * point_size / (length(p - position) + point_size);
}

/// A well-mixed integer hash. The sine hashes line consecutive seeds up along a few diagonals,
/// which a burst of stars launched together would show as a cross.
fn scramble(value: u32) -> u32 {
    let state = value * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(key: u32) -> f32 {
    return f32(scramble(key) >> 8u) / 16777216.0;
}

fn direction_of(key: u32) -> vec2<f32> {
    let angle = unit(key) * TAU;
    return vec2<f32>(cos(angle), sin(angle));
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let stars = max(count(), 1.0);
    let travel = flow() * DRIFT;
    var glow = 0.0;

    // The drifting field. On beat it thins to a quarter, so the volleys carry the picture.
    let drifting = select(stars, ceil(stars * 0.25), on_beat());
    var index = 0.0;
    loop {
        if index >= drifting { break; }
        let seed = index + 1.0;
        // Depth wraps, so a star that passes the viewer reappears far away.
        let depth = fract(hash11(seed) + travel);
        glow += star(p, direction_of(u32(index)), depth);
        index += 1.0;
    }

    if on_beat() {
        // Enough stars per beat that about `count` are in flight while the beats keep coming. A
        // volley starts a little out from the centre and then drifts like the rest of the field.
        let per_beat = clamp(ceil(stars / 6.0), 1.0, 24.0);
        // Faster than the drift so a volley reads, and fixed by speed alone: a star in flight never
        // jumps when the music gets louder.
        let rate = DRIFT * max(speed(), 0.05) * 3.0;
        var landed = 0;
        loop {
            if landed >= BEAT_HISTORY { break; }
            let depth_start = 0.12;
            let travelled = beat_age(landed) * rate;
            if depth_start + travelled * 0.7 > 1.0 { break; }
            let volley = beat_count() - f32(landed);
            var launched = 0.0;
            loop {
                if launched >= per_beat { break; }
                let key = u32(max(volley, 0.0)) * 64u + u32(launched) + 65536u;
                // Each star of a volley starts at its own depth, so the volley reads as a burst.
                let depth = depth_start + travelled * mix(0.7, 1.3, unit(key ^ 0x5bd1e995u));
                if depth < 1.0 {
                    glow += star(p, direction_of(key), depth) * 1.4;
                }
                launched += 1.0;
            }
            landed += 1;
        }
    }

    let intensity = clamp(glow * (1.6 + hihat() * reactivity() * 1.2), 0.0, 1.0);
    return vec4<f32>(primary() * intensity, intensity);
}
