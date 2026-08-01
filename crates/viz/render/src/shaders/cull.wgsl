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
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read_write> tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> tile_lights: array<u32>;

const TILE_SIZE: u32 = 16u;
const MAX_LIGHTS_PER_TILE: u32 = 96u;
const WORKGROUP: u32 = 64u;

var<workgroup> shared_count: atomic<u32>;
var<workgroup> shared_indices: array<u32, 96>;

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

    for (var corner: u32 = 0u; corner < 8u; corner = corner + 1u) {
        let sign = vec3<f32>(
            select(-1.0, 1.0, (corner & 1u) != 0u),
            select(-1.0, 1.0, (corner & 2u) != 0u),
            select(-1.0, 1.0, (corner & 4u) != 0u),
        );
        let world = vec4<f32>(centre + sign * radius, 1.0);
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

    let total = min(atomicLoad(&shared_count), MAX_LIGHTS_PER_TILE);
    for (var index = local; index < total; index = index + WORKGROUP) {
        tile_lights[tile * MAX_LIGHTS_PER_TILE + index] = shared_indices[index];
    }
    if (local == 0u) {
        atomicStore(&tile_counts[tile], total);
    }
}
