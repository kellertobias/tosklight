// Bloom extraction, separable blur, and the tonemapped composite.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: vec4<f32>; // exposure, threshold, direction x, direction y

struct FullscreenOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOutput {
    // One oversized triangle avoids a vertex buffer entirely.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var output: FullscreenOutput;
    output.uv = uv;
    output.clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return output;
}

@fragment
fn extract(input: FullscreenOutput) -> @location(0) vec4<f32> {
    let colour = textureSample(source, source_sampler, input.uv).rgb;
    let luminance = dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722));
    let threshold = settings.y;
    let contribution = max(luminance - threshold, 0.0) / max(luminance, 1e-4);
    return vec4<f32>(colour * contribution, 1.0);
}

@fragment
fn blur(input: FullscreenOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source, 0));
    let step = settings.zw / dimensions;
    let weights = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    var accumulated = textureSample(source, source_sampler, input.uv).rgb * weights[0];
    for (var index = 1; index < 5; index = index + 1) {
        let offset = step * f32(index);
        accumulated += textureSample(source, source_sampler, input.uv + offset).rgb * weights[index];
        accumulated += textureSample(source, source_sampler, input.uv - offset).rgb * weights[index];
    }
    return vec4<f32>(accumulated, 1.0);
}

fn aces(colour: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((colour * (a * colour + b)) / (colour * (c * colour + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@group(1) @binding(0) var bloom: texture_2d<f32>;
@group(1) @binding(1) var bloom_sampler: sampler;

@fragment
fn composite(input: FullscreenOutput) -> @location(0) vec4<f32> {
    var colour = textureSample(source, source_sampler, input.uv).rgb;
    colour += textureSample(bloom, bloom_sampler, input.uv).rgb * settings.y;
    colour *= settings.x;
    // `settings.z` is the tonemap flag. A drawn plan is ink on paper: its colours are already the
    // final ones, and rolling them through a filmic curve would grey the page.
    let mapped = aces(colour);
    return vec4<f32>(mix(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), mapped, settings.z), 1.0);
}
