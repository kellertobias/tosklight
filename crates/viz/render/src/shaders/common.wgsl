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
    params4: vec4<f32>,           // gobos flag, beam fall-off flag, flat-ink flag, spare
};

// What the quality tier has paid for. The tiers are a ladder of what is in the beam: Draft draws
// the cone, Standard puts the glass in it, High gives it its fall-off, Ultra makes the haze itself
// uneven. Asking here rather than branching on a tier number keeps the tiers a renderer concern.
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
    gate: vec4<f32>,              // gobo artwork layer or -1, spare
    shadow: vec4<f32>,            // atlas tile index or -1, tile origin u, v, tile size
};

@group(2) @binding(0) var shadow_atlas: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var<storage, read> shadow_matrices: array<mat4x4<f32>>;
@group(2) @binding(3) var gobo_artwork: texture_2d_array<f32>;
@group(2) @binding(4) var gobo_sampler: sampler;

/// How much of `world` this light can actually see, `1.0` being fully lit.
///
/// A light with no map in this frame's budget is not darkened: a missing shadow reads as an
/// unshadowed light, never as a black stage.
fn shadow_lookup(light: Light, world: vec3<f32>, taps: bool) -> f32 {
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
const MAX_LIGHTS_PER_TILE: u32 = 96u;
const PI: f32 = 3.14159265359;
/// How far each prism copy sits from the beam axis, in gate radii.
const PRISM_SPREAD: f32 = 0.62;
/// The radius each copy occupies once it is out there. Copies just touching at the default
/// spread is what a prism looks like at its most useful.
const PRISM_COPY_RADIUS: f32 = 0.55;

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

// Inverse-square falloff normalised to a stage-sized reference throw, so a fixture six metres
// from the floor produces a usable pool instead of a black one.
const REFERENCE_THROW: f32 = 6.0;

fn distance_attenuation(distance: f32) -> f32 {
    let ratio = distance / REFERENCE_THROW;
    return 1.0 / (1.0 + ratio * ratio);
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

/// The framing shutters, as four straight cuts across the gate.
fn shaper_transmission(light: Light, position: vec2<f32>, softness: f32) -> f32 {
    let blades = light.shapers;
    if (blades.x + blades.y + blades.z + blades.w <= 0.001) {
        return 1.0;
    }
    // A blade at `0` is fully open and at `1` has crossed the whole gate.
    let edge = max(softness, 0.02);
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
    var transmission = 1.0;
    transmission *= smoothstep(1.0 - blades.x + edge, 1.0 - blades.x - edge, rotated[0].x);
    transmission *= smoothstep(1.0 - blades.y + edge, 1.0 - blades.y - edge, -rotated[1].x);
    transmission *= smoothstep(1.0 - blades.z + edge, 1.0 - blades.z - edge, rotated[2].y);
    transmission *= smoothstep(1.0 - blades.w + edge, 1.0 - blades.w - edge, -rotated[3].y);
    return transmission;
}

fn beam_profile(light: Light, to_light: vec3<f32>) -> f32 {
    let axis = light.direction_cos_outer.xyz;
    let cos_angle = dot(-to_light, axis);
    let cos_outer = light.direction_cos_outer.w;
    if (cos_angle <= cos_outer) {
        return 0.0;
    }
    let cos_inner = max(light.params.x, cos_outer + 0.0005);
    // Sharpness is the width of this transition: a profile cuts within a fraction of a degree,
    // a wash blends across its whole field.
    let edge = smoothstep(cos_outer, cos_inner, cos_angle);
    let radial = clamp((1.0 - cos_angle) / max(1.0 - cos_outer, 1e-5), 0.0, 1.0);
    // Uniformity is how the light inside that field is distributed, which is a separate question
    // from how the field ends. A good LED wash has no rim at all and is still flat across the
    // middle; a PAR is hot in the centre and has faded long before its own rim. `1` is flat, `0`
    // is a bright centre falling away to a dim edge.
    let uniformity = clamp(light.params.z, 0.0, 1.0);
    let hot_spot = 0.35 + 2.4 * pow(1.0 - radial, 2.5);
    let core = mix(hot_spot, 1.0, uniformity);
    // Fall-off is the field ending softly and the light dropping away across the pool. Without it
    // the cone is evenly filled and cut square at its rim, which reads perfectly clearly and is
    // what the cheap tiers draw.
    //
    // Anything past the field edge has already returned zero above, so without fall-off the cone
    // is simply full to its own rim.
    var profile = 1.0;
    if (beam_falloff_enabled()) {
        profile = edge * core;
    }

    let gobo_slot = select(0u, u32(light.optics.x), gobos_enabled());
    let facets = light.optics.z;
    let shaped = select(0.0, light.shapers.x + light.shapers.y + light.shapers.z + light.shapers.w, gobos_enabled());
    if (gobo_slot == 0u && facets < 2.0 && shaped <= 0.001) {
        return profile;
    }

    // Everything a head puts in front of its lamp lives in the gate: a flat disc across the
    // beam, turning with the head. `gate` is a point on that disc with the field edge at one.
    let ray = -to_light;
    let along = max(dot(ray, axis), 1e-4);
    let across = ray - axis * along;
    let tangent = light.tangent_frost.xyz;
    let bitangent = cross(axis, tangent);
    let scale = max(sqrt(max(1.0 - cos_outer * cos_outer, 1e-6)) / max(cos_outer, 1e-3), 1e-4);
    var gate = vec2<f32>(dot(across, tangent), dot(across, bitangent)) / (along * scale);

    // A prism deviates the beam into one copy per facet, arranged around the axis. Folding the
    // gate into a single wedge draws every copy from one evaluation, which is what keeps this
    // affordable inside the march; recentring on the wedge's own axis is what separates the
    // copies instead of stacking them.
    var prism_aperture = 1.0;
    if (facets >= 2.0) {
        let wedge = 2.0 * PI / facets;
        let angle = atan2(gate.y, gate.x) + light.optics.w;
        let folded = (angle - floor(angle / wedge) * wedge) - wedge * 0.5;
        let radius = length(gate);
        let copy = vec2<f32>(cos(folded), sin(folded)) * radius;
        // Each facet's copy sits out along its own axis and is magnified to fill the field it
        // now occupies, so a prism spreads the beam rather than shrinking what is in it.
        gate = (copy - vec2<f32>(PRISM_SPREAD, 0.0)) / PRISM_COPY_RADIUS;
        // A facet passes nothing outside its own copy. That gap is what makes a prism read as
        // several beams rather than one — with a gobo in the gate and without one.
        prism_aperture = 1.0 - smoothstep(0.82, 1.0, length(gate));
    }

    let frost = light.tangent_frost.w;
    if (gobo_slot > 0u) {
        let turn = light.optics.y;
        let rotated = vec2<f32>(
            gate.x * cos(turn) - gate.y * sin(turn),
            gate.x * sin(turn) + gate.y * cos(turn),
        );
        // Frost is a diffuser: it does not remove the gobo, it stops it holding an edge.
        let layer = i32(light.gate.x);
        var pattern = 0.0;
        if (layer >= 0) {
            pattern = gobo_artwork_transmission(layer, rotated);
        } else {
            pattern = gobo_transmission(gobo_slot, rotated);
        }
        profile *= mix(pattern, 1.0, clamp(frost * 1.4, 0.0, 0.95));
    }
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
