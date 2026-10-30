/** Draws the rig from above, each fixture coloured and brightened by its current output. */

import { fixtureLabel, type PatchedFixture } from "./backend";
import { base64, Raster } from "./png";

interface OutputValue {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value: any };
}

const WIDTH = 900;
const HEIGHT = 560;
const MARGIN = 40;

/** CIE XYZ (D65) to gamma-encoded sRGB in 0–255, normalised so the brightest channel is full. */
export function xyzToRgb({ x, y, z }: { x: number; y: number; z: number }): [number, number, number] {
	const linear = [
		3.2406 * x - 1.5372 * y - 0.4986 * z,
		-0.9689 * x + 1.8758 * y + 0.0415 * z,
		0.0557 * x - 0.204 * y + 1.057 * z,
	].map((channel) => Math.max(0, channel));
	const peak = Math.max(...linear);
	if (peak <= 0) return [255, 255, 255];
	return linear.map((channel) => {
		const value = channel / peak;
		const encoded = value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055;
		return Math.round(encoded * 255);
	}) as [number, number, number];
}

export function snapshotImage(fixtures: readonly PatchedFixture[], output: readonly OutputValue[]) {
	const byFixture = new Map<string, { intensity: number | null; color: [number, number, number] | null }>();
	for (const entry of output) {
		const current = byFixture.get(entry.fixture_id) ?? { intensity: null, color: null };
		if (entry.attribute === "intensity" && entry.value.kind === "normalized") current.intensity = entry.value.value;
		if (entry.attribute === "color" && entry.value.kind === "color_xyz") current.color = xyzToRgb(entry.value.value);
		byFixture.set(entry.fixture_id, current);
	}
	const placed = fixtures
		.filter((fixture) => fixture.virtual_fixture_number == null)
		.map((fixture) => {
			const location = (fixture.location as { x?: number; y?: number } | undefined) ?? {};
			return { fixture, x: Number(location.x ?? 0), y: Number(location.y ?? 0) };
		});
	const xs = placed.map((item) => item.x);
	const ys = placed.map((item) => item.y);
	const [minX, maxX] = [Math.min(0, ...xs), Math.max(1, ...xs)];
	const [minY, maxY] = [Math.min(0, ...ys), Math.max(1, ...ys)];
	const scale = Math.min((WIDTH - MARGIN * 2) / (maxX - minX || 1), (HEIGHT - MARGIN * 2) / (maxY - minY || 1));
	const summary: Array<{ fixture: string; x: number; y: number; intensity_percent: number | null; color: string | null }> = [];
	const raster = new Raster(WIDTH, HEIGHT, [11, 14, 18]);
	for (const { fixture, x, y } of placed) {
		const look = byFixture.get(fixture.fixture_id) ?? { intensity: null, color: null };
		const level = look.intensity ?? 0;
		const color = look.color ?? ([255, 236, 200] as [number, number, number]);
		summary.push({
			fixture: fixtureLabel(fixture) ?? fixture.name,
			x,
			y,
			intensity_percent: look.intensity == null ? null : Math.round(level * 100),
			color: look.color ? `#${color.map((channel) => channel.toString(16).padStart(2, "0")).join("")}` : null,
		});
		// Stage +Y is upstage, which is up the picture.
		raster.disc(MARGIN + (x - minX) * scale, HEIGHT - MARGIN - (y - minY) * scale, 13, color, Math.max(0.06, level), [138, 150, 160]);
	}
	return { png: base64(raster.png()), summary };
}
