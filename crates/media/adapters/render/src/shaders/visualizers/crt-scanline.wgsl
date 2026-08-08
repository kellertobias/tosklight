// A curved tube: aperture grid, scanlines, vignette, and a flash on bass.
fn shade(p: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    // Barrel distortion first, so everything after it curves with the glass.
    let centred = uv * 2.0 - 1.0;
    let bend = curvature() * dot(centred, centred);
    let curved = centred * (1.0 + bend * 0.35);
    if abs(curved.x) > 1.0 || abs(curved.y) > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let screen = curved * 0.5 + 0.5;

    let density = max(count(), 1.0);
    let scanline = 0.6 + 0.4 * sin(screen.y * density * TAU);
    let aperture = 0.75 + 0.25 * sin(screen.x * density * TAU * 1.5);
    let vignette = 1.0 - dot(curved, curved) * 0.35;
    let flash = bass() * reactivity() * 0.4 + beat() * 0.2;

    let brightness = clamp(scanline * aperture * vignette + flash, 0.0, 1.0);
    return vec4<f32>(primary() * brightness, 1.0);
}
