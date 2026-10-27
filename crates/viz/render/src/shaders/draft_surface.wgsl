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
    if (input.surface.y > 0.5) { discard; }
    let normal = normalize(input.world_normal);
    let albedo = input.base_colour.rgb;
    let ambient = globals.params2.z;
    let key = normalize(vec3<f32>(-0.6, 0.8, 0.5));
    let fill = normalize(vec3<f32>(0.8, 0.35, -0.6));
    let room = ambient * (mix(0.12, 0.38, normal.y * 0.5 + 0.5)
        + 0.95 * max(dot(normal, key), 0.0) + 0.20 * max(dot(normal, fill), 0.0));
    var radiance = input.emissive.rgb + albedo * room;
    let tile = tile_index_for(input.clip_position.xy);
    let count = tile_light_count(tile);
    for (var index = 0u; index < count; index++) {
        let light = lights[tile_light_index(tile, index)];
        let offset = light.position_range.xyz - input.world_position;
        let distance_squared = dot(offset, offset);
        if (distance_squared > light.position_range.w * light.position_range.w) { continue; }
        // Positive distance normalization cannot change the diffuse dot's sign.
        if (dot(normal, offset) <= 0.0) { continue; }
        let distance = sqrt(distance_squared);
        let to_light = offset / max(distance, 1e-4);
        let diffuse = max(dot(normal, to_light), 0.0);
        if (diffuse <= 0.0) { continue; }
        var cone = draft_surface_beam_profile(light, to_light);
        if (cone <= 0.0) { continue; }
        // Opaque meshes still cast cached atlas shadows; current curtains remain exact even
        // for lights beyond the two-map budget. Draft removes material BRDF/noise, not blockers.
        cone *= shadow_lookup(light, input.world_position, false);
        let attenuation = cone * distance_attenuation(distance) * range_envelope(distance, light.position_range.w);
        radiance += albedo / PI * light.colour_intensity.rgb * diffuse * attenuation * LIGHT_POWER;
    }
    return vec4<f32>(radiance, 1.0);
}
