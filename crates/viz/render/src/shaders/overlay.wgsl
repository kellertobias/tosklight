// Operator overlay: panels and text drawn in physical pixels over the composited image.

struct OverlayGlobals {
    // width, height, 1/width, 1/height
    screen: vec4<f32>,
};

struct QuadInput {
    @location(0) rect: vec4<f32>,   // x, y, width, height in pixels
    @location(1) uv_rect: vec4<f32>, // u0, v0, u1, v1
    @location(2) colour: vec4<f32>,
};

struct QuadOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) colour: vec4<f32>,
};

@group(0) @binding(0) var<uniform> overlay: OverlayGlobals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

@vertex
fn vertex_main(@builtin(vertex_index) index: u32, input: QuadInput) -> QuadOutput {
    let corner = vec2<f32>(
        f32(index == 1u || index == 2u || index == 4u),
        f32(index == 2u || index == 4u || index == 5u),
    );
    let pixel = input.rect.xy + corner * input.rect.zw;
    let ndc = vec2<f32>(
        pixel.x * overlay.screen.z * 2.0 - 1.0,
        1.0 - pixel.y * overlay.screen.w * 2.0,
    );
    var output: QuadOutput;
    output.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    output.uv = input.uv_rect.xy + corner * (input.uv_rect.zw - input.uv_rect.xy);
    output.colour = input.colour;
    return output;
}

@fragment
fn fragment_main(input: QuadOutput) -> @location(0) vec4<f32> {
    // Glyph cells are white with a coverage alpha, so a tint colours them. The icon region
    // carries its own colours and is drawn with a white tint.
    let texel = textureSample(atlas, atlas_sampler, input.uv);
    return vec4<f32>(input.colour.rgb * texel.rgb, input.colour.a * texel.a);
}
