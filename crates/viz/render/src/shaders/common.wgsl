// Shared bindings and helpers. Prepended to every Viz shader by `shaders.rs`.

struct Globals {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    camera_position: vec4<f32>,   // xyz world, w time in seconds
    screen: vec4<f32>,            // width, height, 1/width, 1/height
    params: vec4<f32>,            // exposure, fog density, near, far
    params2: vec4<f32>,           // light count, volumetric steps, ambient, tiles_x
    params3: vec4<f32>,           // plot flag, fog detail, time, laser brightness
    params4: vec4<f32>,           // gobos flag, beam fall-off flag, flat-ink flag, opaque curtain count
    params5: vec4<f32>,           // lamp cloudiness/turbulence, laser cloudiness/turbulence
};

// Quality controls expensive glass/noise work, not a fixture's basic field or physical cuts.
// Draft and Standard retain soft radial optics; High/Ultra retain their established formula.
fn gobos_enabled() -> bool {
    return globals.params4.x > 0.5;
}

fn beam_falloff_enabled() -> bool {
    return globals.params4.y > 0.5;
}

/// Whether this view's colours are already the final ones.
///
/// True for a drawn plan and for the outline view: neither simulates light, so nothing in them
/// should be brightened on the way to the screen.
fn flat_ink() -> bool {
    return globals.params4.z > 0.5;
}

struct Light {
    position_range: vec4<f32>,
    direction_cos_outer: vec4<f32>,
    colour_intensity: vec4<f32>,
    params: vec4<f32>,            // cos_inner, feather, kind, aperture radius
    tangent_frost: vec4<f32>,     // beam right axis, frost
    optics: vec4<f32>,            // gobo slot, gobo rotation, prism facets, prism rotation
    shapers: vec4<f32>,           // blade insertions: +u, -u, +v, -v
    shaper_angles: vec4<f32>,     // per-blade rotation in radians
    shaper_params: vec4<f32>,    // manual edge >0; zero inherits dynamic feather+frost; yzw reserved
    gate: vec4<f32>,              // first artwork layer or -1, gobo wheel count, prism wheel count, curtain header or -1
    shadow: vec4<f32>,            // atlas tile index or -1, tile origin u, v, tile size
    gobo_wheels: array<vec4<f32>, 8>, // slot, rotation, artwork layer or -1, spare
    prism_wheels: array<vec4<f32>, 8>, // facets, rotation, linear flag, spread
};

@group(2) @binding(0) var shadow_atlas: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var<storage, read> shadow_matrices: array<mat4x4<f32>>;
@group(2) @binding(3) var gobo_artwork: texture_2d_array<f32>;
@group(2) @binding(4) var gobo_sampler: sampler;

// Opaque drapes must block every lamp, including those outside the shadow-map budget.
// The centre sheet is finite and follows the posed curtain. Detailed fold shadows still come
// from the atlas, but their quality budget must never make blackout cloth transmit light.
struct CurtainBlocker {
    // Four planes through the lamp and rectangle edges, then the cloth plane facing away
    // from the lamp. Positive half-spaces form the exact finite rectangle's shadow volume.
    planes: array<vec4<f32>, 5>,
};
@group(2) @binding(5) var<storage, read> curtain_blockers: array<CurtainBlocker>;

fn curtain_blocks_segment(light: Light, destination: vec3<f32>) -> bool {
    if (globals.params4.w < 1.0 || light.gate.w < 0.0) {
        return false;
    }
    let first = u32(light.gate.w);
    let count = u32(curtain_blockers[first].planes[4].z);
    let point = vec4<f32>(destination, 1.0);
    for (var offset = 0u; offset < count; offset = offset + 1u) {
        let blocker = curtain_blockers[first + 1u + offset];
        // Check the cloth side first; front-side receivers never need edge tests.
        if (dot(blocker.planes[4], point) <= 0.0) {
            continue;
        }
        if (dot(blocker.planes[0], point) >= 0.0
            && dot(blocker.planes[1], point) >= 0.0
            && dot(blocker.planes[2], point) >= 0.0
            && dot(blocker.planes[3], point) >= 0.0) {
            return true;
        }
    }
    return false;
}

