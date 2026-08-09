// A noise-driven swarm: bass widens the cloud, mids fatten the particles.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let particles = max(count(), 1.0);
    let spread = radius() * (1.0 + bass() * reactivity());
    let dot_size = size() * (0.5 + mid() * reactivity());
    var glow = 0.0;
    var index = 0.0;
    loop {
        if index >= particles { break; }
        let seed = index + 1.0;
        let drift = seconds() * speed() * 0.2;
        let position = vec2<f32>(
            value_noise(vec2<f32>(seed * 3.1, drift)) - 0.5,
            value_noise(vec2<f32>(drift, seed * 5.7)) - 0.5,
        ) * 2.0 * spread;
        glow += dot_size / (length(p - position) + dot_size);
        index += 1.0;
    }
    let intensity = clamp(glow * 0.12, 0.0, 1.0);
    return vec4<f32>(primary() * intensity, intensity);
}
