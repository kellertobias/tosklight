// Rotating radial lines whose length grows with bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let rays = max(floor(count()), 1.0);
    let spin = seconds() * speed() * 0.5;
    let angle = atan2(p.y, p.x) + spin;
    let wedge = TAU / rays;
    let across = abs(fract(angle / wedge + 0.5) - 0.5) * wedge;
    let distance = length(p);

    let reach = size() * (1.0 + bass() * reactivity() * 2.0) * 8.0;
    let along = 1.0 - smoothstep(reach * 0.7, reach, distance);
    let line = 1.0 - smoothstep(thickness(), thickness() + 0.01, across * distance);
    let alpha = line * along;
    return vec4<f32>(primary(), alpha);
}
