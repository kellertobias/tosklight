// Concentric rings whose radius follows bass, expanding on a hit.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let distance = length(p);
    let rings = max(count(), 1.0);
    let push = bass() * reactivity() * 0.4 + beat() * (1.0 - decay()) * 0.3;
    var alpha = 0.0;
    var index = 0.0;
    loop {
        if index >= rings { break; }
        let ring = size() * (index + 1.0) * 2.0 + push * (index + 1.0) / rings;
        if filled() {
            alpha = max(alpha, solid(distance - ring) * (1.0 - index / rings));
        } else {
            alpha = max(alpha, edge(distance - ring, thickness() * 4.0));
        }
        index += 1.0;
    }
    return vec4<f32>(primary(), alpha);
}
