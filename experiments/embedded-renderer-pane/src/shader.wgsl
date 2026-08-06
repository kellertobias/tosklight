// A stand-in scene: unmistakably rendered, obviously animated, and driven by the camera the web
// side forwards. Nothing here is proposed for production — the point is that these pixels come
// from the GPU and land inside the pane rectangle and nowhere else.

struct Uniforms {
    // x, y, width, height of the pane in physical pixels.
    pane: vec4<f32>,
    // yaw, pitch, distance, seconds.
    camera: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One oversized triangle. The scissor decides what of it survives.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

/// Distance from a point to a line segment, for the beams and the floor grid.
fn segment_distance(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let span = end - start;
    let t = clamp(dot(point - start, span) / max(dot(span, span), 1e-6), 0.0, 1.0);
    return length(point - (start + span * t));
}

@fragment
fn fragment_main(@builtin(position) fragment: vec4<f32>) -> @location(0) vec4<f32> {
    // Pane-local coordinates, so the scene is composed against the pane rather than the window.
    let local = fragment.xy - uniforms.pane.xy;
    let size = uniforms.pane.zw;
    let uv = local / size;
    let aspect = size.x / max(size.y, 1.0);
    // Centred, aspect-corrected, and scaled by the camera distance the operator set.
    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5) * uniforms.camera.z;

    let yaw = uniforms.camera.x;
    let pitch = uniforms.camera.y;
    let time = uniforms.camera.w;

    // A dark stage that is clearly not a web gradient: a vignette, a horizon that the pitch
    // moves, and a floor grid that the yaw slides.
    let horizon = -0.12 + pitch * 0.35;
    var colour = vec3<f32>(0.03, 0.035, 0.05);
    if (p.y < horizon) {
        let depth = clamp((horizon - p.y) * 6.0, 0.0, 1.0);
        let grid_x = abs(fract((p.x * 6.0 / max(depth, 0.05)) + yaw * 2.0) - 0.5);
        let grid_z = abs(fract(depth * 8.0) - 0.5);
        let line = smoothstep(0.02, 0.0, min(grid_x, grid_z) * depth);
        colour = mix(vec3<f32>(0.05, 0.05, 0.07), vec3<f32>(0.16, 0.18, 0.24), line)
            * (1.0 - depth * 0.6);
    }

    // Three beams sweeping in different phases, so a still screenshot still reads as motion and a
    // dropped frame is obvious.
    for (var i = 0; i < 3; i = i + 1) {
        let index = f32(i);
        let origin = vec2<f32>(-0.32 + index * 0.32, 0.34);
        let sweep = sin(time * (0.7 + index * 0.23) + index * 2.1 + yaw) * 0.42;
        let aim = vec2<f32>(origin.x + sweep, horizon - 0.05);
        let distance = segment_distance(p, origin, aim);
        let core = smoothstep(0.02, 0.0, distance);
        let haze = smoothstep(0.16, 0.0, distance) * 0.35;
        let hue = vec3<f32>(
            0.5 + 0.5 * sin(time * 0.5 + index * 2.0),
            0.5 + 0.5 * sin(time * 0.5 + index * 2.0 + 2.1),
            0.5 + 0.5 * sin(time * 0.5 + index * 2.0 + 4.2),
        );
        colour = colour + hue * (core + haze);
        // The lantern body, so there is something solid to occlude and to aim at.
        colour = mix(colour, hue * 0.8, smoothstep(0.022, 0.0, length(p - origin)));
    }

    // A soft vignette towards the pane edges, which also makes the clip boundary easy to see.
    let vignette = 1.0 - smoothstep(0.55, 1.15, length(vec2<f32>(p.x / max(aspect, 0.001), p.y)) * 1.6);
    colour = colour * clamp(vignette, 0.25, 1.0);

    return vec4<f32>(colour, 1.0);
}
