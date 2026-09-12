// A wavy triangular net seen from just above its own surface: nodes joined by dead-straight paths,
// riding four bands of rolling swell, dissolving into nothing before the horizon.
//
// The net is never subdivided, exactly as in the still it is modelled on -- every path stays one
// straight segment between two nodes however wild the waves get, and all the ruggedness comes from
// moving the nodes up and down.
//
// Nothing here moves on its own. The clock is never read: the wave is carried forward by the music
// and by nothing else, so in a silent room the net holds the shape it was drawn with and stands
// still. Bass carries the big swells, treble the fine detail, and a held spectrum laid across the
// net's width decides where the wave stands high. That is the whole point of it as a visualizer
// rather than a wallpaper: an operator watching the output can see which part of the sound is
// moving which part of the picture.
//
// Everything is drawn from the height field, never from a list of nodes: the ray is walked forward
// until it meets the surface, and only the handful of nodes around that meeting point are worked
// out. A ridge therefore hides what stands behind it, and the cost of a fragment does not grow
// with how much net is in shot.

// --- the surface ------------------------------------------------------------------------------
//
// Metres, matching the scene the look comes from: nodes about 0.8 m apart on a plane 52 m across,
// with the camera 2.45 m above it and 17 m back from its middle, tilted 4 degrees up so the
// horizon lands just below the middle of the frame. The setback is also what the front-to-back
// bend is measured against, so bending the net keeps shaping the same stretch of it.
const NET_HALF_WIDTH: f32 = 26.0;
const NET_SETBACK: f32 = 17.0;
const NET_CAMERA_HEIGHT: f32 = 2.45;
const NET_CAMERA_TILT: f32 = 0.0698132;
// The width `count` divides, so the control reads as "nodes across the middle of the net".
const NET_SPAN: f32 = 26.0;
// A row of a triangular lattice sits sqrt(3)/2 of a spacing from the next.
const NET_ROW_PITCH: f32 = 0.8660254;
const NET_JITTER: f32 = 0.25;
// Metres from the camera at which the net is still solid, and at which it has gone entirely.
const NET_FADE_START: f32 = 24.0;
const NET_FADE_END: f32 = 38.0;
// A bend of +/-1 reaches this many metres at the far edge.
const NET_BEND_HEIGHT: f32 = 8.0;

/// How high the sound is holding this position across the net, `0..1`.
///
/// The held levels, not the live bands. A band that is loud right now is a flicker at this scale;
/// what an operator wants to see is the hit itself -- a bass beat raises a mountain where the bass
/// sits, and the mountain then sinks back over as long as `decay` says. They arrive as a share of
/// the loudest thing heard lately, so none of this depends on how anyone set their input gain, and
/// a mountain genuinely falls rather than being renormalised back to full height the moment the
/// rest of the music goes quiet.
fn net_share(across: f32) -> f32 {
    return clamp(band_held_at(clamp(across, 0.0, 1.0)), 0.0, 1.0);
}

/// One wave component: `amplitude * sin(k . position + w t)`, where `carried` is how far the tone
/// this component belongs to has taken it. Whole numbers of cycles, so a band that is carried all
/// the way round returns to exactly where it started.
fn net_wave(position: vec2<f32>, carried: f32, amplitude: f32, k: vec2<f32>, cycles: f32, phase: f32) -> f32 {
    return amplitude * sin(dot(k, position) + TAU * (cycles * carried + phase));
}

