// A wavy triangular net seen from just above its own surface: nodes joined by dead-straight paths,
// riding four bands of rolling swell, dissolving into nothing before the horizon.
//
// The net is never subdivided, exactly as in the still it is modelled on -- every path stays one
// straight segment between two nodes however wild the waves get, and all the ruggedness comes from
// moving the nodes up and down. It is a wireframe and nothing else: there is no surface to hide
// behind, so the far net shows through the near net the way it does in the still.
//
// Nothing here moves on its own. The clock is never read: the wave is carried forward by the beat
// and by nothing else. A kick heaves the big swells a long way and raises them into mountains, a
// snare shoves the mid-scale chop, a hi-hat ripples the fine detail a little, and each push eases
// out at the rate `decay` asks for. Between songs, or on a dead input, the net holds the shape it
// was drawn with and stands still.
//
// Everything is drawn from the height field, never from a list of nodes: the ray is walked through
// the band of space the net can occupy, and at every place it passes through the surface the
// handful of nodes around that place are worked out. The cost of a fragment therefore does not
// grow with how much net is in shot.

// --- the surface ------------------------------------------------------------------------------
//
// Metres, matching the scene the look comes from: nodes about 0.8 m apart, with the camera 2.45 m
// above the plane and 17 m back from its middle, tilted 4 degrees up so the horizon lands just
// below the middle of the frame. The setback is also what the front-to-back bend is measured
// against, so bending the net keeps shaping the same stretch of it.
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
// The total height of each band as the still was built, and so the most each can add or take away
// at its designed size: swells, chop, detail, scatter.
const NET_SWELL: f32 = 0.600;
const NET_CHOP: f32 = 1.351;
const NET_DETAIL: f32 = 0.650;
const NET_SCATTER: f32 = 0.450;

/// What the beat is doing, read once per fragment and handed down, so nothing inside the walk
/// touches the analysis texture.
struct NetBeat {
    /// How far the kick, snare and hi-hat have each carried their band of the wave, in turns.
    turns: vec3<f32>,
    /// Height gain for the swells, the chop, and the detail with the scatter.
    gains: vec3<f32>,
    /// What is left of the last kick, `0..1`, for everything that is not height or movement.
    kick: f32,
}

fn net_beat() -> NetBeat {
    let reach = clamp(reactivity(), 0.0, 2.0);
    let held = clamp(held_hits(), vec3<f32>(0.0), vec3<f32>(1.0));
    var drive: NetBeat;
    drive.turns = hit_turns();
    // A kick raises mountains: the swells can stand two and a half times their designed height
    // while it is held. A snare or a hi-hat roughens the surface rather than lifting it, so each of
    // those gets a fraction of that. At rest every band is exactly the height the still was built
    // with -- the beat builds on the net rather than being all there is to it.
    drive.gains = amount() * (vec3<f32>(1.0) + held * reach * vec3<f32>(1.5, 0.5, 0.5));
    drive.kick = held.x;
    return drive;
}

/// One wave component: `amplitude * sin(k . position + w t)`, where `carried` is how far the
/// instrument this component belongs to has taken it. Whole numbers of cycles, so a band carried
/// all the way round returns exactly to where it started.
fn net_wave(position: vec2<f32>, carried: f32, amplitude: f32, k: vec2<f32>, cycles: f32, phase: f32) -> f32 {
    return amplitude * sin(dot(k, position) + TAU * (cycles * carried + phase));
}

