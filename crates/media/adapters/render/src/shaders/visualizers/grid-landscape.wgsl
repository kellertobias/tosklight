fn line_segment(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>, width: f32) -> f32 {
    let span = end - start;
    let along = clamp(dot(point - start, span) / max(dot(span, span), 0.0001), 0.0, 1.0);
    return 1.0 - smoothstep(width, width + 0.006, length(point - (start + span * along)));
}

fn roadside(point: vec2<f32>, side: f32, scenery: f32, travel: f32) -> f32 {
    if scenery < 0.5 { return 0.0; }
    var light = 0.0;
    var index = 0.0;
    loop {
        if index >= 7.0 { break; }
        let depth = fract(index / 7.0 + travel * 0.09);
        let perspective = depth * depth;
        let base = vec2<f32>(side * (0.16 + perspective * 0.62), -0.04 + perspective * 1.02);
        let scale = 0.025 + perspective * 0.16;
        if scenery < 1.5 {
            let top = base - vec2<f32>(0.0, scale * 1.8);
            light += line_segment(point, base, top, 0.004 + scale * 0.025);
            light += solid(length(point - top) - scale * (0.15 + beat() * 0.06));
        } else {
            let crown = base - vec2<f32>(side * scale * 0.12, scale * 1.45);
            light += line_segment(point, base, crown, 0.005 + scale * 0.035);
            light += line_segment(point, crown, crown + vec2<f32>(-scale, -scale * 0.35), 0.006);
            light += line_segment(point, crown, crown + vec2<f32>(scale, -scale * 0.35), 0.006);
            light += line_segment(point, crown, crown + vec2<f32>(-scale * 0.75, scale * 0.25), 0.006);
            light += line_segment(point, crown, crown + vec2<f32>(scale * 0.75, scale * 0.25), 0.006);
        }
        index += 1.0;
    }
    return clamp(light, 0.0, 1.0);
}

// A transparent synthwave landscape: road and mountain grids race toward the viewer, while the
// distant sun fades softly into the background and each roadside has its own scenery type.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let travel = seconds() * speed();
    let horizon = -0.10;
    let floor_y = p.y - horizon;
    let road_edge = 0.12 + max(floor_y, 0.0) * 0.62;

    var grid = 0.0;
    if floor_y > 0.0 {
        let depth = 0.16 / max(floor_y, 0.025);
        let cross = abs(fract(depth * (5.0 + count() * 0.12) - travel * 0.75) - 0.5);
        let cross_line = 1.0 - smoothstep(0.44, 0.49, cross);
        let rays = 1.0 - smoothstep(0.012, 0.025, abs(fract((p.x / floor_y) * 0.42) - 0.5));
        grid = max(cross_line * 0.72, rays * 0.52);
    }

    let mountain_side = smoothstep(road_edge, road_edge + 0.05, abs(p.x));
    let mountain_height = 0.12
        + value_noise(vec2<f32>(abs(p.x) * 3.5, floor(p.x * 9.0))) * (0.22 + size() * 0.65);
    let mountain_body = mountain_side * (1.0 - smoothstep(mountain_height, mountain_height + 0.035, -p.y));
    let mountain_vertical = 1.0 - smoothstep(0.015, 0.035, abs(fract(abs(p.x) * 10.0) - 0.5));
    let mountain_horizontal = 1.0 - smoothstep(0.025, 0.055, abs(fract((p.y + 0.5) * 10.0) - 0.5));
    let mountain_grid = mountain_body * max(mountain_vertical, mountain_horizontal);

    let sun_radius = 0.10 + radius() * 0.30;
    let sun_distance = length((p - vec2<f32>(0.0, horizon - 0.10)) / vec2<f32>(1.0, 1.15));
    let sun = 1.0 - smoothstep(sun_radius * 0.50, sun_radius, sun_distance);
    let sun_bands = smoothstep(0.08, 0.16, abs(fract((p.y - horizon) * 18.0) - 0.5));
    let soft_sun = sun * sun_bands * amount();

    let left = roadside(p, -1.0, mode(), travel);
    let right = roadside(p, 1.0, iterations(), travel);
    let road = grid * (1.0 - smoothstep(road_edge, road_edge + 0.08, abs(p.x)));
    let landscape = max(road, mountain_grid);
    let light = clamp(max(landscape, max(left, right)) + soft_sun, 0.0, 1.0);
    let colour = mix(secondary(), primary(), clamp(landscape + left + right, 0.0, 1.0));
    let audio = 0.72 + energy() * 0.28;
    return vec4<f32>(colour * light * audio, light);
}
