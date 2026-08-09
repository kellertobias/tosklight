// Particles thrown outward on a bass hit, pulled down by gravity, fading over a lifetime.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let particles = max(count(), 1.0);
    var glow = 0.0;
    var index = 0.0;
    loop {
        if index >= particles { break; }
        let seed = index + 1.0;
        // Each particle belongs to a burst; bursts advance with time and jump on a beat.
        let burst = floor(seconds() / max(lifetime(), 0.05) + hash11(seed) );
        let age = fract(seconds() / max(lifetime(), 0.05) + hash11(seed));
        let angle = hash11(seed + burst * 13.0) * TAU;
        let velocity = (0.4 + hash11(seed + burst * 7.0)) * speed();
        let travelled = age * velocity;
        let position = vec2<f32>(cos(angle), sin(angle)) * travelled
            - vec2<f32>(0.0, gravity() * age * age * 2.0);
        let brightness = (1.0 - age) * (0.3 + beat() * 0.7);
        glow += brightness * size() * 0.4 / (length(p - position) + size() * 0.4);
        index += 1.0;
    }
    // Additive: overlapping particles brighten rather than occlude.
    let intensity = clamp(glow * 0.55 * reactivity(), 0.0, 1.0);
    return vec4<f32>(primary() * intensity * 2.0, intensity);
}
