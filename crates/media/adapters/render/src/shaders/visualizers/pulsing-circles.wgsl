// The original's concentric circles: one audio-driven radius, constant spacing and
// progressively fainter outer rings. The runtime keeps the original decayed radius.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let distance = length(p);
    let rings = max(floor(count()), 1.0);
    let pulse = textureLoad(analysis, vec2<i32>(RHYTHM + 10 + BEAT_HISTORY, 1), 0).x;
    var alpha = 0.0;
    var index = 0.0;
    loop {
        if index >= rings { break; }
        let ring = pulse + index * size() * 2.0;
        if ring > 0.0 {
            let fade = 1.0 - index / rings;
            var coverage = edge(distance - ring, 5.0 * 2.0 / 1080.0);
            if filled() { coverage = solid(distance - ring); }
            // Circles were drawn from the centre outward with ordinary alpha blending.
            alpha = 1.0 - (1.0 - alpha) * (1.0 - coverage * fade);
        }
        index += 1.0;
    }
    return vec4<f32>(primary(), alpha);
}
