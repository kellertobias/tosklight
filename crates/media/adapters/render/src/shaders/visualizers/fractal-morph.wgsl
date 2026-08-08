// A Julia set whose constant is pushed around by bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let drive = bass() * reactivity();
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
        return vec4<f32>(primary() * 0.15, 1.0);
    }
    let escape = taken / limit;
    return vec4<f32>(hue_to_rgb(escape + angle * 0.05) * (0.4 + escape), 1.0);
}
