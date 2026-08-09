// The whole frame cycling through hues, its brightness driven by bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let hue = fract(seconds() * speed() * 0.1);
    let saturated = mix(vec3<f32>(1.0), hue_to_rgb(hue), clamp(amount(), 0.0, 1.0));
    let brightness = clamp(0.25 + bass() * reactivity(), 0.0, 1.0);
    return vec4<f32>(saturated * brightness, 1.0);
}
