// Depth-only pass from one light's point of view, into its tile of the shadow atlas.
//
// The light being drawn is selected by an immediate value rather than by a bind group, so the
// whole budget of lights is drawn without rebinding anything between them.

struct ShadowVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
};

@vertex
fn vertex_main(input: ShadowVertexInput) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return shadow_matrices[shadow_index()] * world;
}
