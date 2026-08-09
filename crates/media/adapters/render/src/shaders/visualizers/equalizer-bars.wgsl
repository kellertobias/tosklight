// Spectrum bars with a low-to-high gradient, optional mirroring, and an additive glow.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let bars = max(count(), 1.0);
    var across = uv.x;
    if mirrored() {
        // Two halves meeting in the middle, low frequencies at the centre.
        across = abs(uv.x - 0.5) * 2.0;
    }
    let index = floor(across * bars);
    let centre = (index + 0.5) / bars;
    let level = pow(band_at(index / bars), mix(1.0, 0.6, smoothing())) * reactivity();
    let height = clamp(level, 0.0, 1.0);

    let half_width = (size() * 0.5) / bars * 2.0;
    let inside_bar = abs(across - centre) < half_width;
    let inside_height = (1.0 - uv.y) < height;

    let tint = mix(primary(), secondary(), index / max(bars - 1.0, 1.0));
    var colour = vec3<f32>(0.0);
    var alpha = 0.0;
    if inside_bar && inside_height {
        colour = tint;
        alpha = 1.0;
    }
    // The glow is additive and unclipped by the bar, so tall bars bleed into their neighbours.
    let glow = amount() * height * exp(-abs(across - centre) * bars * 1.5)
        * exp(-max((1.0 - uv.y) - height, 0.0) * 12.0);
    colour += tint * glow;
    alpha = clamp(alpha + glow, 0.0, 1.0);
    return vec4<f32>(colour, alpha);
}
