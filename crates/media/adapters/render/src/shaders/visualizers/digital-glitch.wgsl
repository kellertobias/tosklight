// Block displacement and channel separation over a procedural signal.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let strength = clamp(amount() * (0.3 + bass() * reactivity()), 0.0, 1.0);
    let tick = floor(seconds() * max(speed(), 0.01) * 12.0);

    // Whole horizontal bands jump sideways, the way a broken digital signal tears.
    let band_index = floor(uv.y * 24.0);
    let jump = (hash21(vec2<f32>(band_index, tick)) - 0.5) * strength * 0.4
        * step(0.7, hash21(vec2<f32>(band_index * 3.0, tick)));
    let shifted = fract(uv + vec2<f32>(jump, 0.0));

    // Each channel reads the signal from a slightly different place.
    let separation = strength * 0.02;
    let signal = vec3<f32>(
        value_noise(vec2<f32>(shifted.x + separation, shifted.y) * 20.0 + tick),
        value_noise(vec2<f32>(shifted.x, shifted.y) * 20.0 + tick),
        value_noise(vec2<f32>(shifted.x - separation, shifted.y) * 20.0 + tick),
    );
    let colour = mix(primary(), signal, 0.75);
    return vec4<f32>(colour * (0.4 + strength), 1.0);
}
