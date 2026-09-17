// Music props -- headphones, cassettes, records, microphones and notes -- flying through the frame.
//
// Every prop is a signed distance field, so the set ships with the shader and needs no model files.
// Each pixel tests every prop's projected bounding circle first and raymarches only the few it can
// see, so a crowd of props costs little more than one.
//
// Flight pattern (mode, repeating every four): 0 fly-through toward the camera, 1 drift across,
// 2 orbit around the centre, 3 rise from the bottom. Speed sets the flight and spin rate, Count the
// number of props, Size how big each one is. A held kick swells the props and pushes their flight
// forward, scaled by Reactivity; a held snare or
// the beat flash brightens the accents. Nothing raw from the analysis
// is read, so a loud input cannot blow the picture out.

const PROPS_MOST: i32 = 48;
const FOCAL: f32 = 1.6;
const PROP_KINDS: f32 = 5.0;

fn fp_rotate(point: vec3<f32>, yaw: f32, pitch: f32, roll: f32) -> vec3<f32> {
    let cy = cos(yaw);
    let sy = sin(yaw);
    let cp = cos(pitch);
    let sp = sin(pitch);
    let cr = cos(roll);
    let sr = sin(roll);
    let a = vec3<f32>(point.x * cy - point.z * sy, point.y, point.x * sy + point.z * cy);
    let b = vec3<f32>(a.x, a.y * cp - a.z * sp, a.y * sp + a.z * cp);
    return vec3<f32>(b.x * cr - b.y * sr, b.x * sr + b.y * cr, b.z);
}

fn fp_box(point: vec3<f32>, half: vec3<f32>, rounding: f32) -> f32 {
    let q = abs(point) - half + vec3<f32>(rounding);
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - rounding;
}

