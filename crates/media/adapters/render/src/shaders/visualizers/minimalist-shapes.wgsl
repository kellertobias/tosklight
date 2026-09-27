// Randomly scattered boxes/circles, each with its own size, rotation and rotation speed.
// A landed beat sends one shape; its birth seed stays stable while it rotates and fades.
fn minimalist_shape(p: vec2<f32>, seed: f32, age: f32, fade: f32) -> f32 {
    let centre = (hash22(vec2<f32>(seed, 4.71)) - 0.5) * vec2<f32>(aspect(), 1.0) * 2.0;
    let initial = hash11(seed + 8.23) * TAU;
    let spin = initial + age * speed() * (hash11(seed + 17.43) - 0.5) * 10.47;
    let local = p - centre;
    let turned = vec2<f32>(
        local.x * cos(spin) + local.y * sin(spin),
        -local.x * sin(spin) + local.y * cos(spin),
    );
    // Size remains fixed during the fade; bass expands it, just like the old shapes.
    let extent = size() * (0.5 + hash11(seed + 29.17)) * (1.0 + bass() * 2.0);
    var distance = length(turned) - extent;
    if mode() < 0.5 {
        let corner = abs(turned) - vec2<f32>(extent);
        distance = length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0);
    }
    return solid(distance) * fade;
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let duration = 2.125 / max(speed(), 0.01);
    var alpha = 0.0;
    var index = 0;
    if on_beat() {
        // The runtime reproduces the old spawn-capacity gate and keeps every live shape.
        // Unlike a beat-history reconstruction this cannot replace a still-living shape.
        let base = RHYTHM + 10 + BEAT_HISTORY;
        let live_count = i32(textureLoad(analysis, vec2<i32>(base + 1, 1), 0).x);
        loop {
            if index >= live_count { break; }
            let shape_base = base + 2 + index * 5;
            let centre_uv = vec2<f32>(
                textureLoad(analysis, vec2<i32>(shape_base, 1), 0).x,
                textureLoad(analysis, vec2<i32>(shape_base + 1, 1), 0).x,
            );
            let centre = (centre_uv - 0.5) * vec2<f32>(aspect(), 1.0) * 2.0;
            let extent = textureLoad(analysis, vec2<i32>(shape_base + 2, 1), 0).x * (1.0 + bass() * 2.0);
            let spin = textureLoad(analysis, vec2<i32>(shape_base + 3, 1), 0).x;
            let fade = textureLoad(analysis, vec2<i32>(shape_base + 4, 1), 0).x;
            let local = p - centre;
            let turned = vec2<f32>(
                local.x * cos(spin) + local.y * sin(spin),
                -local.x * sin(spin) + local.y * cos(spin),
            );
            var distance = length(turned) - extent;
            if mode() < 0.5 {
                let corner = abs(turned) - vec2<f32>(extent);
                distance = length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0);
            }
            let coverage = solid(distance) * fade;
            alpha = 1.0 - (1.0 - alpha) * (1.0 - coverage);
            index += 1;
        }
    } else {
        // The existing continuous option keeps a full population in staggered cycles.
        loop {
            if f32(index) >= count() { break; }
            let slot = f32(index);
            let cycle = clock() / 2.125 + slot / count();
            let age = fract(cycle) * duration;
            let seed = slot + floor(cycle) * count();
            let coverage = minimalist_shape(p, seed, age, 1.0 - fract(cycle));
            alpha = 1.0 - (1.0 - alpha) * (1.0 - coverage);
            index += 1;
        }
    }
    return vec4<f32>(primary(), alpha);
}