/// How high the net stands at a point on the plane.
///
/// Four bands of wave -- big rolling swells, mid-scale chop, fine detail, and a scatter short
/// enough that neighbouring nodes never quite agree with each other. Height and movement come from
/// different places on purpose. What the music is *holding* at a point across the net sets how
/// high the wave stands there; which tone is *playing* sets which of the four bands is moving, so
/// bass rolls the swells along and treble scurries the detail. In a silent room the net keeps the
/// shape it was drawn with and stands perfectly still.
///
/// This is the only definition of the surface, and the nodes stand exactly on it. That is what
/// lets one walk of the ray answer both where the surface is and which nodes are visible there.
fn net_height(position: vec2<f32>, turns: vec3<f32>) -> f32 {
    let reach = clamp(reactivity(), 0.0, 2.0);
    var across = (position.x + NET_HALF_WIDTH) / (2.0 * NET_HALF_WIDTH);
    if mirrored() {
        across = abs(position.x) / NET_HALF_WIDTH;
    }
    // The designed height is the floor and the music builds on it, rather than the music being all
    // there is: a quiet passage leaves the net looking like itself instead of flattening it. The
    // ceiling is there because the camera stands 2.45 m up and a wave that swallows it is a fault.
    let here = min(1.0 + net_share(across) * reach * 0.90, 1.90);

    let scale = amount() * here;

    // The three heights below are the ones the still was built with, and they still add up to it.
    // What has changed is what carries each band round: the swells are the bass's to move, the
    // chop the mid's, the detail and the scatter the treble's. The cycle counts inside a band are
    // what make its components drift against each other rather than march in step.
    var height = 0.0;
    // Big rolling swells, 0.60 m of height, carried by the bass. Slowest, so a bass hit reads as
    // the whole landscape heaving rather than as anything twitching.
    height += net_wave(position, turns.x, scale * 0.288, vec2<f32>(0.115, 0.055), 1.0, 0.00);
    height += net_wave(position, turns.x, scale * 0.192, vec2<f32>(-0.070, 0.130), 2.0, 0.35);
    height += net_wave(position, turns.x, scale * 0.120, vec2<f32>(0.190, -0.090), 3.0, 0.70);
    // Mid-scale chop, 1.35 m, carried by the mid at several times the cycles.
    height += net_wave(position, turns.y, scale * 0.608, vec2<f32>(0.240, 0.230), 4.0, 0.15);
    height += net_wave(position, turns.y, scale * 0.432, vec2<f32>(-0.330, 0.150), 8.0, 0.55);
    height += net_wave(position, turns.y, scale * 0.311, vec2<f32>(0.420, -0.360), 6.0, 0.20);
    // Fine detail, 0.65 m, carried by the treble, and fastest of the three.
    height += net_wave(position, turns.z, scale * 0.299, vec2<f32>(0.720, 0.480), 15.0, 0.80);
    height += net_wave(position, turns.z, scale * 0.208, vec2<f32>(-0.610, 0.830), 18.0, 0.42);
    height += net_wave(position, turns.z, scale * 0.143, vec2<f32>(1.050, -0.740), 24.0, 0.10);
    // Scatter, 0.45 m at a wavelength of about two node spacings, on the treble with the detail.
    // Three components at incommensurate angles, so it reads as per-node randomness rather than as
    // a pattern, while staying a smooth field the ray can be walked against. The still it comes
    // from scatters each node independently; nothing a ray can be traced through can do that, and
    // at this wavelength no one can tell the difference.
    height += net_wave(position, turns.z, scale * 0.180, vec2<f32>(3.310, -2.150), 16.0, 0.13);
    height += net_wave(position, turns.z, scale * 0.153, vec2<f32>(-2.620, 3.440), 20.0, 0.61);
    height += net_wave(position, turns.z, scale * 0.117, vec2<f32>(4.530, 4.100), 28.0, 0.29);

    // The static front-to-back bend. Centred, so the middle of the control is a flat plane.
    let bend = (curvature() - 0.5) * 2.0;
    let depth = clamp((position.y + NET_SETBACK) / (2.0 * NET_SETBACK), 0.0, 1.0);
    if depth > 0.55 {
        let along = (depth - 0.55) / 0.45;
        height += bend * NET_BEND_HEIGHT * along * along;
    }
    return height;
}

/// Where one node of the lattice stands: its scattered place on the plane, and its height there.
fn net_node(cell: vec2<i32>, spacing: f32, turns: vec3<f32>) -> vec3<f32> {
    let row = f32(cell.y);
    let offset = f32(cell.y & 1) * 0.5;
    let base = vec2<f32>((f32(cell.x) + offset) * spacing, row * spacing * NET_ROW_PITCH);
    let scatter = (hash22(vec2<f32>(f32(cell.x), row)) - 0.5) * 2.0 * NET_JITTER * spacing;
    let place = base + scatter;
    return vec3<f32>(place, net_height(place, turns));
}

