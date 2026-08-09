// A field of crossing lines that audio either rotates, scales, or shifts.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let lines = max(floor(count()), 1.0);
    let drive = bass() * reactivity();
    var turned = p;
    var scale = lines;
    var offset = seconds() * speed() * 0.2;

    if mode() < 0.5 {
        let spin = seconds() * speed() * 0.2 + drive;
        turned = vec2<f32>(
            p.x * cos(spin) - p.y * sin(spin),
            p.x * sin(spin) + p.y * cos(spin),
        );
    } else if mode() < 1.5 {
        scale = lines * (1.0 + drive);
    } else {
        offset += drive;
    }

    let first = abs(fract(turned.x * scale + offset) - 0.5);
    let second = abs(fract(turned.y * scale - offset) - 0.5);
    let one = 1.0 - smoothstep(0.0, 0.06, first);
    let two = 1.0 - smoothstep(0.0, 0.06, second);
    let colour = primary() * one + secondary() * two;
    return vec4<f32>(colour, clamp(one + two, 0.0, 1.0));
}
