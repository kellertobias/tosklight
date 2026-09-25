// Opaque scenery, fixture bodies, and emissive apertures.
//
// `LIGHT_POWER` converts a normalised fixture intensity into scene radiance before tonemapping.
//
// Surfaces are lit by the tile light list, so a beam is visible where it lands even with clear
// air. Volumetric shafts come from `beam.wgsl` and require atmosphere.

const LIGHT_POWER: f32 = 12.0;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(7) normal0: vec4<f32>,
    @location(8) normal1: vec4<f32>,
    @location(9) normal2: vec4<f32>,
    @location(10) base_colour: vec4<f32>,
    @location(11) emissive: vec4<f32>,
    @location(12) surface: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) base_colour: vec4<f32>,
    @location(3) emissive: vec4<f32>,
    // The finish is drawn in the instance's own space, scaled to metres, so it stays on the
    // object as it moves and its grain has a physical size whatever the mesh was stretched to.
    @location(4) local_position: vec3<f32>,
    @location(5) local_normal: vec3<f32>,
    @location(6) @interpolate(flat) local_scale: vec3<f32>,
    @location(7) @interpolate(flat) surface: vec4<f32>,
};

// The finishes, matching `Surface` on the Rust side.
const SURFACE_PLAIN: i32 = 0;
const SURFACE_ALUMINIUM: i32 = 1;
const SURFACE_MULTIPLEX: i32 = 2;
const SURFACE_FABRIC: i32 = 3;

