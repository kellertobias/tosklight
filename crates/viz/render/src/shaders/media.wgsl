struct MediaInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(7) crop: vec4<f32>,
    @location(8) material: vec4<f32>,
};

struct MediaOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) material: vec4<f32>,
    @location(2) world: vec3<f32>,
    @location(3) panel_uv: vec2<f32>,
    @location(4) normal: vec3<f32>,
};

@group(1) @binding(0) var media_atlas: texture_2d_array<f32>;
@group(1) @binding(1) var media_sampler: sampler;

@vertex
fn media_vertex(input: MediaInput, @builtin(instance_index) instance: u32) -> MediaOutput {
    let model = mat4x4<f32>(input.model0, input.model1, input.model2, input.model3);
    let world = model * vec4<f32>(input.position, 1.0);
    var output: MediaOutput;
    output.clip = globals.view_projection * world;
    output.uv = input.crop.xy + input.uv * input.crop.zw;
    output.material = input.material;
    output.world = world.xyz;
    output.panel_uv = input.uv;
    output.normal = normalize((model * vec4<f32>(input.normal, 0.0)).xyz);
    return output;
}

@fragment
fn media_fragment(input: MediaOutput) -> @location(0) vec4<f32> {
    let layer = i32(input.material.x);
    var colour = textureSample(media_atlas, media_sampler, input.uv, layer).rgb;
    let kind = input.material.y;
    let gain = input.material.z;
    let feather = input.material.w;
    if kind < 0.5 {
        let edge = min(min(input.panel_uv.x, 1.0 - input.panel_uv.x), min(input.panel_uv.y, 1.0 - input.panel_uv.y));
        colour *= smoothstep(0.0, max(feather, 0.0001), edge);
        var reflected = colour * globals.params2.z;
        let tile = tile_index_for(input.clip.xy);
        let count = min(tile_counts[tile], MAX_LIGHTS_PER_TILE);
        for (var index: u32 = 0u; index < count; index = index + 1u) {
            let light = lights[tile_lights[tile * MAX_LIGHTS_PER_TILE + index]];
            // A linked projector derives its cone from this surface; it never paints or
            // shadow-masks the image back onto that surface.
            if (light.params.w > 0.5) {
                continue;
            }
            let offset = light.position_range.xyz - input.world;
            let distance = length(offset);
            if (distance > light.position_range.w) {
                continue;
            }
            let to_light = offset / max(distance, 0.0001);
            var cone = beam_profile(light, to_light);
            cone *= shadow_factor(light, input.world);
            let incidence = max(dot(normalize(input.normal), to_light), 0.0);
            reflected += colour * light.colour_intensity.rgb * incidence * cone
                * distance_attenuation(distance) * 3.8;
        }
        colour = reflected * gain;
    } else if kind < 1.5 {
        colour *= gain;
    } else {
        let pitch_frequency = clamp(1000.0 / max(feather, 1.0), 32.0, 300.0);
        let cell = fract(input.uv * pitch_frequency);
        let diode = 1.0 - smoothstep(0.16, 0.42, length(cell - vec2<f32>(0.5)));
        colour *= gain * (0.45 + 0.55 * diode);
    }
    return vec4<f32>(colour, 1.0);
}
