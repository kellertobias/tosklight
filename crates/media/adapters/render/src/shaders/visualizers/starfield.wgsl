// A perspective starfield. Treble and bass hits push the travel speed up.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let stars = max(count(), 1.0);
    let travel = seconds() * speed() * (0.5 + treble() * reactivity() + beat() * 0.5);
    var glow = 0.0;
    var index = 0.0;
    loop {
        if index >= stars { break; }
        let seed = index + 1.0;
        let direction = normalize(hash22(vec2<f32>(seed, seed * 1.7)) - 0.5 + vec2<f32>(0.001));
        // Depth wraps, so a star that passes the viewer reappears far away.
        let depth = fract(hash11(seed) + travel * 0.2);
        let distance_from_centre = depth * depth * 2.0;
        let position = direction * distance_from_centre;
        let brightness = depth * depth;
        let point_size = 0.004 + depth * 0.014;
        glow += brightness * point_size / (length(p - position) + point_size);
        index += 1.0;
    }
    let intensity = clamp(glow * 1.6, 0.0, 1.0);
    return vec4<f32>(primary() * intensity, intensity);
}