/// How much of `world` this light can actually see, `1.0` being fully lit.
///
/// A light with no map in this frame's budget is not darkened: a missing shadow reads as an
/// unshadowed light, never as a black stage.
fn shadow_lookup(light: Light, world: vec3<f32>, taps: bool) -> f32 {
    if (curtain_blocks_segment(light, world)) {
        return 0.0;
    }
    return shadow_atlas_lookup(light, world, taps);
}

// Haze checks its precomputed curtain mask before calling the atlas-only path.
fn shadow_atlas_lookup(light: Light, world: vec3<f32>, taps: bool) -> f32 {
    let index = light.shadow.x;
    if (index < 0.0) {
        return 1.0;
    }
    let clip = shadow_matrices[u32(index)] * vec4<f32>(world, 1.0);
    if (clip.w <= 0.0) {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    if (ndc.z <= 0.0 || ndc.z >= 1.0) {
        return 1.0;
    }
    let tile = vec2<f32>(light.shadow.y, light.shadow.z);
    let size = light.shadow.w;
    let inside = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (inside.x < 0.0 || inside.x > 1.0 || inside.y < 0.0 || inside.y > 1.0) {
        return 1.0;
    }
    let centre = tile + inside * size;
    if (!taps) {
        // Inside the haze the shadow is already soft, so one tap is indistinguishable from four
        // and the volumetric pass takes this lookup at every step of every beam.
        return textureSampleCompareLevel(shadow_atlas, shadow_sampler, centre, ndc.z);
    }
    // Four taps across the tile soften the edge on a surface without turning it to mush.
    let texel = size / 512.0;
    var lit = 0.0;
    for (var index_x = -1; index_x <= 1; index_x = index_x + 2) {
        for (var index_y = -1; index_y <= 1; index_y = index_y + 2) {
            let offset = vec2<f32>(f32(index_x), f32(index_y)) * texel * 0.5;
            lit += textureSampleCompareLevel(shadow_atlas, shadow_sampler, centre + offset, ndc.z);
        }
    }
    return lit * 0.25;
}

fn shadow_factor(light: Light, world: vec3<f32>) -> f32 {
    return shadow_lookup(light, world, true);
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> tile_counts: array<u32>;
@group(0) @binding(3) var<storage, read> tile_lights: array<u32>;

const TILE_SIZE: u32 = 16u;
const MAX_LIGHTS_PER_TILE: u32 = 256u;
const PI: f32 = 3.14159265359;

fn light_count() -> u32 {
    return u32(globals.params2.x);
}

fn tiles_x() -> u32 {
    return u32(globals.params2.w);
}

fn tile_index_for(position: vec2<f32>) -> u32 {
    let tile = vec2<u32>(position) / TILE_SIZE;
    return tile.y * tiles_x() + tile.x;
}

// The compact tile list is exact until it overflows. Crowded tiles fall back to the full
// light array; silently truncating a parallel append list changes which lamps survive each
// frame, drawing flickering tile-sized squares. Keep ordinary tiles on the compact fast path.
fn tile_light_count(tile: u32) -> u32 {
    if (tile_counts[tile] > MAX_LIGHTS_PER_TILE) {
        return light_count();
    }
    return tile_counts[tile];
}

fn tile_light_index(tile: u32, index: u32) -> u32 {
    if (tile_counts[tile] > MAX_LIGHTS_PER_TILE) {
        return index;
    }
    return tile_lights[tile * MAX_LIGHTS_PER_TILE + index];
}

// Inverse-square falloff normalised to a stage-sized reference throw, so a fixture six metres
// from the floor produces a usable pool instead of a black one.
const REFERENCE_THROW: f32 = 6.0;

fn distance_attenuation(distance: f32) -> f32 {
    let ratio = distance / REFERENCE_THROW;
    return 1.0 / (1.0 + ratio * ratio);
}

// Range is a finite work bound, not a physical iris. Fade energy and its derivative to zero
// before culling removes the light, so the bound cannot draw a hard circle on a receiver.
fn range_envelope(distance: f32, reach: f32) -> f32 {
    return 1.0 - smoothstep(reach * 0.8, reach, distance);
}

// Beam profile across the cone: flat through the inner cone, feathered to the field edge, with a
// brighter core so a narrow beam reads as a beam rather than a flat wedge.
/// The artwork on one piece of glass, sampled in the beam's own disc.
///
/// The gate runs `-1..=1` across the field, and the image covers exactly that square, so a gobo
/// projects at the size the fixture's own optics give it. Outside the disc the glass is opaque:
/// the pattern ends where the field does rather than tiling into the dark.
fn gobo_artwork_transmission(layer: i32, position: vec2<f32>) -> f32 {
    let uv = position * 0.5 + vec2<f32>(0.5, 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 0.0;
    }
    return textureSampleLevel(gobo_artwork, gobo_sampler, uv, layer, 0.0).r;
}

/// One drawn gobo pattern, evaluated in the beam's own disc.
///
/// `position` is the point in the gate, normalised so the field edge is at radius one, already
/// rotated by the wheel. These are what a profile that declares no wheel of its own projects: a
/// pattern of the right character in the right slot, turning at the right rate, rather than any
/// particular glass. A profile that carries artwork uses that instead.
fn gobo_transmission(slot: u32, position: vec2<f32>) -> f32 {
    if (slot == 0u) {
        return 1.0;
    }
    let radius = length(position);
    let angle = atan2(position.y, position.x);
    switch slot {
        // Breakup: overlapping blobs, the most-used pattern on any rig.
        case 1u: {
            let cell = sin(position.x * 9.0) * sin(position.y * 7.3)
                + 0.6 * sin(position.x * 17.0 + 1.7) * sin(position.y * 13.0 - 0.9);
            return smoothstep(-0.25, 0.35, cell);
        }
        // Dots.
        case 2u: {
            let grid = fract(position * 3.0) - vec2<f32>(0.5, 0.5);
            return 1.0 - smoothstep(0.16, 0.26, length(grid));
        }
        // Radial spokes.
        case 3u: {
            return smoothstep(0.1, 0.45, abs(sin(angle * 6.0)));
        }
        // Concentric rings.
        case 4u: {
            return smoothstep(0.15, 0.4, abs(sin(radius * 14.0)));
        }
        // A single hard-edged triangle, for a shaped projection.
        case 5u: {
            let folded = abs(position);
            return 1.0 - smoothstep(0.42, 0.5, folded.x + folded.y * 0.8);
        }
        // Bars.
        case 6u: {
            return smoothstep(0.2, 0.5, abs(sin(position.y * 8.0)));
        }
        // A tight iris-like centre spot.
        default: {
            return 1.0 - smoothstep(0.35, 0.5, radius);
        }
    }
}

// Explicit installation endpoints must not retain a penumbra at the open/opposite field edge.
// Keep the legacy dynamic-DMX interpolation unchanged between its existing endpoints.
fn shaper_plane_transmission(blade: f32, position: f32, edge: f32, manual: bool) -> f32 {
    if (manual) {
        if (blade <= 0.0) { return 1.0; }
        if (blade >= 2.0) { return 0.0; }
    }
    return smoothstep(1.0 - blade + edge, 1.0 - blade - edge, position);
}

/// The framing shutters, as four straight cuts across the gate.
fn shaper_transmission(light: Light, position: vec2<f32>, softness: f32) -> f32 {
    let blades = light.shapers;
    if (blades.x + blades.y + blades.z + blades.w <= 0.001) {
        return 1.0;
    }
    // Legacy live insertion 1 reaches centre; installed full-stroke insertion 2 crosses it.
    // Manual barn-door penumbra is independent of a Fresnel's broad radial feather/frost.
    let edge = select(max(softness, 0.02), light.shaper_params.x, light.shaper_params.x > 0.0);
    let rotated = array<vec2<f32>, 4>(
        vec2<f32>(
            position.x * cos(light.shaper_angles.x) - position.y * sin(light.shaper_angles.x),
            position.x * sin(light.shaper_angles.x) + position.y * cos(light.shaper_angles.x),
        ),
        vec2<f32>(
            position.x * cos(light.shaper_angles.y) - position.y * sin(light.shaper_angles.y),
            position.x * sin(light.shaper_angles.y) + position.y * cos(light.shaper_angles.y),
        ),
        vec2<f32>(
            position.x * cos(light.shaper_angles.z) - position.y * sin(light.shaper_angles.z),
            position.x * sin(light.shaper_angles.z) + position.y * cos(light.shaper_angles.z),
        ),
        vec2<f32>(
            position.x * cos(light.shaper_angles.w) - position.y * sin(light.shaper_angles.w),
            position.x * sin(light.shaper_angles.w) + position.y * cos(light.shaper_angles.w),
        ),
    );
    let manual = light.shaper_params.x > 0.0;
    var transmission = 1.0;
    transmission *= shaper_plane_transmission(blades.x, rotated[0].x, edge, manual);
    transmission *= shaper_plane_transmission(blades.y, -rotated[1].x, edge, manual);
    transmission *= shaper_plane_transmission(blades.z, rotated[2].y, edge, manual);
    transmission *= shaper_plane_transmission(blades.w, -rotated[3].y, edge, manual);
    return transmission;
}

// Basic optics are shared by surfaces, marched shafts and Draft's bounded field evaluation.
// This is exactly the established High/Ultra radial formula, without a quality-tier bypass.
fn beam_radial_profile(light: Light, to_light: vec3<f32>) -> f32 {
    return radial_profile_with_math(light, to_light, false);
}

fn radial_profile_with_math(light: Light, to_light: vec3<f32>, draft_surface: bool) -> f32 {
    let axis = light.direction_cos_outer.xyz;
    let cos_angle = min(dot(-to_light, axis), 1.0);
    let cos_outer = light.direction_cos_outer.w;
    let span = 1.0 - cos_outer;
    if (span <= 0.0 || cos_angle <= cos_outer) {
        return 0.0;
    }
    // A fixed cosine epsilon can put the inner edge beyond the physically reachable axis
    // of a narrow cone. Bound the guard by its available span, preserving authored sharpness.
    // Work with widths: adding a tiny guard to cos_outer can round both smoothstep endpoints
    // to the same f32, whereas the positive span-relative width remains representable.
    let transition = min(max(light.params.x - cos_outer, min(0.0005, span * 0.01)), span);
    let edge_position = clamp((cos_angle - cos_outer) / transition, 0.0, 1.0);
    let edge = edge_position * edge_position * (3.0 - 2.0 * edge_position);
    let radial = clamp((1.0 - cos_angle) / span, 0.0, 1.0);
    // Uniformity is how the light inside that field is distributed, which is a separate question
    // from how the field ends. A good LED wash has no rim at all and is still flat across the
    // middle; a PAR is hot in the centre and has faded long before its own rim. `1` is flat, `0`
    // is a bright centre falling away to a dim edge.
    let uniformity = clamp(light.params.z, 0.0, 1.0);
    var hot_spot: f32;
    if (draft_surface) {
        let centre = 1.0 - radial;
        hot_spot = 0.35 + 2.4 * centre * centre * sqrt(centre);
    } else {
        hot_spot = 0.35 + 2.4 * pow(1.0 - radial, 2.5);
    }
    let core = mix(hot_spot, 1.0, uniformity);
    return edge * core;
}

fn beam_profile(light: Light, to_light: vec3<f32>) -> f32 {
    return profile_with_math(light, to_light, false);
}

// Only Draft opaque surfaces select the algebraic hotspot; all other callers retain pow.
fn draft_surface_beam_profile(light: Light, to_light: vec3<f32>) -> f32 {
    return profile_with_math(light, to_light, true);
}

fn profile_with_math(light: Light, to_light: vec3<f32>, draft_surface: bool) -> f32 {
    var profile = radial_profile_with_math(light, to_light, draft_surface);
    if (profile <= 0.0) { return 0.0; }
    let axis = light.direction_cos_outer.xyz;
    let cos_outer = light.direction_cos_outer.w;
    let gobo_count = select(0u, min(u32(light.gate.y), 8u), gobos_enabled());
    let prism_count = min(u32(light.gate.z), 8u);
    // A physical barn door or shutter still cuts the field when gobo artwork is disabled.
    let shaped = light.shapers.x + light.shapers.y + light.shapers.z + light.shapers.w;
    var optics_active = shaped > 0.001;
    for (var wheel = 0u; wheel < gobo_count; wheel += 1u) {
        optics_active = optics_active || light.gobo_wheels[wheel].x > 0.0;
    }
    for (var wheel = 0u; wheel < prism_count; wheel += 1u) {
        optics_active = optics_active || light.prism_wheels[wheel].x >= 2.0;
    }
    if (!optics_active) { return profile; }

    // Everything a head puts in front of its lamp lives in the gate: a flat disc across the
    // beam, turning with the head. `gate` is a point on that disc with the field edge at one.
    let ray = -to_light;
    let along = max(dot(ray, axis), 1e-4);
    let across = ray - axis * along;
    let tangent = light.tangent_frost.xyz;
    let bitangent = cross(axis, tangent);
    let scale = max(sqrt(max(1.0 - cos_outer * cos_outer, 1e-6)) / max(cos_outer, 1e-3), 1e-4);
    var gate = vec2<f32>(dot(across, tangent), dot(across, bitangent)) / (along * scale);

    // Each authored prism is a separate gate transform. Copies carry the combined gobos;
    // wheel rotation belongs to its prism rather than rotating every optic together.
    var prism_aperture = 1.0;
    for (var wheel = 0u; wheel < prism_count; wheel += 1u) {
        let prism = light.prism_wheels[wheel];
        if (prism.x < 2.0) { continue; }
        let angle = prism.y;
        let rotated = vec2<f32>(gate.x * cos(angle) - gate.y * sin(angle),
            gate.x * sin(angle) + gate.y * cos(angle));
        var copy: vec2<f32>;
        if (prism.z > 0.5) {
            // Linear copies evenly spaced along the rotating prism's own axis.
            let spacing = max(2.0 * prism.w / max(prism.x - 1.0, 1.0), 0.0001);
            let index = clamp(round(rotated.x / spacing + (prism.x - 1.0) * 0.5), 0.0, prism.x - 1.0);
            let centre = (index - (prism.x - 1.0) * 0.5) * spacing;
            let copy_radius = clamp(spacing * 0.42, 0.025, 0.35);
            copy = vec2<f32>(rotated.x - centre, rotated.y) / copy_radius;
        } else {
            let wedge = 2.0 * PI / prism.x;
            let heading = atan2(rotated.y, rotated.x);
            let folded = (heading - floor(heading / wedge) * wedge) - wedge * 0.5;
            let copy_radius = clamp(prism.w * sin(PI / prism.x) * 0.85, 0.025, 0.35);
            copy = (vec2<f32>(cos(folded), sin(folded)) * length(rotated)
                - vec2<f32>(prism.w, 0.0)) / copy_radius;
        }
        // Return to the common gate axes: gobo indexing is independent of prism rotation.
        gate = vec2<f32>(copy.x * cos(angle) + copy.y * sin(angle),
            -copy.x * sin(angle) + copy.y * cos(angle));
        prism_aperture *= 1.0 - smoothstep(0.82, 1.0, length(copy));
    }

    let frost = light.tangent_frost.w;
    var transmission = 1.0;
    for (var wheel = 0u; wheel < gobo_count; wheel += 1u) {
        let gobo = light.gobo_wheels[wheel];
        let slot = u32(gobo.x);
        if (slot == 0u) { continue; }
        let turn = gobo.y;
        let rotated = vec2<f32>(gate.x * cos(turn) - gate.y * sin(turn),
            gate.x * sin(turn) + gate.y * cos(turn));
        let layer = i32(gobo.z);
        var pattern: f32;
        if (layer >= 0) {
            pattern = gobo_artwork_transmission(layer, rotated);
        } else {
            pattern = gobo_transmission(slot, rotated);
        }
        // Wheels in series mask one another. An open wheel passes the preceding pattern.
        transmission *= pattern;
    }
    profile *= mix(transmission, 1.0, clamp(frost * 1.4, 0.0, 0.95));
    if (shaped > 0.001) {
        profile *= shaper_transmission(light, gate, light.params.y + frost);
    }
    // Frost fills the gaps between a prism's copies, exactly as it softens everything else in
    // the gate: a diffuser in front of a prism blurs the copies back into one field.
    return profile * mix(prism_aperture, 1.0, clamp(frost * 1.4, 0.0, 0.95));
}

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denominator = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denominator * denominator, 1e-5);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let gv = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let gl = n_dot_l / (n_dot_l * (1.0 - k) + k);
    return gv * gl;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
