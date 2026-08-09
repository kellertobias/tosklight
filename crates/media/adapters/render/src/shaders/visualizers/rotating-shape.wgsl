// A rotating solid, scaled by audio. Raymarched, so a wireframe is an edge test rather than a mesh.
fn rotate_y(point: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(point.x * c - point.z * s, point.y, point.x * s + point.z * c);
}

fn rotate_x(point: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(point.x, point.y * c - point.z * s, point.y * s + point.z * c);
}

fn shape_distance(point: vec3<f32>, extent: f32) -> f32 {
    if mode() < 0.5 {
        let corner = abs(point) - vec3<f32>(extent);
        return length(max(corner, vec3<f32>(0.0))) + min(max(corner.x, max(corner.y, corner.z)), 0.0);
    }
    if mode() < 1.5 {
        return length(point) - extent;
    }
    // A faceted ball: a sphere cut by a few planes reads as an icosphere without a mesh.
    var distance = length(point) - extent;
    let facet = abs(point.x) + abs(point.y) + abs(point.z) - extent * 1.45;
    return max(distance, -facet * 0.35);
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    // Sized to read across a whole output: a solid an operator selects should arrive big.
    let extent = (0.25 + size() * 3.0) * (1.0 + bass() * reactivity() * 0.4);
    let spin = seconds() * speed();
    var origin = vec3<f32>(p, -2.5);
    let direction = normalize(vec3<f32>(p * 0.6, 1.0));

    var travelled = 0.0;
    var hit = false;
    var last = 1.0;
    var step_index = 0;
    loop {
        if step_index >= 48 { break; }
        let world = rotate_x(rotate_y(origin + direction * travelled, spin), spin * 0.7);
        last = shape_distance(world, extent);
        if last < 0.002 { hit = true; break; }
        travelled += last;
        if travelled > 8.0 { break; }
        step_index += 1;
    }
    if !hit {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let world = rotate_x(rotate_y(origin + direction * travelled, spin), spin * 0.7);
    let facing = clamp(1.0 - travelled / 6.0, 0.0, 1.0);
    if wireframe() {
        // Near a silhouette the surface turns away from the ray, which is where an edge is.
        let grazing = 1.0 - smoothstep(0.0, 0.08, abs(shape_distance(world + vec3<f32>(0.02), extent) - last));
        return vec4<f32>(primary(), grazing);
    }
    return vec4<f32>(primary() * (0.35 + facing * 0.65), 1.0);
}