// Value noise on a lattice, from an integer hash: enough for grain, and it needs no texture.
fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn value_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(cell), hash3(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash3(cell + vec3<f32>(0.0, 1.0, 0.0)), hash3(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash3(cell + vec3<f32>(0.0, 0.0, 1.0)), hash3(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash3(cell + vec3<f32>(0.0, 1.0, 1.0)), hash3(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    return value_noise(p) * 0.5 + value_noise(p * 2.03 + 17.1) * 0.3 + value_noise(p * 4.11 + 3.7) * 0.2;
}

// How much of a pattern `cycles_per_metre` fine survives at this pixel, given how many metres
// of the surface one pixel covers along it: a grain finer than the pixel is faded out rather than
// left to shimmer.
fn pattern_visibility(cycles_per_metre: f32, metres_per_pixel: f32) -> f32 {
    let pixels_per_cycle = 1.0 / max(cycles_per_metre * metres_per_pixel, 1e-6);
    return smoothstep(2.0, 6.0, pixels_per_cycle);
}

struct Finish {
    albedo: vec3<f32>,
    roughness: f32,
    normal: vec3<f32>,
    // A sheen the base colour does not control: the nap of fabric catching grazing light.
    sheen: f32,
    // 1.0 for a surface whose shape is what an operator reads it by, like a gathered drape.
    folded: f32,
};

// The finish of this pixel: colour, roughness and normal after the surface's own structure.
fn finish(input: VertexOutput, normal: vec3<f32>) -> Finish {
    var out: Finish;
    out.albedo = input.base_colour.rgb;
    out.roughness = clamp(input.base_colour.w, 0.045, 1.0);
    out.normal = normal;
    out.sheen = 0.0;
    out.folded = 0.0;
    let material = i32(input.surface.x + 0.5);
    if (material == SURFACE_PLAIN) {
        return out;
    }
    // Physical position on the object, in metres from its centre, and how much of it one pixel
    // spans. Screen derivatives are taken here, before any branch, so they stay well defined.
    let metres = input.local_position * input.local_scale;
    let footprint = fwidth(metres);
    if (material == SURFACE_ALUMINIUM) {
        // Extruded tube: the grain runs along the local `Y` the unit cylinder stands on, so
        // the streaks are long along the tube and change round it. A cube coupler receiver or
        // a plate reads the same rule as brushed along its length.
        let around = atan2(input.local_position.z, input.local_position.x) * 12.0;
        let streak = value_noise(vec3<f32>(around, metres.y * 6.0, around * 0.37));
        let fine = value_noise(vec3<f32>(around * 6.0, metres.y * 90.0, 0.0));
        let grain = fbm(metres * 250.0);
        let visible = pattern_visibility(90.0, footprint.y);
        let tone = 0.86 + streak * 0.24 + (fine - 0.5) * 0.10 * visible + (grain - 0.5) * 0.08;
        out.albedo = out.albedo * tone;
        out.roughness = clamp(out.roughness + (streak - 0.5) * 0.18 + (grain - 0.5) * 0.12, 0.35, 0.95);
        return out;
    }
    if (material == SURFACE_MULTIPLEX) {
        let on_face = abs(input.local_normal.y) > 0.5;
        if (on_face) {
            // The phenolic anti-slip face: a wire-mesh imprint pressed into the film, over a
            // mottle that keeps a large deck from reading as one flat sheet.
            let pitch = 0.0125;
            let cell = fract(metres.xz / pitch);
            let line = min(min(cell.x, 1.0 - cell.x), min(cell.y, 1.0 - cell.y));
            let visible = pattern_visibility(1.0 / pitch, max(footprint.x, footprint.z));
            let mesh = (1.0 - smoothstep(0.0, 0.14, line)) * visible;
            let mottle = fbm(metres * 9.0);
            let wear = value_noise(metres * 60.0);
            out.albedo = out.albedo * (0.78 + mottle * 0.40 + wear * 0.10) * (1.0 - mesh * 0.35);
            out.roughness = clamp(out.roughness + mesh * 0.12 + (mottle - 0.5) * 0.2, 0.5, 1.0);
            // The mottle is slightly raised, so light catches the face unevenly: its slope is
            // read from the noise itself, along the deck's own axes, and tilts the normal.
            let step = 0.004;
            let slope_x = fbm((metres + vec3<f32>(step, 0.0, 0.0)) * 9.0) - mottle;
            let slope_z = fbm((metres + vec3<f32>(0.0, 0.0, step)) * 9.0) - mottle;
            let across = normalize(cross(normal, select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normal.x) > 0.7)));
            let along = cross(across, normal);
            out.normal = normalize(normal - (along * slope_x + across * slope_z) * 3.0);
        } else {
            // An edge shows the plies: light and dark veneers stacked through the thickness,
            // 1.5 mm each in birch multiplex, with a little wander along the edge.
            let thickness = 0.0015;
            let along = metres.x + metres.z;
            let ply = fract(metres.y / thickness + value_noise(vec3<f32>(along * 30.0, 0.0, 0.0)) * 0.4);
            let visible = pattern_visibility(1.0 / thickness, footprint.y);
            let veneer = mix(0.5, smoothstep(0.35, 0.5, ply) - smoothstep(0.85, 1.0, ply), visible);
            // Raw birch edge: much paler than the film, and yellower.
            let birch = vec3<f32>(0.42, 0.30, 0.16);
            let glue = birch * 0.45;
            out.albedo = mix(glue, birch, veneer) * (0.8 + fbm(metres * 120.0) * 0.4);
            out.roughness = 0.9;
        }
        return out;
    }
    if (material == SURFACE_FABRIC) {
        // Serge: a wool nap with a fine weave under it, and the fold's own hang running
        // down the drop. The base colour is the dyed cloth; the nap is what gives it depth.
        let nap = fbm(metres * 35.0);
        let weave = value_noise(vec3<f32>(metres.x * 900.0, metres.y * 900.0, metres.z * 300.0));
        let hang = value_noise(vec3<f32>(metres.x * 18.0 + metres.z * 6.0, metres.y * 0.9, 0.0));
        let visible = pattern_visibility(900.0, max(footprint.x, footprint.y));
        let tone = 0.9 + nap * 0.14 + (weave - 0.5) * 0.08 * visible + hang * 0.06;
        out.albedo = out.albedo * tone;
        out.roughness = clamp(out.roughness - 0.08 + (nap - 0.5) * 0.1, 0.6, 1.0);
        // The nap catches light along the grain of the folds: the soft grazing sheen velvet has.
        // A few percent at most: black serge stays black except right on a fold's edge.
        out.sheen = 0.03 + nap * 0.015;
        out.folded = 1.0;
        return out;
    }
    return out;
}

// Fresnel that a rough surface loses at grazing angles, for ambient light that has no half vector.
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model0, input.model1, input.model2, input.model3);
    let normal_matrix = mat3x3<f32>(input.normal0.xyz, input.normal1.xyz, input.normal2.xyz);
    let world = model * vec4<f32>(input.position, 1.0);
    var output: VertexOutput;
    output.clip_position = globals.view_projection * world;
    output.world_position = world.xyz;
    output.world_normal = normalize(normal_matrix * input.normal);
    output.base_colour = input.base_colour;
    output.emissive = input.emissive;
    output.local_position = input.position;
    output.local_normal = input.normal;
    output.local_scale = vec3<f32>(length(input.model0.xyz), length(input.model1.xyz), length(input.model2.xyz));
    output.surface = input.surface;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let surface = finish(input, normalize(input.world_normal));
    let normal = surface.normal;
    let roughness = surface.roughness;
    let metallic = clamp(input.emissive.w, 0.0, 1.0);
    let albedo = surface.albedo;
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let n_dot_v = max(dot(normal, view_direction), 1e-4);

    var radiance = input.emissive.rgb;
    // The room's own light is not flat: a venue is lit from above, the floor throws less back
    // than the roof lets down, and what faces the operator is seen a little brighter than what
    // turns away. Without that every solid is one tone and a truss is a silhouette. Metal takes
    // it as a reflection rather than as a diffuse tint, which is what makes it read as metal.
    let ambient = globals.params2.z;
    if (flat_ink()) {
        radiance += albedo * ambient;
    } else {
        let hemisphere = mix(0.55, 1.2, normal.y * 0.5 + 0.5);
        // Cloth is seen by its folds, which face every way, so it takes the facing term twice.
        let facing = mix(0.7 + 0.3 * n_dot_v, 0.25 + 0.75 * n_dot_v, surface.folded);
        let room = ambient * hemisphere * facing;
        let ambient_fresnel = fresnel_schlick_roughness(n_dot_v, f0, roughness);
        radiance += albedo * (1.0 - metallic) * room;
        radiance += ambient_fresnel * room * (1.0 - roughness * 0.6);
        radiance += vec3<f32>(surface.sheen) * pow(1.0 - n_dot_v, 4.0) * ambient * 2.0;
    }

    let tile = tile_index_for(input.clip_position.xy);
    let count = min(tile_counts[tile], MAX_LIGHTS_PER_TILE);
    for (var index: u32 = 0u; index < count; index = index + 1u) {
        let light = lights[tile_lights[tile * MAX_LIGHTS_PER_TILE + index]];
        let offset = light.position_range.xyz - input.world_position;
        let distance = length(offset);
        if (distance > light.position_range.w) {
            continue;
        }
        let to_light = offset / max(distance, 1e-4);
        var cone = beam_profile(light, to_light);
        if (cone <= 0.0) {
            continue;
        }
        // What the light cannot see, it cannot light.
        cone *= shadow_factor(light, input.world_position);
        if (cone <= 0.0) {
            continue;
        }
        let n_dot_l = max(dot(normal, to_light), 0.0);
        if (n_dot_l <= 0.0) {
            continue;
        }
        let half_vector = normalize(to_light + view_direction);
        let n_dot_h = max(dot(normal, half_vector), 0.0);
        let specular_distribution = distribution_ggx(n_dot_h, roughness);
        let geometry = geometry_smith(n_dot_v, n_dot_l, roughness);
        let fresnel = fresnel_schlick(max(dot(half_vector, view_direction), 0.0), f0);
        let specular = specular_distribution * geometry * fresnel / max(4.0 * n_dot_v * n_dot_l, 1e-4);
        let diffuse = (vec3<f32>(1.0) - fresnel) * (1.0 - metallic) * albedo / PI;
        let sheen = vec3<f32>(surface.sheen) * pow(1.0 - max(dot(normal, half_vector), 0.0), 3.0) / PI;
        let attenuation = cone * distance_attenuation(distance);
        radiance += (diffuse + specular + sheen) * light.colour_intensity.rgb * n_dot_l * attenuation * LIGHT_POWER;
    }
    return vec4<f32>(radiance, 1.0);
}
