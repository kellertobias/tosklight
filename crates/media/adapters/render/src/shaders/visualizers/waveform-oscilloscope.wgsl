// The 512-point waveform drawn as a line, or filled to the centre.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    // Stabilizing anchors the trace to a rising zero crossing, so a steady tone stops sliding.
    let anchor = smoothing() * 0.25;
    let sample = wave_at(uv.x * (1.0 - anchor) + anchor) * amount();
    let centre = 0.5 - sample * 0.45;
    let distance = uv.y - centre;

    var alpha = edge(distance, max(thickness(), 0.002));
    if filled() {
        let between = step(min(centre, 0.5), uv.y) * step(uv.y, max(centre, 0.5));
        alpha = max(alpha, between * 0.55);
    }
    return vec4<f32>(primary(), alpha);
}
