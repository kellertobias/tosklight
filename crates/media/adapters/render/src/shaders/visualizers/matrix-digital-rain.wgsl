// Falling procedural code columns. Each cell contains a stable pseudo-glyph; column speeds and
// trail lengths vary independently so the result reads as digital rain rather than a regular grid.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let columns = clamp(round(count()), 8.0, 160.0);
    let rows = columns / max(aspect(), 0.25) * 0.62;
    let cell = vec2<f32>(uv.x * columns, uv.y * rows);
    let cell_id = floor(cell);
    let local = fract(cell);
    let column_seed = hash11(cell_id.x + 17.0);
    let rate = max(speed(), 0.02) * mix(0.16, 0.52, column_seed) * (0.85 + energy() * 0.45);
    let head = fract(seconds() * rate + hash11(cell_id.x * 3.71));
    let trail_distance = fract(head - uv.y);
    let trail_length = mix(0.18, 0.62, hash11(cell_id.x * 8.13));
    let trail = 1.0 - smoothstep(0.0, trail_length, trail_distance);
    let head_glow = 1.0 - smoothstep(0.0, 0.035, abs(trail_distance));

    // Refresh a column's character sequence as the head advances. The strokes form small blocky
    // code glyphs, including vertical stems and independently selected cross-bars.
    let tick = floor(seconds() * rate * rows);
    let glyph_seed = hash21(vec2<f32>(cell_id.x * 11.7, cell_id.y + tick));
    let stem = 1.0 - smoothstep(0.10, 0.18, abs(local.x - mix(0.32, 0.68, glyph_seed)));
    let bar_a = (1.0 - smoothstep(0.08, 0.16, abs(local.y - 0.26)))
        * step(0.28, hash11(glyph_seed * 31.0));
    let bar_b = (1.0 - smoothstep(0.08, 0.16, abs(local.y - 0.72)))
        * step(0.45, hash11(glyph_seed * 67.0));
    let side = (1.0 - smoothstep(0.08, 0.16, abs(local.x - 0.76)))
        * step(0.62, hash11(glyph_seed * 97.0));
    let glyph = clamp(max(stem, max(bar_a, max(bar_b, side))), 0.0, 1.0)
        * smoothstep(0.04, 0.12, local.y)
        * smoothstep(0.04, 0.12, 1.0 - local.y);

    let audio_light = 0.65 + energy() * 0.55 + beat() * 0.35;
    let body = mix(secondary(), primary(), trail) * trail;
    let colour = body + primary() * head_glow * 1.25;
    let brightness = mix(0.18, 1.5, clamp(amount(), 0.0, 1.0)) * audio_light;
    return vec4<f32>(colour * glyph * brightness, 1.0);
}
