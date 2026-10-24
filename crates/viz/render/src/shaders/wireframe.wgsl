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
    model:mat4x4<f32>, n0:vec4<f32>, n1:vec4<f32>, n2:vec4<f32>,
};
struct DrawRef { offset:u32, instance:u32 };
@group(1) @binding(1) var<storage,read> instances:array<WireInstance>;
@group(1) @binding(2) var<storage,read> positions:array<vec4<f32>>;
@group(1) @binding(3) var<storage,read> indices:array<u32>;
@group(1) @binding(4) var<storage,read> triangles:array<DrawRef>;
@group(1) @binding(5) var<storage,read> edge_refs:array<DrawRef>;
@vertex fn depth_vertex(@builtin(vertex_index) vertex:u32)->@builtin(position) vec4<f32> {
    let draw_ref=triangles[vertex/3u];
    return camera.view_projection * instances[draw_ref.instance].model * positions[indices[draw_ref.offset+vertex%3u]];
}
@fragment fn depth_fragment()->@location(0) vec4<f32> { return vec4<f32>(0.0); }
struct LineOutput {
    @builtin(position) position:vec4<f32>,
    @location(0) colour:vec4<f32>,
    @location(1) @interpolate(linear) across:f32,
};
// One physical-pixel core plus a half-pixel coverage fringe. Six vertices avoid multisampling
// and are independent of implementation-specific hardware line widths.
fn expanded(a_in:vec4<f32>,b_in:vec4<f32>,ca:vec4<f32>,cb:vec4<f32>,index:u32,visible:bool)->LineOutput {
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
    let corner=index%6u;
    let far=corner==2u || corner==4u || corner==5u;
    let side=select(-1.0,1.0,corner==1u || corner==2u || corner==4u);
    var p=select(a,b,far);
    let half_width=camera.screen.z;
    p.x+=normal.x*side*half_width*2.0/camera.screen.x*p.w;
    p.y+=normal.y*side*half_width*2.0/camera.screen.y*p.w;
    out.position=p;out.colour=select(colour_a,colour_b,far);out.across=side*half_width;
    return out;
}
@vertex fn edge_vertex(@builtin(vertex_index) vertex:u32)->LineOutput {
    let draw_ref=edge_refs[vertex/6u];let edge=segments[draw_ref.offset];let i=instances[draw_ref.instance];let model=i.model;
    let normal=mat3x3<f32>(i.n0.xyz,i.n1.xyz,i.n2.xyz);
    let a=model*vec4<f32>(edge.start.xyz,1.0);let b=model*vec4<f32>(edge.end.xyz,1.0);
    let direction=camera.eye.xyz-(a.xyz+b.xyz)*0.5;
    let facing_a=dot(normal*edge.a.xyz,direction);let facing_b=dot(normal*edge.b.xyz,direction);
    let silhouette=facing_a*facing_b<=0.0;
    return expanded(camera.view_projection*a,camera.view_projection*b,camera.ink,camera.ink,vertex,edge.start.w>0.5 || silhouette);
}
@vertex fn guide_vertex(@builtin(vertex_index) vertex:u32)->LineOutput {
    let edge=segments[vertex/6u];
    return expanded(camera.view_projection*vec4<f32>(edge.start.xyz,1.0),
        camera.view_projection*vec4<f32>(edge.end.xyz,1.0),edge.a,edge.b,vertex,true);
}
@fragment fn line_fragment(input:LineOutput)->@location(0) vec4<f32> {
    let coverage=1.0-smoothstep(0.45,1.25,abs(input.across));
    return vec4<f32>(input.colour.rgb,input.colour.a*coverage);
}
