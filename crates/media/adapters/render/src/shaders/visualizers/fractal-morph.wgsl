// A Julia set whose constant is pushed around by the smoothed bass, shaded from the start colour
// where points escape at once to the end colour along the set's edge and inside it.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    // Smoothed, so the set morphs with the bass instead of shuddering on every kick. Smoothing at
    // zero follows the bass exactly.
    let drive = smooth_bass() * reactivity();
    let angle = seconds() * 0.1 + drive;
    let constant = vec2<f32>(0.7885 * cos(angle), 0.7885 * sin(angle));

    var point = p / max(zoom(), 0.05);
    let limit = max(iterations(), 1.0);
    var taken = 0.0;
    loop {
        if taken >= limit { break; }
        if dot(point, point) > 4.0 { break; }
        point = vec2<f32>(point.x * point.x - point.y * point.y, 2.0 * point.x * point.y) + constant;
        taken += 1.0;
    }

    if taken >= limit {
        // Inside the set: solid, so the shape reads against whatever is behind the layer.
        return vec4<f32>(secondary() * 0.15, 1.0);
    }
    // Quick escapes take the start colour; slow ones, hugging the set, take the end colour.
    let escape = taken / limit;
    let blend = pow(escape, 0.6);
    return vec4<f32>(mix(primary(), secondary(), blend) * (0.3 + escape), 1.0);
}
