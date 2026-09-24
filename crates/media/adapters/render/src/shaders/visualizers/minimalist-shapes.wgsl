// Shapes in a loose grid, each rotating and fading out. Following the audio, every cell keeps its
// own cycle and the bass sizes the shapes; on beat, every landed beat pops shapes into a fresh
// handful of cells, which then fade.

fn shape_distance(local: vec2<f32>, extent: f32, spin: f32) -> f32 {
    let turned = vec2<f32>(
        local.x * cos(spin) - local.y * sin(spin),
        local.x * sin(spin) + local.y * cos(spin),
    );
    if mode() < 0.5 {
        // Boxes.
        let corner = abs(turned) - vec2<f32>(extent);
        return length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0);
    }
    return length(turned) - extent;
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let cells = max(floor(sqrt(count())), 1.0);
    let cell = floor(uv * cells);
    let local = fract(uv * cells) - 0.5;
    let seed = hash21(cell);
    let period = max(2.0 / max(speed(), 0.05), 0.2);
    let spin = seconds() * speed() * (seed - 0.5) * 4.0;

    if on_beat() {
        // A beat lights about a third of the cells, a different third each time. The shape pops
        // big and shrinks as it fades over one period.
        var coverage = 0.0;
        var landed = 0;
        loop {
            if landed >= BEAT_HISTORY { break; }
            let age = beat_age(landed) / period;
            if age >= 1.0 { break; }
            let beat_seed = beat_count() - f32(landed);
            if hash21(cell + vec2<f32>(beat_seed * 0.731, beat_seed * 1.117)) < 0.34 {
                let fade = 1.0 - age;
                let pop = 1.0 + (1.0 - smoothstep(0.0, 0.25, age)) * 0.35 * reactivity();
                let extent = (0.08 + size() * 2.0) * pop * fade;
                coverage = max(coverage, solid(shape_distance(local, extent, spin)) * fade);
            }
            landed += 1;
        }
        return vec4<f32>(primary(), coverage);
    }

    // Each cell keeps its own phase, so they do not all appear and vanish together.
    let age = fract(seconds() / period + seed);
    let fade = 1.0 - age;
    let extent = (0.08 + size() * 2.0) * (0.5 + bass() * reactivity()) * fade;
    return vec4<f32>(primary(), solid(shape_distance(local, extent, spin)) * fade);
}
