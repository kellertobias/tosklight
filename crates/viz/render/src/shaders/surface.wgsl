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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) base_colour: vec4<f32>,
    @location(3) emissive: vec4<f32>,
};

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
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.world_normal);
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let roughness = clamp(input.base_colour.w, 0.045, 1.0);
    let metallic = clamp(input.emissive.w, 0.0, 1.0);
    let albedo = input.base_colour.rgb;
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let n_dot_v = max(dot(normal, view_direction), 1e-4);

    var radiance = input.emissive.rgb;
    radiance += albedo * globals.params2.z;

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
        let attenuation = cone * distance_attenuation(distance);
        radiance += (diffuse + specular) * light.colour_intensity.rgb * n_dot_l * attenuation * LIGHT_POWER;
    }
    return vec4<f32>(radiance, 1.0);
}
