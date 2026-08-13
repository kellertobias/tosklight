// Laser scan paths, drawn as camera-facing glowing ribbons.
//
// A laser is not a cone and none of the beam machinery in `beam.wgsl` applies to it. What is
// physically in the air is a beam a few millimetres across, and what an audience sees is the whole
// path at once because the scanner completes the figure hundreds of times a second. So each
// straight run of the path is drawn as one quad turned to face the camera, with a soft profile
// across its width, added into the frame.
//
// Two things make this read as a laser rather than as a bright wire. Each run is added rather than
// blended, so where the path crosses itself it is brighter — which is exactly what a real one
// does. And the ribbon has a narrow, hot core inside a much wider, dim halo, which is what an eye
// and every camera actually record when a laser is pointed at them.

@group(1) @binding(0) var scene_depth: texture_depth_2d;

// How much wider than the beam itself the surrounding glow reaches. A laser in haze has a halo far
// wider than its core, and without one the path reads as a hard-edged line drawn in a paint
// program.
const HALO_WIDTH: f32 = 5.0;

// How much of the run's radiance goes to the halo rather than the core. The core stays dominant;
// the halo is what makes it look like light in air rather than a stroked path.
const HALO_SHARE: f32 = 0.35;

// Scattering gain, balanced so a laser in typical haze sits alongside a lantern's beam rather than
// blowing past it.
const LASER_GAIN: f32 = 2.2;

struct LaserInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) start_radius: vec4<f32>,
    @location(1) end_radius: vec4<f32>,
    @location(2) colour_landing: vec4<f32>,
};

struct LaserOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Signed position across the ribbon, -1 at one edge and +1 at the other.
    @location(0) across: f32,
    @location(1) colour: vec3<f32>,
    // World position of this fragment, for depth occlusion.
    @location(2) world: vec3<f32>,
    // 1 for the figure lying on the surface the beam landed on, 0 for the beam in the air.
    @location(3) @interpolate(flat) landing: f32,
};

@vertex
fn vertex_main(input: LaserInput) -> LaserOutput {
    let start = input.start_radius.xyz;
    let end = input.end_radius.xyz;
    let axis = end - start;

    // Turn the ribbon to face the camera. The width direction is perpendicular both to the run and
    // to the line of sight, which is what keeps a run pointed straight at the viewer from
    // collapsing to nothing.
    let midpoint = 0.5 * (start + end);
    let to_camera = normalize(globals.camera_position.xyz - midpoint);
    var width_axis = cross(normalize(axis), to_camera);
    let width_length = length(width_axis);
    if (width_length < 1e-4) {
        // The run points almost exactly at the camera, so any perpendicular will do.
        width_axis = normalize(cross(normalize(axis), vec3<f32>(0.0, 1.0, 0.0) + vec3<f32>(0.11, 0.0, 0.37)));
    } else {
        width_axis = width_axis / width_length;
    }

    // Two triangles as six vertices: no vertex buffer, no index buffer, one instance per run.
    let corner = vertex_index_to_corner(input.vertex_index);
    let along = corner.x;
    let across = corner.y;
    let radius = mix(input.start_radius.w, input.end_radius.w, along) * HALO_WIDTH;
    let spine = mix(start, end, along);
    let world = spine + width_axis * across * radius;

    var output: LaserOutput;
    output.clip_position = globals.view_projection * vec4<f32>(world, 1.0);
    output.across = across;
    output.colour = input.colour_landing.xyz;
    output.world = world;
    output.landing = input.colour_landing.w;
    return output;
}

// `x` runs along the ribbon, `y` across it.
fn vertex_index_to_corner(index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    return corners[index];
}

fn linear_depth(depth: f32) -> f32 {
    let near = globals.params.z;
    let far = globals.params.w;
    if (depth >= 1.0) {
        return far;
    }
    return near * far / max(far - depth * (far - near), 1e-6);
}

fn laser_haze_variation(position: vec3<f32>, cloudiness: f32, turbulence: f32, time: f32) -> f32 {
    if (cloudiness <= 0.001) {
        return 1.0;
    }
    let drift = vec3<f32>(0.19, 0.11, -0.17) * time * turbulence;
    let cell = position * 0.43 + drift;
    let waves = sin(cell.x * 1.7) * sin(cell.y * 1.3 + 0.7) * sin(cell.z * 1.9 - 0.4);
    let pocket = pow(clamp(waves * 0.85 + 0.78, 0.0, 1.6), 1.8);
    return mix(1.0, pocket, cloudiness);
}

@fragment
fn fragment_main(input: LaserOutput) -> @location(0) vec4<f32> {
    // Geometry in front of the beam hides it. Without this a laser shows through the deck and
    // through anything hanging in its path, which reads as a rendering error rather than as light.
    let depth = textureLoad(scene_depth, vec2<i32>(input.clip_position.xy), 0);
    let forward = normalize(-vec3<f32>(globals.view[0][2], globals.view[1][2], globals.view[2][2]));
    let to_fragment = input.world - globals.camera_position.xyz;
    let along_view = dot(to_fragment, forward);
    if (along_view > linear_depth(depth)) {
        discard;
    }

    let across = abs(input.across);
    // The core: the beam itself, occupying the middle of the drawn width.
    let core_edge = 1.0 / HALO_WIDTH;
    let core = 1.0 - smoothstep(0.0, core_edge, across);
    // The halo: light scattered out of the beam by whatever is in the air.
    let halo = exp(-across * across * 6.0);

    // The figure is a beam and is drawn as one: a hot core inside its halo. A ray of the fan is
    // not a beam but a sample of the sheet a beam swept, and drawing those with cores puts a comb
    // of bright lines across what should be one surface — so they are the halo alone, and their
    // overlap is the sheet.
    let beam = core * (1.0 - HALO_SHARE) + halo * HALO_SHARE;
    let profile = mix(halo, beam, input.landing);
    // Haze is what makes a beam visible in the air at all, so it scales the light on its way to
    // whatever it hits. In clear air that is drawn faintly rather than not at all: the beam is
    // real even when almost nothing is scattering off it, which is why a laser show without haze
    // is bright figures on the surfaces and next to nothing in between.
    //
    // A laser answers haze harder than a lantern does. A lantern's shaft is light spread through a
    // cone metres across; a laser's is a millimetres-thin sheet cutting one slice out of the same
    // air, and every watt of it is in that slice. Hazing a room a lantern barely notices is what
    // turns a laser rig from figures on the floor into the thing people came to look at.
    //
    // The figure itself is not scattered light and does not fade with the haze. A laser pointed at
    // a wall in still air puts the same pattern on it.
    let density = globals.params.y;
    let scattered = 0.08 + density * 1.62;
    let local_haze = laser_haze_variation(
        input.world,
        globals.params5.z,
        globals.params5.w,
        globals.params3.z,
    );
    let visibility = mix(scattered * local_haze, 1.0, input.landing);

    // The operator's own strength for every laser in the rig, on top of all of it. It scales the
    // figure and the beam together, because it answers "how strong are the lasers in this
    // picture" rather than anything about one of the two.
    let brightness = globals.params3.w;
    let radiance = input.colour * profile * visibility * LASER_GAIN * brightness;
    return vec4<f32>(radiance, 1.0);
}