/// How high the net stands at a point on the plane.
///
/// Four bands of wave, and the scale of a movement says which instrument made it: the kick carries
/// the big rolling swells, the snare the mid-scale chop, and the hi-hat the fine detail together
/// with a scatter short enough that neighbouring nodes never quite agree with each other.
///
/// This is the only definition of the surface, and the nodes stand exactly on it. That is what
/// lets the walk of a ray answer both where the net is and which nodes are visible there.
fn net_height(position: vec2<f32>, drive: NetBeat) -> f32 {
    let turns = drive.turns;
    let swell = drive.gains.x;
    let chop = drive.gains.y;
    let detail = drive.gains.z;

    var height = 0.0;
    // Big rolling swells, carried by the kick. Slowest and widest, so a kick reads as the whole
    // landscape heaving rather than as anything twitching.
    height += net_wave(position, turns.x, swell * 0.288, vec2<f32>(0.115, 0.055), 1.0, 0.00);
    height += net_wave(position, turns.x, swell * 0.192, vec2<f32>(-0.070, 0.130), 2.0, 0.35);
    height += net_wave(position, turns.x, swell * 0.120, vec2<f32>(0.190, -0.090), 3.0, 0.70);
    // Mid-scale chop, carried by the snare.
    height += net_wave(position, turns.y, chop * 0.608, vec2<f32>(0.240, 0.230), 4.0, 0.15);
    height += net_wave(position, turns.y, chop * 0.432, vec2<f32>(-0.330, 0.150), 8.0, 0.55);
    height += net_wave(position, turns.y, chop * 0.311, vec2<f32>(0.420, -0.360), 6.0, 0.20);
    // Fine detail, carried by the hi-hat.
    height += net_wave(position, turns.z, detail * 0.299, vec2<f32>(0.720, 0.480), 15.0, 0.80);
    height += net_wave(position, turns.z, detail * 0.208, vec2<f32>(-0.610, 0.830), 18.0, 0.42);
    height += net_wave(position, turns.z, detail * 0.143, vec2<f32>(1.050, -0.740), 24.0, 0.10);
    // Scatter, at a wavelength of about two node spacings, on the hi-hat with the detail. Three
    // components at incommensurate angles, so it reads as per-node randomness rather than as a
    // pattern, while staying a smooth field a ray can be walked through. The still scatters each
    // node independently; nothing a ray can be traced through can do that, and at this wavelength
    // no one can tell the difference.
    height += net_wave(position, turns.z, detail * 0.180, vec2<f32>(3.310, -2.150), 16.0, 0.13);
    height += net_wave(position, turns.z, detail * 0.153, vec2<f32>(-2.620, 3.440), 20.0, 0.61);
    height += net_wave(position, turns.z, detail * 0.117, vec2<f32>(4.530, 4.100), 28.0, 0.29);

    // The static front-to-back bend. Centred, so the middle of the control is a flat plane.
    let bend = (curvature() - 0.5) * 2.0;
    let depth = clamp((position.y + NET_SETBACK) / (2.0 * NET_SETBACK), 0.0, 1.0);
    if depth > 0.55 {
        let along = (depth - 0.55) / 0.45;
        height += bend * NET_BEND_HEIGHT * along * along;
    }
    return height;
}

/// The lowest and highest the net can stand anywhere, as driven right now.
///
/// This is what lets a ray above the horizon be walked at all. A kick can raise the swells higher
/// than the camera stands, and a ray looking up at such a peak still has to find it -- throwing
/// every upward ray away is exactly what sliced the tops off the mountains. Outside this band a ray
/// can never meet the net again, which is also what lets the walk stop early.
fn net_extent(drive: NetBeat) -> vec2<f32> {
    let reach = drive.gains.x * NET_SWELL + drive.gains.y * NET_CHOP
        + drive.gains.z * (NET_DETAIL + NET_SCATTER);
    let bend = (curvature() - 0.5) * 2.0 * NET_BEND_HEIGHT;
    return vec2<f32>(-reach + min(bend, 0.0), reach + max(bend, 0.0));
}

/// Where one node of the lattice stands: its scattered place on the plane, and its height there.
fn net_node(cell: vec2<i32>, spacing: f32, drive: NetBeat) -> vec3<f32> {
    let row = f32(cell.y);
    let offset = f32(cell.y & 1) * 0.5;
    let base = vec2<f32>((f32(cell.x) + offset) * spacing, row * spacing * NET_ROW_PITCH);
    let scatter = (hash22(vec2<f32>(f32(cell.x), row)) - 0.5) * 2.0 * NET_JITTER * spacing;
    let place = base + scatter;
    return vec3<f32>(place, net_height(place, drive));
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

// Five rows of five, centred on the cell where the ray passes through the net.
const NET_REACH: i32 = 2;
const NET_SIDE: i32 = 5;
const NET_CELLS: i32 = 25;
// Steps grow with distance, because so does the ground one step covers on screen, and stop
// growing before they could step clean over a mid-scale peak.
const NET_STEPS: i32 = 72;
const NET_LONGEST_STEP: f32 = 1.5;
// A see-through net can be passed through several times along one ray: into a near swell, out of
// it, into the one behind. Past a few of those the far ones are faded to nothing anyway.
const NET_CROSSINGS: i32 = 4;

/// How much a fragment is lit by the net where the ray passes through it at `ground`.
struct NetLight {
    light: f32,
    /// The part of `light` that is nodes rather than paths.
    glow: f32,
}

fn net_draw(p: vec2<f32>, ground: vec3<f32>, drive: NetBeat, focal: f32) -> NetLight {
    let spacing = NET_SPAN / max(count(), 1.0);
    let row = i32(round(ground.y / (spacing * NET_ROW_PITCH)));
    let column = i32(round(ground.x / spacing - f32(row & 1) * 0.5));

    // Every node in reach, projected once, so a path can be drawn from both of its ends without
    // working either of them out twice.
    var screen: array<vec3<f32>, 25>;
    for (var index = 0; index < NET_CELLS; index += 1) {
        let cell = vec2<i32>(
            column + index % NET_SIDE - NET_REACH,
            row + index / NET_SIDE - NET_REACH,
        );
        screen[index] = net_project(net_node(cell, spacing, drive));
    }

    let node_size = 0.015 + size() * 0.30;
    let path_width = thickness() * 1.4;
    var lit: NetLight;
    lit.light = 0.0;
    lit.glow = 0.0;

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
            lit.light = max(lit.light, net_segment(p, here.xy, there.xy, width) * net_fade(span));
        }

        // The nodes sit over the paths rather than half buried in them, and the most distant ones
        // are drawn smaller so the far field reads as scatter instead of as a wall of dots.
        let shrink = mix(1.0, 0.55, smoothstep(14.0, 34.0, here.z));
        let radius = max(node_size * shrink * focal / here.z, 0.0022);
        let dot_light = solid(length(p - here.xy) - radius) * net_fade(here.z);
        lit.light = max(lit.light, dot_light);
        lit.glow = max(lit.glow, dot_light);
    }
    return lit;
}

fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let drive = net_beat();
    let focal = net_focal();
    let camera = net_camera();
    let forward = net_forward();
    let up = net_up();
    let extent = net_extent(drive);

    // The ray this fragment looks along, scaled so travelling `t` along it covers `t` metres of
    // distance from the lens -- which makes every distance below directly comparable.
    let ray = forward + vec3<f32>(p.x / focal, 0.0, 0.0) + up * (-p.y / focal);
    if ray.z >= 0.0 && camera.z > extent.y {
        // Looking level or up from above anything the net can reach: it is not in this direction.
        return vec4<f32>(0.0);
    }

    // Walk the ray through the net, and draw wherever it passes through the surface -- going in or
    // coming out, because a wireframe has nothing to stop the eye at the first crossing. Each one
    // is closed in on by bisection, which is what holds up at a grazing angle, where the surface
    // slides along under the ray for metres at a time.
    let limit = NET_FADE_END + 3.0;
    var near = 0.3;
    var start = camera + ray * near;
    var below = start.z < net_height(start.xy, drive);
    var light = 0.0;
    var glow = 0.0;
    var crossings = 0;
    for (var step = 0; step < NET_STEPS && near < limit && crossings < NET_CROSSINGS; step += 1) {
        let far = min(near + min(0.30 + near * 0.05, NET_LONGEST_STEP), limit);
        let point = camera + ray * far;
        // Above the highest the net can stand and still climbing, or under the lowest and still
        // falling: nothing further along this ray can ever be net.
        if (ray.z >= 0.0 && point.z > extent.y) || (ray.z <= 0.0 && point.z < extent.x) {
            break;
        }
        let now_below = point.z < net_height(point.xy, drive);
        if now_below != below {
            var low = near;
            var high = far;
            for (var closing = 0; closing < 4; closing += 1) {
                let middle = (low + high) * 0.5;
                let sample = camera + ray * middle;
                if (sample.z < net_height(sample.xy, drive)) == below {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let distance = (low + high) * 0.5;
            let drawn = net_draw(p, camera + ray * distance, drive, focal);
            // The scene this comes from is lit, not glowing: the near net is bright and distance
            // does the rest of the depth work.
            let lit = 0.45 + 0.55 * (1.0 - smoothstep(2.0, 30.0, distance));
            light = max(light, drawn.light * lit);
            glow = max(glow, drawn.glow);
            crossings += 1;
        }
        below = now_below;
        near = far;
    }

    if light <= 0.0 {
        return vec4<f32>(0.0);
    }

    // A kick warms the net toward the accent colour and it cools again as the kick is let go, so
    // the colour keeps time with the mountains it belongs to.
    let colour = mix(primary(), secondary(), clamp(drive.kick * clamp(reactivity(), 0.0, 2.0), 0.0, 1.0));
    // The nodes ignore the lights in the scene this comes from, so nothing shadows them: here they
    // simply sit a little above the paths they join, and a beat lifts them further.
    let flash = 1.0 + glow * (0.35 + clamp(beat(), 0.0, 1.0) * 0.45);
    // Energy is a root-mean-square of the window, not a normalised level, so a hot input would
    // otherwise blow the net out entirely.
    let level = light * flash * (0.80 + clamp(energy(), 0.0, 1.0) * 0.35);
    return vec4<f32>(colour * level, clamp(level, 0.0, 1.0));
}
