// Uniform, unshadowed Draft shaft integral. Nonuniform optics/atlas shadows retain the march.
fn draft_sphere(origin: vec3<f32>, direction: vec3<f32>, centre: vec3<f32>, radius: f32) -> vec2<f32> {
    let offset = origin - centre;
    let along = dot(offset, direction);
    let across = cross(offset, direction);
    let squared = radius * radius - dot(across, across);
    if (squared <= 0.0) { return vec2<f32>(1.0, -1.0); }
    let half_width = sqrt(squared);
    return vec2<f32>(-along - half_width, -along + half_width);
}
fn draft_inverse(a: f32, b: f32, along: f32, height: f32) -> f32 {
    if (b <= a) { return 0.0; }
    let x = a + along;
    let y = b + along;
    if (height == 0.0) { return 36.0 * (b - a) / (x * y); }
    return 36.0 * atan2(height * (b - a), height * height + x * y) / height;
}
fn draft_square_integral(a: f32, b: f32, origin: vec3<f32>, direction: vec3<f32>, apex: vec3<f32>) -> f32 {
    if (b <= a) { return 0.0; }
    let offset = origin - apex;
    let along = dot(offset, direction);
    let height = length(cross(offset, direction));
    let sphere = draft_sphere(origin, direction, apex, 0.05);
    let left = max(a, sphere.x);
    let right = min(b, sphere.y);
    if (right <= left) { return draft_inverse(a, b, along, height); }
    return draft_inverse(a, left, along, height) + (right - left) * (36.0 / 0.0025)
        + draft_inverse(right, b, along, height);
}
// Four-point Gauss integration only across the smooth final20% lens-centred range taper.
// The constant-range interior uses the exact inverse-square primitive. This taper is a bounded
// numerical approximation, not an assertion that two differently centred distances are rational.
fn draft_taper(a: f32, b: f32, origin: vec3<f32>, direction: vec3<f32>, apex: vec3<f32>, light: Light) -> f32 {
    if (b <= a) { return 0.0; }
    let nodes = array<f32, 4>(-0.8611363116, -0.3399810436, 0.3399810436, 0.8611363116);
    let weights = array<f32, 4>(0.3478548451, 0.6521451549, 0.6521451549, 0.3478548451);
    let centre = (a + b) * 0.5;
    let half_width = (b - a) * 0.5;
    var integral = 0.0;
    for (var index = 0u; index < 4u; index++) {
        let point = origin + direction * (centre + half_width * nodes[index]);
        let distance = max(length(point - apex), 0.05);
        integral += weights[index] * (36.0 / (distance * distance))
            * range_envelope(length(point - light.position_range.xyz), light.position_range.w);
    }
    return integral * half_width;
}
fn draft_visible_integral(a: f32, b: f32, origin: vec3<f32>, direction: vec3<f32>, apex: vec3<f32>, light: Light) -> f32 {
    let range = draft_sphere(origin, direction, light.position_range.xyz, light.position_range.w);
    let first = max(a, range.x);
    let last = min(b, range.y);
    if (last <= first) { return 0.0; }
    let flat = draft_sphere(origin, direction, light.position_range.xyz, light.position_range.w * 0.8);
    let flat_start = max(first, flat.x);
    let flat_end = min(last, flat.y);
    if (flat_end <= flat_start) { return draft_taper(first, last, origin, direction, apex, light); }
    return draft_taper(first, flat_start, origin, direction, apex, light)
        + draft_square_integral(flat_start, flat_end, origin, direction, apex)
        + draft_taper(flat_end, last, origin, direction, apex, light);
}
fn draft_curtain_interval(blocker: CurtainBlocker, origin: vec3<f32>, direction: vec3<f32>, near: f32, far: f32) -> vec2<f32> {
    var entry = near;
    var exit = far;
    for (var face = 0u; face < 5u; face++) {
        let side = (face + 4u) % 5u;
        let plane = blocker.planes[side];
        let at_origin = dot(plane, vec4<f32>(origin, 1.0));
        let slope = dot(plane.xyz, direction);
        let a = at_origin + slope * entry;
        let b = at_origin + slope * exit;
        if ((a < 0.0 && b < 0.0) || (side == 4u && a <= 0.0 && b <= 0.0)) { return vec2<f32>(far, near); }
        if (a < 0.0 || b < 0.0) {
            let crossing = -at_origin / slope;
            if (slope > 0.0) { entry = max(entry, crossing); } else { exit = min(exit, crossing); }
        }
        if (entry > exit) { return vec2<f32>(far, near); }
    }
    return vec2<f32>(entry, exit);
}
// Returns negative only when the established bounded march is required; never drops optics,
// atlas occlusion, or curtains that exceed the local interval capacity.
fn draft_analytic(light: Light, origin: vec3<f32>, direction: vec3<f32>, apex: vec3<f32>, near: f32, far: f32) -> f32 {
    if (light.shadow.x >= 0.0 || globals.params5.x > 0.001 || beam_falloff_enabled() || gobos_enabled()
        || light.position_range.w <= 0.1) { return -1.0; }
    for (var wheel = 0u; wheel < min(u32(light.gate.z), 8u); wheel++) {
        if (light.prism_wheels[wheel].x >= 2.0) { return -1.0; }
    }
    let reach = draft_sphere(origin, direction, apex, light.position_range.w + length(light.position_range.xyz - apex));
    let first = max(near, reach.x);
    let last = min(far, reach.y);
    if (last <= first) { return 0.0; }
    var intervals: array<vec2<f32>, 8>;
    var used = 0u;
    if (light.gate.w >= 0.0 && globals.params4.w >= 1.0) {
        let header = u32(light.gate.w);
        let count = u32(curtain_blockers[header].planes[4].z);
        if (count > 8u) { return -1.0; }
        for (var index = 0u; index < count; index++) {
            let interval = draft_curtain_interval(curtain_blockers[header + 1u + index], origin, direction, first, last);
            if (interval.y <= interval.x) { continue; }
            var insert = used;
            loop {
                if (insert == 0u) { break; }
                if (intervals[insert - 1u].x <= interval.x) { break; }
                intervals[insert] = intervals[insert - 1u]; insert--;
            }
            intervals[insert] = interval; used++;
        }
    }
    var cursor = first;
    var integrated = 0.0;
    for (var index = 0u; index < used; index++) {
        let interval = intervals[index];
        if (interval.x > cursor) { integrated += draft_visible_integral(cursor, interval.x, origin, direction, apex, light); }
        cursor = max(cursor, interval.y);
    }
    return integrated + draft_visible_integral(cursor, last, origin, direction, apex, light);
}
