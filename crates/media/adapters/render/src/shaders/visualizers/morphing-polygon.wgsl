// Straight segments connect individually deformed vertices, as in the original.
// Sampling the low quarter of the spectrum makes each corner respond independently.
fn polygon_vertex(index: f32, sides: f32) -> vec2<f32> {
    let angle = index / sides * TAU;
    let bin = i32(floor(index / sides * f32(BANDS / 4)));
    let kick_landed = textureLoad(analysis, vec2<i32>(RHYTHM + 10 + BEAT_HISTORY + 253, 1), 0).x;
    let deformation = band(bin) + select(0.0, 0.2, kick_landed > 0.5);
    let extent = radius() + deformation * amount() * (400.0 * 2.0 / 1080.0);
    return vec2<f32>(cos(angle), sin(angle)) * extent;
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let sides = max(floor(count()), 3.0);
    var nearest = 1.0e6;
    var inside = false;
    var index = 0.0;
    loop {
        if index >= sides { break; }
        let a = polygon_vertex(index, sides);
        let b = polygon_vertex((index + 1.0) % sides, sides);
        let segment = b - a;
        let along = clamp(dot(p - a, segment) / max(dot(segment, segment), 0.000001), 0.0, 1.0);
        nearest = min(nearest, length(p - (a + segment * along)));
        // Even/odd crossings also handle a polygon made concave by uneven spectrum levels.
        if (a.y > p.y) != (b.y > p.y) {
            let crossing = a.x + (p.y - a.y) * segment.x / segment.y;
            if p.x < crossing { inside = !inside; }
        }
        index += 1.0;
    }
    var alpha = edge(nearest, thickness());
    if filled() { alpha = solid(select(nearest, -nearest, inside)); }
    return vec4<f32>(primary(), alpha);
}
