// The old finite spokes: uniform width, square ends and bass-driven length.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let rays = max(floor(count()), 1.0);
    // Speed 1 is the original designed rotation rate of 15 degrees per second.
    let spin = clock() * TAU / 24.0;
    let angle = atan2(p.y, p.x) - spin;
    let wedge = TAU / rays;
    let nearest = (fract(angle / wedge + 0.5) - 0.5) * wedge;
    let distance = length(p);
    let along = distance * cos(nearest);
    let across = abs(distance * sin(nearest));
    // Size and thickness are output fractions in the shared parameter contract.
    let reach = size() * 2.0 * (1.0 + bass());
    let half_width = thickness();
    let pixel = 2.0 / visualizer.resolution.y;
    let line = 1.0 - smoothstep(max(half_width - pixel * 0.5, 0.0), half_width + pixel * 0.5, across);
    let ends = (1.0 - smoothstep(reach - pixel * 0.5, reach + pixel * 0.5, along))
        * smoothstep(-pixel * 0.5, pixel * 0.5, along);
    return vec4<f32>(primary(), line * ends);
}
