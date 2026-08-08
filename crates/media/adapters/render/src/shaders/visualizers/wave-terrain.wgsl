// A scrolling noise terrain seen edge on, its depth driven by bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let scroll = seconds() * speed() * 0.25;
    let sampled = vec2<f32>(uv.x * zoom() * 6.0, uv.y * zoom() * 6.0 - scroll);
    var height = value_noise(sampled);
    height += value_noise(sampled * 2.0) * 0.5;
    height = height / 1.5;
    height += bass() * reactivity() * size() * 4.0;

    // Depth becomes contour lines, which read as a terrain without a third dimension.
    let contours = fract(height * 12.0);
    var alpha = 0.0;
    if wireframe() {
        alpha = 1.0 - smoothstep(0.0, 0.12, abs(contours - 0.5));
    } else {
        alpha = clamp(height, 0.0, 1.0);
    }
    let tint = mix(primary() * 0.35, primary(), clamp(height, 0.0, 1.0));
    return vec4<f32>(tint, alpha);
}
