// Bolts from the centre, their paths perturbed by noise, struck on bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let bolts = max(floor(count()), 1.0);
    // A strike either exceeds the threshold or happens by chance, so the effect never goes still.
    let strike = step(threshold(), bass() * reactivity())
        + step(0.985, hash11(floor(seconds() * 12.0)));
    let intensity = clamp(strike, 0.0, 1.0) * (0.4 + beat() * 0.6);

    let distance = length(p);
    let angle = atan2(p.y, p.x);
    var glow = 0.0;
    var index = 0.0;
    loop {
        if index >= bolts { break; }
        let root = (index / bolts) * TAU + hash11(index + floor(seconds() * 6.0)) * 0.6;
        let wander = (value_noise(vec2<f32>(distance * 8.0, index * 3.0 + seconds() * 6.0)) - 0.5)
            * distance * 1.2;
        let deviation = abs(atan2(sin(angle - root), cos(angle - root)) - wander);
        let reach = size() * 8.0;
        glow += (1.0 - smoothstep(0.0, 0.06, deviation)) * (1.0 - smoothstep(0.0, reach, distance));
        index += 1.0;
    }
    let brightness = clamp(glow * intensity, 0.0, 1.0);
    return vec4<f32>(primary() * brightness * 1.5, brightness);
}
