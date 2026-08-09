// Spectrum bins mapped onto radial bars around a ring.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let distance = length(p);
    let angle = atan2(p.y, p.x) / TAU + 0.5;
    let bars = max(count(), 1.0);
    var position = angle;
    if mirrored() {
        position = abs(angle - 0.5) * 2.0;
    }
    let index = floor(position * bars);
    let level = band_at(index / bars) * reactivity();

    let inner = radius();
    let outer = inner + clamp(level, 0.0, 1.0) * size() * 8.0;
    let gap = fract(position * bars);
    let solid_bar = step(inner, distance) * step(distance, outer)
        * step(0.15, gap) * step(gap, 0.85);
    let tint = mix(primary(), secondary(), clamp(level, 0.0, 1.0));
    return vec4<f32>(tint, solid_bar);
}