// A cylinder along local y.
fn fp_cylinder(point: vec3<f32>, radius: f32, half_height: f32) -> f32 {
    let d = vec2<f32>(length(point.xz) - radius, abs(point.y) - half_height);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

// A cylinder along local z.
fn fp_disc(point: vec3<f32>, radius: f32, half_depth: f32) -> f32 {
    return fp_cylinder(point.xzy, radius, half_depth);
}

// Material ids: 0 body (primary), 1 accent (secondary), 2 dark detail.
fn fp_pick(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    if b.x < a.x { return b; }
    return a;
}

fn fp_headphones(p: vec3<f32>) -> vec2<f32> {
    // The band: the upper half of a torus in the xy plane.
    let ring = vec2<f32>(length(p.xy) - 0.72, p.z);
    var band = length(ring) - 0.07;
    band = max(band, -p.y - 0.05);
    var result = vec2<f32>(band, 0.0);
    // Ear cups, facing each other along x.
    let side = vec3<f32>(abs(p.x) - 0.74, p.y + 0.12, p.z);
    let cup = fp_cylinder(side.yxz, 0.3, 0.1);
    result = fp_pick(result, vec2<f32>(cup, 0.0));
    let cushion = fp_cylinder((side - vec3<f32>(-0.13, 0.0, 0.0)).yxz, 0.25, 0.05) - 0.02;
    result = fp_pick(result, vec2<f32>(cushion, 1.0));
    return result;
}

fn fp_cassette(p: vec3<f32>) -> vec2<f32> {
    var shell = fp_box(p, vec3<f32>(0.95, 0.6, 0.12), 0.04);
    // Two spool windows through the shell.
    let spool = vec3<f32>(abs(p.x) - 0.36, p.y + 0.05, p.z);
    let hole = fp_disc(spool, 0.13, 0.2);
    shell = max(shell, -hole);
    var result = vec2<f32>(shell, 0.0);
    let label = fp_box(p - vec3<f32>(0.0, 0.2, 0.0), vec3<f32>(0.78, 0.2, 0.125), 0.01);
    result = fp_pick(result, vec2<f32>(max(label, -hole), 1.0));
    let hub = fp_disc(spool, 0.07, 0.1);
    result = fp_pick(result, vec2<f32>(hub, 2.0));
    return result;
}

fn fp_record(p: vec3<f32>) -> vec2<f32> {
    let hole = fp_disc(p, 0.04, 0.2);
    let disc = max(fp_disc(p, 1.0, 0.02), -hole);
    // Grooves read as faint rings on the dark vinyl.
    var result = vec2<f32>(disc, 2.0);
    let label = max(fp_disc(p, 0.34, 0.03), -hole);
    result = fp_pick(result, vec2<f32>(label, 1.0));
    return result;
}

fn fp_microphone(p: vec3<f32>) -> vec2<f32> {
    let head = length(p - vec3<f32>(0.0, 0.55, 0.0)) - 0.34;
    var result = vec2<f32>(head, 2.0);
    let collar = fp_cylinder(p - vec3<f32>(0.0, 0.22, 0.0), 0.2, 0.06);
    result = fp_pick(result, vec2<f32>(collar, 1.0));
    // A handle that narrows toward its end.
    let along = clamp((p.y + 0.8) / 1.0, 0.0, 1.0);
    let handle = fp_cylinder(p - vec3<f32>(0.0, -0.3, 0.0), 0.1 + along * 0.08, 0.5);
    result = fp_pick(result, vec2<f32>(handle * 0.8, 0.0));
    return result;
}

fn fp_note(p: vec3<f32>) -> vec2<f32> {
    // Two beamed notes: tilted heads, stems, and a beam across their tops.
    let local = vec3<f32>(abs(p.x + 0.0) - 0.35, p.y + 0.55, p.z);
    let tilted = vec3<f32>(
        local.x * 0.87 - local.y * 0.5,
        local.x * 0.5 + local.y * 0.87,
        local.z,
    );
    let head = (length(tilted / vec3<f32>(0.22, 0.15, 0.12)) - 1.0) * 0.12;
    var result = vec2<f32>(head, 0.0);
    let stem = fp_box(vec3<f32>(local.x - 0.18, p.y + 0.05, p.z), vec3<f32>(0.03, 0.5, 0.03), 0.0);
    result = fp_pick(result, vec2<f32>(stem, 0.0));
    let beam = fp_box(p - vec3<f32>(0.0, 0.45, 0.0), vec3<f32>(0.56, 0.07, 0.04), 0.0);
    result = fp_pick(result, vec2<f32>(beam, 1.0));
    return result;
}

fn fp_prop(point: vec3<f32>, kind: f32) -> vec2<f32> {
    if kind < 0.5 { return fp_headphones(point); }
    if kind < 1.5 { return fp_cassette(point); }
    if kind < 2.5 { return fp_record(point); }
    if kind < 3.5 { return fp_microphone(point); }
    return fp_note(point);
}

struct FpProp {
    centre: vec3<f32>,
    scale: f32,
    spin: vec3<f32>,
    kind: f32,
    fade: f32,
};

/// Where one prop is at this instant, from its seeds, the flight pattern and the flight phase.
fn fp_place(index: f32, travel: f32, spin_phase: f32, swell: f32) -> FpProp {
    let h1 = hash11(index * 1.37 + 0.11);
    let h2 = hash11(index * 2.71 + 3.3);
    let h3 = hash11(index * 5.19 + 7.7);
    let h4 = hash11(index * 9.83 + 1.9);
    let pattern = floor(mode() + 0.5) % 4.0;
    // World units: the whole picture is two units tall at depth FOCAL.
    let scale = size() * 4.0 * (0.75 + h4 * 0.5) * swell;
    let rate = 0.6 + h4 * 0.8;

    var out: FpProp;
    out.scale = scale;
    // Taken in turn rather than at random, so every kind of prop is on screen at any density.
    out.kind = index % PROP_KINDS;
    out.fade = 1.0;
    out.spin = vec3<f32>(
        h1 * TAU + spin_phase * (0.4 + h2) * select(1.0, -1.0, h3 > 0.5),
        h2 * TAU + spin_phase * (0.2 + h4 * 0.5),
        h4 * TAU + spin_phase * 0.15 * (h1 - 0.5),
    );

    if pattern < 0.5 {
        // Fly-through: from deep in the picture past the camera.
        let phase = fract(h1 + travel * 0.08 * rate);
        let depth = mix(10.0, 1.6, phase);
        let spread = vec2<f32>(aspect(), 1.0) * (0.35 + h4 * 1.6);
        let angle = h2 * TAU;
        out.centre = vec3<f32>(cos(angle) * spread.x, sin(angle) * spread.y, depth);
        // In from the distance, and out again before a prop fills the picture.
        out.fade = smoothstep(0.0, 0.25, phase) * (1.0 - smoothstep(0.8, 0.95, phase));
    } else if pattern < 1.5 {
        // Drift: sideways across the frame, the far ones slower.
        let depth = 3.0 + h1 * 7.0;
        let half_width = depth * aspect() / FOCAL + scale * 1.5;
        let direction = select(1.0, -1.0, h4 > 0.7);
        let phase = fract(h2 + travel * 0.2 * rate / depth * 3.0);
        let x = mix(-half_width, half_width, phase) * direction;
        let y = (h3 * 2.0 - 1.0) * depth / FOCAL * 0.8 + sin(travel * 0.7 + h1 * TAU) * 0.15;
        out.centre = vec3<f32>(x, y, depth);
    } else if pattern < 2.5 {
        // Orbit: a tilted carousel around the middle of the picture.
        let angle = h1 * TAU + travel * 0.25 * select(1.0, 1.15, h4 > 0.5);
        let reach = 1.6 + h2 * 1.4;
        out.centre = vec3<f32>(
            cos(angle) * reach * aspect() * 0.8,
            (h3 - 0.5) * 1.6 + sin(angle) * 0.5,
            6.0 + sin(angle) * reach,
        );
    } else {
        // Rise: floating up from below the picture and out of the top.
        let depth = 3.0 + h1 * 7.0;
        let half_height = depth / FOCAL + scale * 1.5;
        let phase = fract(h2 + travel * 0.15 * rate / depth * 3.0);
        let y = mix(-half_height, half_height, phase);
        let x = (h3 * 2.0 - 1.0) * depth * aspect() / FOCAL * 0.9
            + sin(travel * 0.5 + h4 * TAU) * 0.3;
        out.centre = vec3<f32>(x, y, depth);
    }
    return out;
}

fn fp_local(prop: FpProp, world: vec3<f32>) -> vec3<f32> {
    return fp_rotate((world - prop.centre) / prop.scale, prop.spin.x, prop.spin.y, prop.spin.z);
}

fn fp_normal(prop: FpProp, local: vec3<f32>) -> vec3<f32> {
    let e = 0.004;
    let k = vec2<f32>(1.0, -1.0);
    let n = k.xyy * fp_prop(local + k.xyy * e, prop.kind).x
        + k.yyx * fp_prop(local + k.yyx * e, prop.kind).x
        + k.yxy * fp_prop(local + k.yxy * e, prop.kind).x
        + k.xxx * fp_prop(local + k.xxx * e, prop.kind).x;
    return normalize(n);
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let held = held_hits();
    let push = clamp(reactivity(), 0.0, 2.0);
    // Time that the clock and the kick both move. `hit_turns` already scales with speed.
    let travel = fract(seconds() * speed() / 400.0) * 400.0 + hit_turns().x * push * 4.0;
    let spin_phase = fract(seconds() * speed() / 500.0) * 500.0 + hit_turns().y * push * 3.0;
    let swell = 1.0 + held.x * push * 0.18;
    // The beat flash and a held snare both light the accents; each is 0..1, so this stays bounded.
    let accent_flash = 1.0 + max(held.y, clamp(beat(), 0.0, 1.0)) * push * 0.6;

    // y up in the world, x right.
    let direction = normalize(vec3<f32>(p.x, -p.y, FOCAL));
    let props = min(i32(count()), PROPS_MOST);

    var nearest = 1e9;
    var colour = vec3<f32>(0.0);
    var alpha = 0.0;

    for (var index = 0; index < props; index += 1) {
        let prop = fp_place(f32(index), travel, spin_phase, swell);
        // Every prop fits in a sphere of 1.2 local units.
        let bound = prop.scale * 1.2;
        let along = dot(prop.centre, direction);
        if along <= 0.0 || along - bound > nearest { continue; }
        let off_axis = length(prop.centre - direction * along);
        if off_axis > bound { continue; }

        var travelled = max(along - bound, 0.05);
        let far = along + bound;
        var hit = false;
        var local = vec3<f32>(0.0);
        var material = 0.0;
        for (var march = 0; march < 40; march += 1) {
            local = fp_local(prop, direction * travelled);
            let found = fp_prop(local, prop.kind);
            if found.x < 0.003 {
                hit = true;
                material = found.y;
                break;
            }
            travelled += found.x * prop.scale * 0.9;
            if travelled > far { break; }
        }
        if !hit || travelled >= nearest { continue; }
        nearest = travelled;

        // Straight colour: the fade belongs in alpha only, as the layer shader expects.
        // Light in the prop's own frame, so shading turns with it.
        let normal = fp_normal(prop, local);
        let light = normalize(fp_rotate(vec3<f32>(0.5, 0.7, -0.6), prop.spin.x, prop.spin.y, prop.spin.z));
        let view = normalize(fp_rotate(-direction, prop.spin.x, prop.spin.y, prop.spin.z));
        let diffuse = max(dot(normal, light), 0.0);
        let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0);
        var base = primary();
        if material > 1.5 {
            base = primary() * 0.22 + vec3<f32>(0.06);
            if prop.kind > 1.5 && prop.kind < 2.5 {
                // Record grooves.
                base += vec3<f32>(0.05) * step(0.5, fract(length(local.xy) * 40.0));
            }
        } else if material > 0.5 {
            base = secondary() * accent_flash;
        }
        let depth_dim = clamp(1.25 - nearest / 18.0, 0.35, 1.0);
        colour = (base * (0.22 + diffuse * 0.85) + vec3<f32>(rim * 0.35)) * depth_dim;
        alpha = prop.fade;
    }
    return vec4<f32>(colour, alpha);
}
