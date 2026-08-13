// Analytic volumetric beams.
//
// Each beam draws its own cone proxy. The fragment shader intersects the view ray with the cone
// analytically, clamps the far end against the scene depth so geometry occludes the shaft, and
// integrates in-scattering through the participating medium. With clear air the integral is zero,
// so the operator sees the aperture and the surface hit but no beam floating in the air.

// In-scattering gain balanced against `LIGHT_POWER` so a beam and its floor pool agree.
const SCATTER_GAIN: f32 = 0.03;

@group(1) @binding(0) var scene_depth: texture_depth_2d;
@group(1) @binding(1) var haze_volume: texture_3d<f32>;
@group(1) @binding(2) var haze_sampler: sampler;

struct BeamVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(7) colour: vec4<f32>,
    @location(8) params: vec4<f32>,
};

struct BeamVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    @location(1) params: vec4<f32>,
};

@vertex
fn vertex_main(input: BeamVertexInput) -> BeamVertexOutput {
    let model = mat4x4<f32>(input.model0, input.model1, input.model2, input.model3);
    // Widen the proxy slightly so the feathered edge is not clipped by its own geometry.
    let widened = vec3<f32>(input.position.xy * 1.35, input.position.z);
    let world = model * vec4<f32>(widened, 1.0);
    var output: BeamVertexOutput;
    output.clip_position = globals.view_projection * world;
    output.colour = input.colour;
    output.params = input.params;
    return output;
}

fn linear_depth(depth: f32) -> f32 {
    let near = globals.params.z;
    let far = globals.params.w;
    if (depth >= 1.0) {
        return far;
    }
    // Reverse of the reversed-Z-free perspective projection used by `ResolvedCamera`.
    return near * far / max(far - depth * (far - near), 1e-6);
}

/// How much light reaches a point `distance` from the cone's virtual apex.
///
/// Square law, without the softening the surface shading uses. A shaft has to be brightest where
/// it leaves the lamp and fade down its throw: the cone widens as it travels, so a view ray
/// crosses more and more of it, and anything flatter than an honest inverse square makes the far
/// end of the beam the bright end — which is backwards. Measuring from the virtual apex is what
/// keeps it right for both a wash, whose apex is just behind its lens and which therefore falls
/// away quickly, and a tight beam, whose apex is metres back and which holds down its length.
fn shaft_falloff(distance: f32) -> f32 {
    let ratio = REFERENCE_THROW / max(distance, 0.05);
    return ratio * ratio;
}

struct ConeHit {
    near: f32,
    far: f32,
    hit: bool,
};

// Intersect a ray with a finite cone whose apex is `apex`, axis `axis`, cosine-squared of the
// half angle `cos2`, and axial length `length`.
fn intersect_cone(
    origin: vec3<f32>,
    direction: vec3<f32>,
    apex: vec3<f32>,
    axis: vec3<f32>,
    cos2: f32,
    length: f32,
) -> ConeHit {
    var result: ConeHit;
    result.hit = false;
    result.near = 0.0;
    result.far = 0.0;

    let offset = origin - apex;
    let d_dot_a = dot(direction, axis);
    let o_dot_a = dot(offset, axis);
    let a = d_dot_a * d_dot_a - cos2;
    let b = 2.0 * (d_dot_a * o_dot_a - cos2 * dot(direction, offset));
    let c = o_dot_a * o_dot_a - cos2 * dot(offset, offset);

    var t0 = 0.0;
    var t1 = 0.0;
    if (abs(a) < 1e-6) {
        if (abs(b) < 1e-9) {
            return result;
        }
        t0 = -c / b;
        t1 = t0;
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if (discriminant < 0.0) {
            return result;
        }
        let root = sqrt(discriminant);
        t0 = (-b - root) / (2.0 * a);
        t1 = (-b + root) / (2.0 * a);
    }
    let low = min(t0, t1);
    let high = max(t0, t1);

    // Keep only the segment on the positive sheet and inside the finite length.
    var entry = 1e9;
    var exit = -1e9;
    let samples = array<f32, 2>(low, high);
    for (var index = 0; index < 2; index = index + 1) {
        let t = samples[index];
        if (t < 0.0) {
            continue;
        }
        let axial = dot(origin + direction * t - apex, axis);
        if (axial < 0.0 || axial > length) {
            continue;
        }
        entry = min(entry, t);
        exit = max(exit, t);
    }
    // The camera may sit inside the cone, or the segment may leave through the end cap.
    let inside_axial = dot(offset, axis);
    let inside = inside_axial >= 0.0 && inside_axial <= length
        && (inside_axial * inside_axial) >= cos2 * dot(offset, offset);
    if (inside) {
        entry = 0.0;
    }
    if (a > 0.0 && low < 0.0 && high < 0.0) {
        return result;
    }
    // Clip against the end cap plane so a ray that exits through the base still terminates.
    if (abs(d_dot_a) > 1e-6) {
        let cap_t = (length - o_dot_a) / d_dot_a;
        if (cap_t > 0.0) {
            let radial = origin + direction * cap_t - apex - axis * length;
            let cap_radius2 = (1.0 - cos2) / max(cos2, 1e-6) * length * length;
            if (dot(radial, radial) <= cap_radius2) {
                entry = min(entry, cap_t);
                exit = max(exit, cap_t);
            }
        }
    }
    if (exit <= entry) {
        return result;
    }
    result.near = max(entry, 0.0);
    result.far = exit;
    result.hit = true;
    return result;
}

