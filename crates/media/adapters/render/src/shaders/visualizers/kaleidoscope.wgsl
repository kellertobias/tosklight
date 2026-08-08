// One motif, folded into rotating radial segments.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let segments = max(floor(count()), 2.0);
    let spin = seconds() * speed() * 0.3 + bass() * reactivity();
    let angle = atan2(p.y, p.x) + spin;
    let wedge = TAU / segments;
    // Folding rather than repeating: the motif mirrors at every seam, as a real kaleidoscope does.
    let folded = abs(fract(angle / wedge) - 0.5) * wedge;
    let distance = length(p) * zoom();
    let point = vec2<f32>(cos(folded), sin(folded)) * distance;

    // Two scales of noise crossed with a travelling ring, so the motif has structure instead of
    // resolving into one flat blob wherever the noise happens to be high.
    let motif = value_noise(point * 5.0 + vec2<f32>(0.0, seconds() * speed() * 0.2)) * 0.65
        + value_noise(point * 13.0 - vec2<f32>(seconds() * speed() * 0.1, 0.0)) * 0.35;
    let rings = abs(fract(distance * 2.5 - seconds() * speed() * 0.15) - 0.5) * 2.0;
    let bright = smoothstep(0.35, 0.85, motif * rings + bass() * 0.15);
    return vec4<f32>(mix(primary() * 0.3, primary(), bright), bright);
}
