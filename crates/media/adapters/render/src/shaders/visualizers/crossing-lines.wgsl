// The original centre-crossing line fan. Each line keeps its own transform and
// interpolated colour; the three variants are the old Rotate, Scale and Shift.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let lines = max(floor(count()), 1.0);
    // Legacy time accumulates speed * (1 + raw energy) every frame. The
    // renderer carries it separately from the shared smoothed flow clock.
    let time = textureLoad(analysis, vec2<i32>(348, 1), 0).x;
    let audio = bass() * 2.0;
    let pixel = 2.0 / visualizer.resolution.y;
    // The original two-pixel stroke and 100-pixel translation are scaled from
    // its 1080-line canvas; antialiasing still follows the actual output pixels.
    let legacy_pixel = 2.0 / 1080.0;
    let half_width = legacy_pixel;
    let half_span = max(aspect(), 1.0) * 2.0;
    var colour = vec3<f32>(0.0);
    var alpha = 0.0;

    for (var i = 0; i < i32(lines); i += 1) {
        let index = f32(i);
        let pct = index / lines;
        var angle = pct * TAU;
        var scale = 1.0;
        var shift = 0.0;
        if mode() < 0.5 {
            let direction = select(-1.0, 1.0, i % 2 == 0);
            angle += time * TAU / 36.0 + audio * TAU / 8.0 * direction;
        } else if mode() < 1.5 {
            scale = 1.0 + sin(time + index) * 0.5 + audio;
        } else {
            angle = pct * TAU * 0.5;
            // The legacy translation is along the rotated line, not across it.
            shift = sin(time * 5.0 + index) * 100.0 * audio * legacy_pixel;
        }
        let along = dot(p, vec2<f32>(cos(angle), sin(angle))) - shift;
        let across = dot(p, vec2<f32>(-sin(angle), cos(angle)));
        let coverage = (1.0 - smoothstep(max(half_width - pixel * 0.5, 0.0), half_width + pixel * 0.5, abs(across)))
            * (1.0 - smoothstep(half_span * scale, half_span * scale + pixel, abs(along)));
        let line_colour = mix(primary(), secondary(), pct);
        colour = mix(colour, line_colour, coverage);
        alpha += (1.0 - alpha) * coverage;
    }
    return vec4<f32>(colour / max(alpha, 0.00001), alpha);
}
