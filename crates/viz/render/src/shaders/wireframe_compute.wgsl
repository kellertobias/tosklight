// Project each retained real-model edge once, before its four canonical AA corner vertices.
struct Camera {
    view_projection:mat4x4<f32>, eye:vec4<f32>, screen:vec4<f32>, ink:vec4<f32>,
};
struct Segment { start:vec4<f32>, end:vec4<f32>, a:vec4<f32>, b:vec4<f32> };
struct WireInstance { clip_from_local:mat4x4<f32>, local_eye:vec4<f32> };
struct DrawRef { offset:u32, instance:u32 };
struct ProjectedEdge { start:vec4<f32>, end:vec4<f32>, normal:vec2<f32>, pad:vec2<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
// Identical shared layout to the actual-triangle depth/edge rendering batches.
@group(1) @binding(0) var<storage,read> segments:array<Segment>;
@group(1) @binding(1) var<storage,read> instances:array<WireInstance>;
@group(1) @binding(2) var<storage,read> positions:array<vec4<f32>>;
@group(1) @binding(3) var<storage,read> indices:array<u32>;
@group(1) @binding(4) var<storage,read> depth_vertices:array<DrawRef>;
@group(1) @binding(5) var<storage,read> edge_refs:array<DrawRef>;
@group(2) @binding(0) var<storage,read_write> projected_edges:array<ProjectedEdge>;

fn project_edge(edge:Segment,i:WireInstance)->ProjectedEdge {
    // Invalid and hidden edges become zero-area offscreen quads, retaining their stable IDs.
    var out:ProjectedEdge;
    out.start=vec4<f32>(2.0,2.0,2.0,1.0);out.end=out.start;
    out.normal=vec2<f32>(0.0);out.pad=vec2<f32>(0.0);
    let direction=i.local_eye.xyz-(edge.start.xyz+edge.end.xyz)*0.5;
    let facing_a=dot(edge.a.xyz,direction);let facing_b=dot(edge.b.xyz,direction);
    let silhouette=i.local_eye.w<0.5 || facing_a*facing_b<=0.0;
    if !(edge.start.w>0.5 || silhouette) { return out; }
    var a=i.clip_from_local*vec4<f32>(edge.start.xyz,1.0);
    var b=i.clip_from_local*vec4<f32>(edge.end.xyz,1.0);
    if a.z<0.0 && b.z<0.0 { return out; }
    // Clip before dividing by w, in the same order as the guide/previous edge vertex path.
    if a.z<0.0 { let t=a.z/(a.z-b.z);a=mix(a,b,t); }
    if b.z<0.0 { let t=b.z/(b.z-a.z);b=mix(b,a,t); }
    if !(a.w>0.000001 && b.w>0.000001) { return out; }
    let delta=(b.xy/b.w-a.xy/a.w)*camera.screen.xy;
    let size=length(delta);
    if !(size>=0.0001 && size<3.0e38) { return out; }
    out.start=a;out.end=b;out.normal=vec2<f32>(-delta.y,delta.x)/size;
    return out;
}

@compute @workgroup_size(64)
fn project_edges(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let index=invocation.x;
    if index>=arrayLength(&edge_refs) { return; }
    let draw_ref=edge_refs[index];
    projected_edges[index]=project_edge(segments[draw_ref.offset],instances[draw_ref.instance]);
}
