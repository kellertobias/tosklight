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
    // A drawn plan uses flat ink; a rendered view lets its aim lines glow.
    let plot = globals.params3.x;
    let glow = mix(2.5, 1.0, plot);
    return vec4<f32>(input.colour.rgb * glow, input.colour.a);
}
