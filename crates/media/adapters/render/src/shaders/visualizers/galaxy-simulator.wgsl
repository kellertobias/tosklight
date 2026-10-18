// An inclined spiral galaxy with orbiting stars. Supernovas belong to actual landed beats;
// their stable seed and age come from the rhythm history, never from a free-running timer.
fn galaxy_rotate(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn galaxy_position(seed: f32, turn: f32) -> vec2<f32> {
    let distance = sqrt(hash11(seed + 3.0)) * max(radius(), 0.08) * 1.5;
    let arm = floor(hash11(seed + 7.0) * 3.0) * TAU / 3.0;
    let angle = arm + distance * 5.0 + turn + (hash11(seed + 9.0) - 0.5) * 0.9;
    return vec2<f32>(cos(angle), sin(angle) * 0.58) * distance;
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let turn = clock() * 0.12;
    let planar = galaxy_rotate(vec2<f32>(p.x, p.y / 0.58), -turn);
    let distance = length(planar);
    let angle = atan2(planar.y, planar.x);
    let extent = max(radius(), 0.08) * 1.5;
    let envelope = exp(-distance * distance / (extent * extent * 0.55));
    let arms = pow(0.5 + 0.5 * cos(angle * 3.0 - distance * 15.0), 5.0);
    let dust = 0.5 + 0.5 * value_noise(planar * 14.0 + vec2<f32>(turn));
    let core = exp(-distance * distance * 110.0 / max(extent, 0.1));
    var colour = primary() * envelope * (0.035 + arms * dust * 0.24)
        + mix(secondary(), vec3<f32>(1.0), 0.65) * core * 0.85;

    // Count has a fixed cost ceiling; each point belongs to a spiral arm rather than screen noise.
    let stars = min(count(), 192.0);
    for (var index = 0.0; index < stars; index += 1.0) {
        let seed = index * 3.17 + 17.0;
        let position = galaxy_position(seed, turn);
        let point_distance = length(p - position);
        let width = max(size(), 0.002) * (0.35 + hash11(seed + 2.0) * 0.7);
        let light = exp(-point_distance * point_distance / (width * width));
        let twinkle = 0.6 + 0.4 * sin(clock() * 1.3 + seed);
        colour += mix(primary(), vec3<f32>(1.0), hash11(seed)) * light * twinkle
            * (0.8 + smooth_treble() * reactivity() * 0.35);
    }

    let duration = max(lifetime(), 0.05);
    for (var history = 0; history < BEAT_HISTORY; history += 1) {
        let age = beat_age(history);
        if age >= duration { continue; }
        let progress = age / duration;
        let remaining = (1.0 - progress) * (1.0 - progress);
        for (var event = 0.0; event < min(burst(), 8.0); event += 1.0) {
            let seed = (beat_count() - f32(history)) * 19.73 + event * 41.1;
            // Keep an explosion attached to its star while the galaxy continues orbiting.
            let centre = galaxy_position(seed, turn);
            let d = length(p - centre);
            let expansion = 0.025 + progress * 0.32;
            let shell_width = 0.009 + progress * 0.012;
            let shell = exp(-pow((d - expansion) / shell_width, 2.0));
            let flash = exp(-d * d / (0.004 + progress * 0.012)) * exp(-progress * 9.0);
            colour += mix(secondary(), vec3<f32>(1.0), exp(-progress * 8.0))
                * (flash * 2.2 + shell * 0.8) * remaining * reactivity();
        }
    }
    return vec4<f32>(colour, 1.0);
}