/// Local density of the haze at a point, `1.0` being the even medium the cheap tiers use.
///
/// Two samples of one tiling volume, drifting at different speeds and scales, read as air that is
/// moving without ever looking like a scrolling texture. The result is normalised around one, so
/// the fog level the operator set still means what it says.
fn haze_variation(position: vec3<f32>, cloudiness: f32, turbulence: f32, time: f32) -> f32 {
    if (cloudiness <= 0.001) {
        return 1.0;
    }
    let moving_time = time * turbulence;
    let drift = vec3<f32>(moving_time * 0.03, moving_time * 0.017, moving_time * -0.023);
    // Patches first: a very low frequency that leaves whole pockets of the room thin.
    let pocket = textureSampleLevel(haze_volume, haze_sampler, position * 0.035 + drift * 0.4, 0.0).r;
    let coarse = textureSampleLevel(haze_volume, haze_sampler, position * 0.11 + drift, 0.0).r;
    let fine = textureSampleLevel(haze_volume, haze_sampler, position * 0.3 - drift * 1.6, 0.0).r;
    let combined = pocket * 0.5 + coarse * 0.33 + fine * 0.17;
    // The curve is what makes it read as patchy rather than merely uneven: thin air stays thin
    // and a thick pocket carries most of the light.
    let shaped = pow(clamp(combined * 1.75, 0.0, 1.6), 1.7);
    return mix(1.0, shaped, cloudiness);
}

@fragment
fn fragment_main(input: BeamVertexOutput) -> @location(0) vec4<f32> {
    let density = globals.params.y;
    if (density <= 0.0005) {
        discard;
    }
    let light = lights[u32(input.params.x)];
    let axis = light.direction_cos_outer.xyz;
    // A lamp is not a point: the shaft leaves a lens of real width, so it is a frustum, not a
    // cone. The cone it is cut from converges `apex_offset` behind the lens, and everything in
    // front of that apex — inside the fixture — is dropped below.
    let apex_offset = input.params.z;
    let apex = light.position_range.xyz - axis * apex_offset;
    let cos_outer = light.direction_cos_outer.w;
    let cos2 = cos_outer * cos_outer;
    let length_metres = input.params.y + apex_offset;

    let uv = input.clip_position.xy * globals.screen.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let view_ray = globals.inverse_projection * vec4<f32>(ndc, 1.0, 1.0);
    let view_direction = normalize(view_ray.xyz / view_ray.w);
    // The inverse view rotation lives in the transpose of the view matrix's upper 3x3.
    let world_direction = normalize(
        vec3<f32>(
            dot(globals.view[0].xyz, view_direction),
            dot(globals.view[1].xyz, view_direction),
            dot(globals.view[2].xyz, view_direction),
        )
    );
    let origin = globals.camera_position.xyz;

    let hit = intersect_cone(origin, world_direction, apex, axis, cos2, length_metres);
    if (!hit.hit) {
        discard;
    }

    // Cut the shaft off at the lens. The proxy cone reaches back inside the fixture to its virtual
    // apex, and nothing there is haze the light is travelling through.
    var near = hit.near;
    var end = hit.far;
    let axial_origin = dot(origin - apex, axis);
    let axial_direction = dot(world_direction, axis);
    if (abs(axial_direction) > 1e-6) {
        let lens_t = (apex_offset - axial_origin) / axial_direction;
        if (axial_direction > 0.0) {
            near = max(near, lens_t);
        } else {
            end = min(end, lens_t);
        }
    } else if (axial_origin < apex_offset) {
        discard;
    }

    // Row 2 of the view matrix is `-forward`, so the camera forward axis is its negation.
    let forward = -vec3<f32>(globals.view[0].z, globals.view[1].z, globals.view[2].z);
    let depth = textureLoad(scene_depth, vec2<i32>(input.clip_position.xy), 0);
    let scene_distance = linear_depth(depth) / max(dot(world_direction, forward), 1e-4);
    let far = min(end, scene_distance);
    if (far <= near) {
        discard;
    }

    let steps = max(u32(globals.params2.y), 4u);
    let segment = (far - near) / f32(steps);
    // Interleaved offset breaks up banding without a noise texture.
    let jitter = fract(sin(dot(input.clip_position.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let reach = light.position_range.w + apex_offset;
    var accumulated = 0.0;
    for (var step: u32 = 0u; step < steps; step = step + 1u) {
        let t = near + (f32(step) + jitter) * segment;
        let sample_position = origin + world_direction * t;
        let offset = apex - sample_position;
        let distance = length(offset);
        if (distance > reach) {
            continue;
        }
        let to_light = offset / max(distance, 1e-4);
        var cone = beam_profile(light, to_light);
        if (cone <= 0.0) {
            continue;
        }
        // A shaft is only lit where the light actually reaches, so a truss standing in a beam
        // casts its shadow through the haze and not only onto the floor.
        cone *= shadow_lookup(light, sample_position, false);
        let local = haze_variation(
            sample_position,
            globals.params5.x,
            globals.params5.y,
            globals.params3.z,
        );
        accumulated += cone * shaft_falloff(distance) * segment * local;
    }
    // A shaped density curve keeps a heavy hazer atmospheric instead of milky, while staying
    // monotonic: more fog output is always more visible beam.
    let shaped = density * density * (3.0 - 2.0 * density);
    let scatter = accumulated * shaped * input.colour.w * SCATTER_GAIN;
    if (scatter <= 0.0005) {
        discard;
    }
    return vec4<f32>(input.colour.rgb * scatter, 1.0);
}
