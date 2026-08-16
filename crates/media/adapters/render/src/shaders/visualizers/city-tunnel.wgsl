// A neon urban corridor that continuously travels toward the viewer. Repeating perspective
// frames form the tunnel, while window bands and side towers make the enclosure read as a city.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let travel = seconds() * speed() * 0.7;
    let pulse = 0.72 + energy() * 0.28;
    let density = clamp(count(), 4.0, 96.0);
    let structure = clamp(size() * 8.0, 0.12, 1.0);

    // Perspective coordinates converge at the centre of the output. The inverse distance makes
    // cross-sections accelerate outward as they approach the viewer.
    let vertical = max(abs(p.y), 0.035);
    let depth = 0.18 / vertical;
    let longitudinal = depth * density * 0.075 - travel;
    let frame_phase = abs(fract(longitudinal) - 0.5);
    let transverse_frame = 1.0 - smoothstep(0.43, 0.49, frame_phase);

    let corridor_edge = 0.28 + abs(p.y) * (1.05 + structure * 0.45);
    let side_distance = abs(abs(p.x) - corridor_edge);
    let side_frame = 1.0 - smoothstep(0.006, 0.018, side_distance);

    // Repeated vertical window columns and horizontal floors create recognisable building faces
    // on both sides instead of an abstract line tunnel.
    let wall_x = max(abs(p.x) - corridor_edge, 0.0);
    let windows_x = abs(fract(wall_x * (10.0 + density * 0.14)) - 0.5);
    let windows_y = abs(fract(longitudinal * 0.5 + p.y * 5.0) - 0.5);
    let on_wall = smoothstep(0.0, 0.08, wall_x);
    let window = on_wall
        * (1.0 - smoothstep(0.30, 0.45, windows_x))
        * (1.0 - smoothstep(0.22, 0.42, windows_y));

    // Floor and ceiling centre lines reinforce forward motion even at low building density.
    let ray_coordinate = p.x / vertical;
    let ray = 1.0 - smoothstep(0.012, 0.03, abs(fract(ray_coordinate * 0.42) - 0.5));
    let floor_ceiling = ray * smoothstep(0.12, 0.46, abs(p.y));
    let cross_section = transverse_frame * smoothstep(0.10, 0.48, abs(p.y));

    let city_light = clamp(
        side_frame + cross_section * 0.72 + floor_ceiling * 0.34 + window * 0.86,
        0.0,
        1.0,
    );
    let distance_fade = smoothstep(0.015, 0.22, abs(p.y));
    let colour = mix(secondary(), primary(), clamp(window + side_frame, 0.0, 1.0));
    let brightness = city_light * mix(0.35, 1.0, distance_fade) * pulse * amount();

    // A deep opaque background makes this a usable full-frame environment, not isolated lines.
    let background = mix(vec3<f32>(0.002, 0.004, 0.012), secondary() * 0.055, abs(p.y));
    return vec4<f32>(background + colour * brightness, 1.0);
}
