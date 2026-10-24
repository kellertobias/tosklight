// Technical picture only. No PBR/light/tile/shadow/HDR/post-processing bindings.
struct Camera {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    screen: vec4<f32>,
    ink: vec4<f32>,
};
struct Segment { start:vec4<f32>, end:vec4<f32>, a:vec4<f32>, b:vec4<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
@group(1) @binding(0) var<storage,read> segments:array<Segment>;
struct WireInstance {
    clip_from_local:mat4x4<f32>, local_eye:vec4<f32>,
};
struct DrawRef { offset:u32, instance:u32 };
@group(1) @binding(1) var<storage,read> instances:array<WireInstance>;
@group(1) @binding(2) var<storage,read> positions:array<vec4<f32>>;
@group(1) @binding(3) var<storage,read> indices:array<u32>;
@group(1) @binding(4) var<storage,read> depth_vertices:array<DrawRef>;
@group(1) @binding(5) var<storage,read> edge_refs:array<DrawRef>;
struct ProjectedEdge { start:vec4<f32>, end:vec4<f32>, normal:vec2<f32>, pad:vec2<f32> };
@group(2) @binding(0) var<storage,read> projected_edges:array<ProjectedEdge>;
@vertex fn depth_vertex(@builtin(vertex_index) vertex:u32)->@builtin(position) vec4<f32> {
    // Indexed draw IDs address instance-specific virtual vertices. Shared triangle corners can
    // reuse their transformed result, while the underlying model positions remain shared.
    let draw_ref=depth_vertices[vertex];
    return instances[draw_ref.instance].clip_from_local * positions[draw_ref.offset];
}
@fragment fn depth_fragment()->@location(0) vec4<f32> { return vec4<f32>(0.0); }
struct LineOutput {
    @builtin(position) position:vec4<f32>,
    @location(0) colour:vec4<f32>,
    @location(1) @interpolate(linear) across:f32,
};
// One physical-pixel core plus a half-pixel coverage fringe. Four canonical corners form
// the exact same two triangles with indices [0,1,2,0,2,3], independent of hardware line widths.
fn expanded(a_in:vec4<f32>,b_in:vec4<f32>,ca:vec4<f32>,cb:vec4<f32>,corner:u32,visible:bool)->LineOutput {
    var out:LineOutput;
    out.position=vec4<f32>(2.0,2.0,2.0,1.0);out.colour=vec4<f32>(0.0);out.across=0.0;
    if !visible || (a_in.z<0.0 && b_in.z<0.0) { return out; }
    var a=a_in;var b=b_in;var colour_a=ca;var colour_b=cb;
    // Clip before dividing by w: a line crossing the eye/near plane must not explode into a
    // screen-sized quad or reverse its normals. Positive near-plane z also implies positive w.
    if a.z<0.0 { let t=a.z/(a.z-b.z);a=mix(a,b,t);colour_a=mix(ca,cb,t); }
    if b.z<0.0 { let t=b.z/(b.z-a.z);b=mix(b,a,t);colour_b=mix(cb,colour_a,t); }
    if a.w<=0.000001 || b.w<=0.000001 { return out; }
    let delta=(b.xy/b.w-a.xy/a.w)*camera.screen.xy;
    let length=length(delta);
    if length<0.0001 { return out; }
    let normal=vec2<f32>(-delta.y,delta.x)/length;
    let far=corner>=2u;
    let side=select(-1.0,1.0,corner==1u || corner==2u);
    var p=select(a,b,far);
    let half_width=camera.screen.z;
    p.x+=normal.x*side*half_width*2.0/camera.screen.x*p.w;
    p.y+=normal.y*side*half_width*2.0/camera.screen.y*p.w;
    out.position=p;out.colour=select(colour_a,colour_b,far);out.across=side*half_width;
    return out;
}
@vertex fn edge_vertex(@builtin(vertex_index) vertex:u32)->LineOutput {
    let edge=projected_edges[vertex/4u];
    let corner=vertex%4u;
    let side=select(-1.0,1.0,corner==1u || corner==2u);
    var p=select(edge.start,edge.end,corner>=2u);
    let half_width=camera.screen.z;
    p.x+=edge.normal.x*side*half_width*2.0/camera.screen.x*p.w;
    p.y+=edge.normal.y*side*half_width*2.0/camera.screen.y*p.w;
    var out:LineOutput;
    out.position=p;out.colour=camera.ink;out.across=side*half_width;
    return out;
}
// Guides remain a six-vertex non-indexed list; translate to canonical corners explicitly.
fn guide_corner(vertex:u32)->u32 {
    let corners=array<u32,6>(0u,1u,2u,0u,2u,3u);
    return corners[vertex%6u];
}
@vertex fn guide_vertex(@builtin(vertex_index) vertex:u32)->LineOutput {
    let edge=segments[vertex/6u];
    return expanded(camera.view_projection*vec4<f32>(edge.start.xyz,1.0),
        camera.view_projection*vec4<f32>(edge.end.xyz,1.0),edge.a,edge.b,guide_corner(vertex),true);
}
@fragment fn line_fragment(input:LineOutput)->@location(0) vec4<f32> {
    let coverage=1.0-smoothstep(0.45,1.25,abs(input.across));
    return vec4<f32>(input.colour.rgb,input.colour.a*coverage);
}