fn net_focal() -> f32 {
    // A 24 mm lens, locked on the vertical axis, so the framing survives a change of aspect.
    return 2.0 / zoom();
}

fn net_forward() -> vec3<f32> {
    return vec3<f32>(0.0, cos(NET_CAMERA_TILT), sin(NET_CAMERA_TILT));
}

fn net_up() -> vec3<f32> {
    return vec3<f32>(0.0, -sin(NET_CAMERA_TILT), cos(NET_CAMERA_TILT));
}

fn net_camera() -> vec3<f32> {
    return vec3<f32>(0.0, -NET_SETBACK, NET_CAMERA_HEIGHT);
}

/// A node's place on screen, with the distance to it in `z`. A `z` of zero means behind the lens.
fn net_project(world: vec3<f32>) -> vec3<f32> {
    let view = world - net_camera();
    let distance = dot(view, net_forward());
    if distance < 0.25 {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let focal = net_focal();
    return vec3<f32>(
        view.x * focal / distance,
        -dot(view, net_up()) * focal / distance,
        distance,
    );
}

fn net_segment(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>, width: f32) -> f32 {
    let span = end - start;
    let along = clamp(dot(point - start, span) / max(dot(span, span), 0.0001), 0.0, 1.0);
    return 1.0 - smoothstep(width, width + 0.0035, length(point - (start + span * along)));
}

/// How solid the net is at this distance. Past the end of the fade there is nothing left to draw,
/// which is also where the walk gives up: the net has no edge, it runs out of sight.
fn net_fade(distance: f32) -> f32 {
    return 1.0 - smoothstep(NET_FADE_START, NET_FADE_END, distance);
}

// Five rows of five, centred on the cell the ray came down in. The surface the ray met is the one
// in front, so everything a ridge hides is already excluded and the window can stay small.
const NET_REACH: i32 = 2;
const NET_SIDE: i32 = 5;
const NET_CELLS: i32 = 25;
// Steps grow with distance, because so does the ground one step covers on screen.
const NET_STEPS: i32 = 72;

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    // Time, as the music has doled it out. Nothing here reads the clock: `speed` is the exchange
    // rate between sound and motion, and it is applied where the turns are counted.
    let turns = tone_turns();
    let focal = net_focal();
    let camera = net_camera();
    let forward = net_forward();
    let up = net_up();

    // The ray this fragment looks along, scaled so travelling `t` along it covers `t` metres of
    // distance from the lens -- which makes every distance below directly comparable.
    let ray = forward + vec3<f32>(p.x / focal, 0.0, 0.0) + up * (-p.y / focal);
    if ray.z > -0.0005 {
        // Level with the horizon or above it: the surface is not in this direction at all.
        return vec4<f32>(0.0);
    }

    // Walk forward to the first crossing, then close in on it. Bisecting is what makes this hold
    // up at a grazing angle, where the surface slides along under the ray for metres at a time and
    // dropping onto the flat plane and correcting from there would never settle.
    let limit = NET_FADE_END + 3.0;
    var distance = -1.0;
    var near = 0.3;
    for (var step = 0; step < NET_STEPS && near < limit; step += 1) {
        let far = min(near + 0.30 + near * 0.06, limit);
        let point = camera + ray * far;
        if point.z < net_height(point.xy, turns) {
            var low = near;
            var high = far;
            for (var closing = 0; closing < 4; closing += 1) {
                let middle = (low + high) * 0.5;
                let sample = camera + ray * middle;
                if sample.z < net_height(sample.xy, turns) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            distance = (low + high) * 0.5;
            break;
        }
        near = far;
    }
    if distance < 0.0 {
        return vec4<f32>(0.0);
    }

    let ground = camera + ray * distance;
    let spacing = NET_SPAN / max(count(), 1.0);
    let row = i32(round(ground.y / (spacing * NET_ROW_PITCH)));
    let column = i32(round(ground.x / spacing - f32(row & 1) * 0.5));

    // Every node in reach, projected once, so an edge can be drawn from both of its ends without
    // working either of them out twice.
    var screen: array<vec3<f32>, 25>;
    for (var index = 0; index < NET_CELLS; index += 1) {
        let cell = vec2<i32>(
            column + index % NET_SIDE - NET_REACH,
            row + index / NET_SIDE - NET_REACH,
        );
        screen[index] = net_project(net_node(cell, spacing, turns));
    }

    let node_size = 0.015 + size() * 0.30;
    let path_width = thickness() * 1.4;
    var light = 0.0;
    var glow = 0.0;

    for (var index = 0; index < NET_CELLS; index += 1) {
        let here = screen[index];
        if here.z <= 0.0 { continue; }
        let column_offset = index % NET_SIDE;
        let row_offset = index / NET_SIDE;
        let odd = (row + row_offset - NET_REACH) & 1;

        // The three paths that leave this node without repeating one another: along the row, and
        // the two that reach the row behind it.
        var reach = array<i32, 3>(
            select(-1, index + 1, column_offset + 1 < NET_SIDE),
            select(-1, index + NET_SIDE + odd, row_offset + 1 < NET_SIDE && column_offset + odd < NET_SIDE),
            select(-1, index + NET_SIDE + odd - 1, row_offset + 1 < NET_SIDE && column_offset + odd - 1 >= 0),
        );
        for (var leg = 0; leg < 3; leg += 1) {
            let other = reach[leg];
            if other < 0 { continue; }
            let there = screen[other];
            if there.z <= 0.0 { continue; }
            let span = min(here.z, there.z);
            // A path thinner than about a pixel would simply drop out, and the far field is where
            // most of the paths are: hold every one at a drawable width and let the distance fade,
            // not the arithmetic, decide where the net stops.
            let width = max(path_width * focal / span, 0.0016);
            light = max(light, net_segment(p, here.xy, there.xy, width) * net_fade(span));
        }

        // The nodes sit over the paths rather than half buried in them, and the most distant ones
        // are drawn smaller so the far field reads as scatter instead of as a wall of dots.
        let shrink = mix(1.0, 0.55, smoothstep(14.0, 34.0, here.z));
        let radius = max(node_size * shrink * focal / here.z, 0.0022);
        let dot_light = solid(length(p - here.xy) - radius) * net_fade(here.z);
        light = max(light, dot_light);
        glow = max(glow, dot_light);
    }

    if light <= 0.0 {
        return vec4<f32>(0.0);
    }

    // Which part of the sound this part of the net belongs to, so the loud end of the spectrum
    // colours its own stretch of the wave rather than tinting the whole picture.
    var across = (ground.x + NET_HALF_WIDTH) / (2.0 * NET_HALF_WIDTH);
    if mirrored() {
        across = abs(ground.x) / NET_HALF_WIDTH;
    }
    let local = clamp(net_share(across) * clamp(reactivity(), 0.0, 2.0), 0.0, 1.0);
    let colour = mix(primary(), secondary(), local);

    // The scene it comes from is lit, not glowing: the near net is bright and distance does the
    // rest of the depth work.
    let lit = 0.45 + 0.55 * (1.0 - smoothstep(2.0, 30.0, distance));
    // The nodes ignore the lights in the scene this comes from, so nothing shadows them: here they
    // simply sit a little above the paths they join, and a beat lifts them further.
    let flash = 1.0 + glow * (0.35 + clamp(beat(), 0.0, 1.0) * 0.45);
    // Energy is a root-mean-square of the window, not a normalised level, so a hot input would
    // otherwise blow the net out entirely.
    let level = light * lit * flash * (0.80 + clamp(energy(), 0.0, 1.0) * 0.35);
    return vec4<f32>(colour * level, clamp(level, 0.0, 1.0));
}
