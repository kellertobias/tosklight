// Aim lines for `3D Lines` and the orthographic plan views.

struct LineVertexInput {
    @location(0) position: vec4<f32>,
    @location(1) colour: vec4<f32>,
};

struct LineVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) colour: vec4<f32>,
};

@vertex
fn vertex_main(input: LineVertexInput) -> LineVertexOutput {
    var output: LineVertexOutput;
    output.clip_position = globals.view_projection * vec4<f32>(input.position.xyz, 1.0);
    output.colour = input.colour;
    return output;
}

@fragment
fn fragment_main(input: LineVertexOutput) -> @location(0) vec4<f32> {
    // Lines glow only where the view simulates light, because there the glow is the light. A drawn
    // plan and an outline view use flat ink: their lines are structure, not output, and multiplying
    // them made every fixture outline read as bright whatever ink it was given.
    let glow = select(2.5, 1.0, flat_ink());
    return vec4<f32>(input.colour.rgb * glow, input.colour.a);
}
