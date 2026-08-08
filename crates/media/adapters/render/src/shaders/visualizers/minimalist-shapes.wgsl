// Shapes spawned on beats in a loose grid, each rotating and fading out.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let cells = max(floor(sqrt(count())), 1.0);
    let cell = floor(uv * cells);
    let local = fract(uv * cells) - 0.5;
    let seed = hash21(cell);

    // Each cell keeps its own phase, so they do not all appear and vanish together.
    let period = max(2.0 / max(speed(), 0.05), 0.2);
    let age = fract(seconds() / period + seed);
    let fade = 1.0 - age;
    let extent = (0.08 + size() * 2.0) * (0.5 + bass() * reactivity()) * fade;

    let spin = seconds() * speed() * (seed - 0.5) * 4.0;
    let turned = vec2<f32>(
        local.x * cos(spin) - local.y * sin(spin),
        local.x * sin(spin) + local.y * cos(spin),
    );

    var distance = 0.0;
    if mode() < 0.5 {
        // Boxes.
        let corner = abs(turned) - vec2<f32>(extent);
        distance = length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0);
    } else {
        distance = length(turned) - extent;
    }
    return vec4<f32>(primary(), solid(distance) * fade);
}
