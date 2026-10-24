// Screen-tile light culling.
//
// Bounding the work per tile is what keeps hundreds of simultaneous beams inside the frame
// budget: without it every fragment would evaluate every fixture in the show.

struct Globals {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    screen: vec4<f32>,
    params: vec4<f32>,
    params2: vec4<f32>,
};

struct Light {
    position_range: vec4<f32>,
    direction_cos_outer: vec4<f32>,
    colour_intensity: vec4<f32>,
    params: vec4<f32>,
    // Keep this storage layout byte-for-byte identical to `Light` in common.wgsl and
    // `GpuLight` in instances.rs. Storage-buffer arrays use the struct stride to locate every
    // light; a shortened culling-only declaration makes light 1 read the middle of light 0.
    tangent_frost: vec4<f32>,
    optics: vec4<f32>,
    shapers: vec4<f32>,
    shaper_angles: vec4<f32>,
    gate: vec4<f32>,
    shadow: vec4<f32>,
    gobo_wheels: array<vec4<f32>, 8>,
    prism_wheels: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read_write> tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> tile_lights: array<u32>;

override DRAFT_CONE_BOUNDS: bool = false;

const TILE_SIZE: u32 = 16u;
const MAX_LIGHTS_PER_TILE: u32 = 256u;
const WORKGROUP: u32 = 64u;

var<workgroup> shared_count: atomic<u32>;
var<workgroup> shared_indices: array<u32, MAX_LIGHTS_PER_TILE>;

struct ScreenBounds {
    min: vec2<f32>,
    max: vec2<f32>,
    visible: bool,
};

// Conservative screen-space bounds of the light's bounding sphere.
fn light_bounds(light: Light) -> ScreenBounds {
    var result: ScreenBounds;
    result.visible = true;
    result.min = vec2<f32>(1e9);
    result.max = vec2<f32>(-1e9);

    // Bounding sphere of the spot cone, tighter than the raw range sphere.
    let cos_outer = clamp(light.direction_cos_outer.w, -1.0, 1.0);
    let range = light.position_range.w;
    var centre = light.position_range.xyz;
    var radius = range * 0.5;
    if (cos_outer >= 0.5) {
        let along = range / (2.0 * cos_outer);
        centre = light.position_range.xyz + light.direction_cos_outer.xyz * along;
        radius = along;
    } else {
        let sin_outer = sqrt(max(1.0 - cos_outer * cos_outer, 0.0));
        centre = light.position_range.xyz + light.direction_cos_outer.xyz * (range * cos_outer);
        radius = range * sin_outer;
    }

    var world_min = centre - vec3<f32>(radius);
    var world_max = centre + vec3<f32>(radius);
    let axis_length = length(light.direction_cos_outer.xyz);
    // Draft surface lighting rejects both distance>range and dot(ray,axis)<=cos_outer,
    // before evaluating any optics. Thus both enclosing AABBs may be intersected safely.
    // Other tiers retain their existing sphere bounds, including all malformed/wide cases.
    if (DRAFT_CONE_BOUNDS && cos_outer >= 0.5 && cos_outer <= 1.0
        && axis_length >= 0.9999 && axis_length <= 1.0001
        && range > 0.0 && range < 1e10
        && all(abs(light.position_range.xyz) < vec3<f32>(1e10))) {
        let axis = light.direction_cos_outer.xyz / axis_length;
        let effective_cos = light.direction_cos_outer.w / axis_length;
        if (effective_cos >= 0.5 && effective_cos <= 1.0) {
            let end = light.position_range.xyz + axis * range;
            let radial = range * sqrt(max(1.0 - effective_cos * effective_cos, 0.0)) / effective_cos;
            let extent = radial * sqrt(max(vec3<f32>(1.0) - axis * axis, vec3<f32>(0.0)));
            let padding = vec3<f32>(max(range * 1e-4, 1e-4));
            world_min = max(world_min, min(light.position_range.xyz, end - extent) - padding);
            world_max = min(world_max, max(light.position_range.xyz, end + extent) + padding);
        }
    }

    for (var corner: u32 = 0u; corner < 8u; corner = corner + 1u) {
        let sign = vec3<f32>(
            select(-1.0, 1.0, (corner & 1u) != 0u),
            select(-1.0, 1.0, (corner & 2u) != 0u),
            select(-1.0, 1.0, (corner & 4u) != 0u),
        );
        let world = vec4<f32>(select(world_min, world_max, sign > vec3<f32>(0.0)), 1.0);
        let clip = globals.view_projection * world;
        if (clip.w <= 0.0001) {
            // Straddles the camera plane: treat as covering the whole screen.
            result.min = vec2<f32>(0.0);
            result.max = globals.screen.xy;
            return result;
        }
        let ndc = clip.xy / clip.w;
        let screen = vec2<f32>((ndc.x * 0.5 + 0.5) * globals.screen.x, (0.5 - ndc.y * 0.5) * globals.screen.y);
        result.min = min(result.min, screen);
        result.max = max(result.max, screen);
    }
    if (result.max.x < 0.0 || result.max.y < 0.0
        || result.min.x > globals.screen.x || result.min.y > globals.screen.y) {
        result.visible = false;
    }
    return result;
}

@compute @workgroup_size(64)
fn cull_main(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    let tiles_x = u32(globals.params2.w);
    let tile = group.y * tiles_x + group.x;
    if (local == 0u) {
        atomicStore(&shared_count, 0u);
    }
    workgroupBarrier();

    let tile_min = vec2<f32>(f32(group.x * TILE_SIZE), f32(group.y * TILE_SIZE));
    let tile_max = tile_min + vec2<f32>(f32(TILE_SIZE));
    let count = u32(globals.params2.x);
    for (var index = local; index < count; index = index + WORKGROUP) {
        let bounds = light_bounds(lights[index]);
        if (!bounds.visible) {
            continue;
        }
        if (bounds.max.x < tile_min.x || bounds.min.x > tile_max.x
            || bounds.max.y < tile_min.y || bounds.min.y > tile_max.y) {
            continue;
        }
        let slot = atomicAdd(&shared_count, 1u);
        if (slot < MAX_LIGHTS_PER_TILE) {
            shared_indices[slot] = index;
        }
    }
    workgroupBarrier();

    let count_in_tile = atomicLoad(&shared_count);
    let stored = min(count_in_tile, MAX_LIGHTS_PER_TILE);
    for (var index = local; index < stored; index = index + WORKGROUP) {
        tile_lights[tile * MAX_LIGHTS_PER_TILE + index] = shared_indices[index];
    }
    if (local == 0u) {
        // Preserve overflow. Surface shaders then evaluate all lights in a stable order,
        // rather than dropping whichever lights lost the workgroup's atomic arrival race.
        atomicStore(&tile_counts[tile], count_in_tile);
    }
}
