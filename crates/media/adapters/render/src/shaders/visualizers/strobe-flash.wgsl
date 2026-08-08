// A full-frame flash when energy crosses the threshold, decaying afterwards.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let trigger = max(bass(), mid()) * reactivity();
    // `beat` already decays frame to frame; `decay` decides how much of it still counts.
    let held = beat() * (1.0 - decay());
    let level = clamp(max(step(threshold(), trigger) * trigger, held), 0.0, 1.0);

    var colour = primary() * level;
    var alpha = level;
    if mirrored() {
        // Inverted: the frame is lit and the flash punches a hole in it.
        colour = primary() * (1.0 - level);
        alpha = 1.0 - level;
    }
    return vec4<f32>(colour, alpha);
}
