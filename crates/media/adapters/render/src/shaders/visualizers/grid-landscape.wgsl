/// How far a point is from a segment, and how far along it the nearest point lies, `0..1`.
fn segment_distance(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> vec2<f32> {
    let span = end - start;
    let along = clamp(dot(point - start, span) / max(dot(span, span), 0.0000001), 0.0, 1.0);
    return vec2<f32>(length(point - (start + span * along)), along);
}

/// A point on a quadratic curve.
fn curve_at(start: vec2<f32>, bend: vec2<f32>, end: vec2<f32>, t: f32) -> vec2<f32> {
    return mix(mix(start, bend, t), mix(bend, end, t), t);
}

/// A tapered stroke along a quadratic curve, `width_start` wide at its start and `width_end` at
/// its end. Four straight pieces are plenty at the size a roadside object is drawn.
fn tapered_curve(
    point: vec2<f32>,
    start: vec2<f32>,
    bend: vec2<f32>,
    end: vec2<f32>,
    width_start: f32,
    width_end: f32,
) -> f32 {
    var cover = 0.0;
    var piece = 0.0;
    loop {
        if piece >= 4.0 { break; }
        let head = curve_at(start, bend, end, piece / 4.0);
        let tail = curve_at(start, bend, end, (piece + 1.0) / 4.0);
        let hit = segment_distance(point, head, tail);
        let t = (piece + hit.y) / 4.0;
        cover = max(cover, solid(hit.x - mix(width_start, width_end, t)));
        piece += 1.0;
    }
    return cover;
}

/// One palm frond: a leaf arching out from the crown and drooping, widest a third of the way out
/// and serrated along its edge so it reads as a row of leaflets rather than a paddle.
fn frond(point: vec2<f32>, crown: vec2<f32>, heading: f32, reach: f32, droop: f32) -> f32 {
    let outward = vec2<f32>(cos(heading), sin(heading));
    let tip = crown + outward * reach + vec2<f32>(0.0, droop * reach);
    let bend = crown + outward * reach * 0.55 - vec2<f32>(0.0, reach * 0.18);
    var cover = 0.0;
    var piece = 0.0;
    loop {
        if piece >= 5.0 { break; }
        let head = curve_at(crown, bend, tip, piece / 5.0);
        let tail = curve_at(crown, bend, tip, (piece + 1.0) / 5.0);
        let hit = segment_distance(point, head, tail);
        let t = (piece + hit.y) / 5.0;
        let leaflets = 0.55 + 0.45 * abs(fract(t * 9.0) - 0.5) * 2.0;
        let width = reach * (0.012 + 0.085 * sin(3.14159 * pow(t, 0.7)) * leaflets);
        cover = max(cover, solid(hit.x - width));
        piece += 1.0;
    }
    return cover;
}

/// A street lamp standing at `base`, its arm reaching over the road. Returns its lens in `x`, its
/// body in `y`, the cone its light throws toward the road in `z`, and the glow around the lens in
/// `w`, so the head can blaze white while the pole stays the landscape's colour.
fn street_lamp(point: vec2<f32>, base: vec2<f32>, side: f32, scale: f32) -> vec4<f32> {
    let top = base - vec2<f32>(0.0, scale * 2.6);
    // A tapered pole on a plinth, with a collar where the arm leaves it.
    var body = tapered_curve(point, base, base - vec2<f32>(0.0, scale * 1.3), top,
        scale * 0.06, scale * 0.032);
    let plinth = abs(point - (base - vec2<f32>(0.0, scale * 0.1))) - vec2<f32>(scale * 0.1, scale * 0.1);
    body = max(body, solid(max(plinth.x, plinth.y)));
    let collar = abs(point - (top + vec2<f32>(0.0, scale * 0.12))) - vec2<f32>(scale * 0.055, scale * 0.03);
    body = max(body, solid(max(collar.x, collar.y)));
    // The arm curves up and out over the road, toward the centre line.
    let reach = top + vec2<f32>(-side * scale * 0.75, scale * 0.04);
    let arm_bend = top + vec2<f32>(-side * scale * 0.1, -scale * 0.3);
    body = max(body, tapered_curve(point, top, arm_bend, reach, scale * 0.03, scale * 0.022));
    // The head: a long, flat housing with the lens underneath it.
    let head_centre = reach + vec2<f32>(-side * scale * 0.1, scale * 0.03);
    let housing = abs(point - head_centre) - vec2<f32>(scale * 0.24, scale * 0.045);
    body = max(body, solid(max(housing.x, housing.y) - scale * 0.02));
    let lens_centre = head_centre + vec2<f32>(0.0, scale * 0.06);
    let lens = length((point - lens_centre) / vec2<f32>(1.0, 0.3)) - scale * 0.2;
    let halo_size = scale * 0.3;
    let halo = halo_size / (length(point - lens_centre) + halo_size);
    // The light falls in a widening cone toward the ground.
    let below = point.y - lens_centre.y;
    let spread = scale * 0.2 + below * 0.45;
    let cone = step(0.0, below) * step(point.y, base.y)
        * (1.0 - smoothstep(spread * 0.6, spread, abs(point.x - lens_centre.x)))
        * (1.0 - below / max(base.y - lens_centre.y, 0.0001));
    return vec4<f32>(solid(lens), body, cone, halo);
}

/// A palm tree rooted at `base`, its trunk leaning away from the road.
fn palm_tree(point: vec2<f32>, base: vec2<f32>, side: f32, scale: f32) -> f32 {
    let crown = base + vec2<f32>(side * scale * 0.42, -scale * 2.3);
    let bend = base + vec2<f32>(side * scale * 0.02, -scale * 1.2);
    // A trunk thick at the root and thin under the crown, ringed where old fronds fell.
    var body = 0.0;
    var piece = 0.0;
    loop {
        if piece >= 5.0 { break; }
        let head = curve_at(base, bend, crown, piece / 5.0);
        let tail = curve_at(base, bend, crown, (piece + 1.0) / 5.0);
        let hit = segment_distance(point, head, tail);
        let t = (piece + hit.y) / 5.0;
        let rings = 0.8 + 0.2 * smoothstep(0.35, 0.5, abs(fract(t * 14.0) - 0.5));
        body = max(body, solid(hit.x - scale * mix(0.075, 0.04, t) * rings));
        piece += 1.0;
    }
    // Fronds fan out all round and droop, the lower ones most.
    var leaf = 0.0;
    var index = 0.0;
    loop {
        if index >= 8.0 { break; }
        let heading = -3.14159 * (0.02 + index / 7.0 * 0.96);
        let upward = -sin(heading);
        let reach = scale * mix(0.95, 1.25, hash11(index * 3.7 + 1.0));
        let droop = mix(0.95, 0.35, upward);
        leaf = max(leaf, frond(point, crown, heading, reach, droop));
        index += 1.0;
    }
    // Coconuts clustered under the crown.
    let nuts = min(
        length(point - (crown + vec2<f32>(scale * 0.07, scale * 0.08))),
        length(point - (crown + vec2<f32>(-scale * 0.06, scale * 0.09))),
    );
    return max(max(body, leaf), solid(nuts - scale * 0.055));
}

/// The scenery along one side of the road: its colour, and how much it covers.
///
/// Street lamps light up in a wave from the front to the back whenever a beat lands, their heads
/// turning white as it passes.
fn roadside(point: vec2<f32>, side: f32, scenery: f32, travel: f32) -> vec4<f32> {
    if scenery < 0.5 { return vec4<f32>(0.0); }
    var colour = vec3<f32>(0.0);
    var cover = 0.0;
    var index = 0.0;
    loop {
        if index >= 7.0 { break; }
        let depth = fract(index / 7.0 + travel * 0.09);
        let perspective = depth * depth;
        let base = vec2<f32>(side * (0.16 + perspective * 0.62), -0.04 + perspective * 1.02);
        let scale = 0.025 + perspective * 0.16;
        // Nothing to test for a pixel well away from this object.
        let offset = point - base;
        if abs(offset.x) > scale * 1.9 || offset.y > scale * 0.3 || offset.y < -scale * 3.7 {
            index += 1.0;
            continue;
        }
        // Fade in from the horizon, so an object never pops into existence.
        let appear = smoothstep(0.0, 0.12, depth);
        if scenery < 1.5 {
            // The wave runs from the nearest lamp to the farthest in half a second; each recent
            // beat sends one.
            var wave = 0.0;
            var landed = 0;
            loop {
                if landed >= 4 { break; }
                let front = 1.0 - beat_age(landed) * 2.0;
                if front < -0.3 { break; }
                let behind = (depth - front) / 0.14;
                wave = max(wave, exp(-behind * behind));
                landed += 1;
            }
            let lamp = street_lamp(point, base, side, scale);
            let body_colour = mix(secondary(), primary(), 0.5 + 0.5 * perspective);
            let head_colour = mix(primary() * 0.7, vec3<f32>(1.0), wave);
            let glow = max(lamp.w - 0.22, 0.0) * wave * 2.2;
            let head = lamp.x * (0.4 + wave * 1.8) + glow;
            let pool = lamp.z * (0.08 + wave * 0.3);
            colour = max(colour, (body_colour * lamp.y + head_colour * (head + pool)) * appear);
            cover = max(cover, max(lamp.y, clamp(head + pool, 0.0, 1.0)) * appear);
        } else {
            let palm = palm_tree(point, base, side, scale);
            let palm_colour = mix(secondary(), primary(), 0.35 + 0.65 * perspective);
            colour = max(colour, palm_colour * palm * appear);
            cover = max(cover, palm * appear);
        }
        index += 1.0;
    }
    return vec4<f32>(colour, clamp(cover, 0.0, 1.0));
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
    let ground_light = clamp(landscape + soft_sun, 0.0, 1.0);
    let ground = mix(secondary(), primary(), clamp(landscape, 0.0, 1.0)) * ground_light;
    let audio = 0.72 + energy() * 0.28;
    // The scenery stands in front of the landscape and keeps its own colours: a lamp's head can
    // flash white while the grid behind it stays in the palette.
    let scenery = max(left, right);
    let colour = mix(ground * audio, scenery.rgb, scenery.a);
    let light = clamp(max(ground_light, scenery.a), 0.0, 1.0);
    return vec4<f32>(colour, light);
}
