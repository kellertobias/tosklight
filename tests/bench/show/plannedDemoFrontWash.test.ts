import { describe, expect, it } from "vitest";
import {
	frontWashAppearance,
	frontWashLensOrigin,
	frontWashPlacement,
} from "../../support/plannedDemoFrontWash";

/** Independent floor-ray evaluation of the renderer's documented field/gate contract. */
function lightAt(number: number, x: number, y: number) {
	const pose = frontWashPlacement(number)!;
	const appearance = frontWashAppearance(number, pose.location, pose.rotation)!;
	const origin = frontWashLensOrigin(pose.location, pose.rotation);
	const h = origin.z - 0.5,
		d = (pose.location.z - 0.5) * Math.tan(pose.rotation.x * Math.PI / 180);
	const ray = [x - origin.x, y - origin.y, -h];
	const axisHeight = pose.location.z - 0.5;
	const axisLength = Math.hypot(axisHeight, d);
	const along = (ray[1] * d - ray[2] * axisHeight) / axisLength;
	if (along <= 0) return 0;
	const tangent = Math.tan(24 * Math.PI / 180);
	const u = ray[0] / (along * tangent);
	const v = (-ray[1] * axisHeight - ray[2] * d) / (axisLength * along * tangent);
	const cosine = along / Math.hypot(...ray),
		outer = Math.cos(24 * Math.PI / 180);
	const inner = Math.cos(24 * 0.42 * Math.PI / 180);
	const smooth = (a: number, b: number, value: number) => {
		const t = Math.max(0, Math.min(1, (value - a) / (b - a)));
		return t * t * (3 - 2 * t);
	};
	if (cosine <= outer) return 0;
	const radial = (1 - cosine) / (1 - outer);
	let output =
		smooth(outer, inner, cosine) *
		((0.35 + 2.4 * (1 - radial) ** 2.5) * 0.45 + 0.55);
	appearance.manual_optics.shaper_blades.forEach((insertion, blade) => {
		if (insertion <= 0) return;
		const angle = appearance.shaper_angles_degrees[blade] * Math.PI / 180;
		const q = [
			u * Math.cos(angle) - v * Math.sin(angle),
			u * Math.sin(angle) + v * Math.cos(angle),
		];
		const projected = [q[0], -q[0], q[1], -q[1]][blade];
		const threshold = 1 - insertion * 2;
		output *= smooth(threshold + 0.04, threshold - 0.04, projected);
	});
	return output;
}

describe("six wide Fresnel front wash", () => {
	it("keeps six distinct front-truss locations and leaves other fixtures untouched", () => {
		const locations = Array.from(
			{ length: 6 },
			(_, i) => frontWashPlacement(i + 1)!,
		);
		expect(new Set(locations.map((pose) => pose.location.x)).size).toBe(6);
		for (const pose of locations) {
			expect(pose.location.y).toBe(-3);
			expect(Math.abs(pose.location.x)).toBeLessThan(4);
		}
		expect(frontWashPlacement(7)).toBeUndefined();
		expect(frontWashPlacement(11)).toBeUndefined();
	});
	it("covers most of the stage with soft overlapping fields and no exterior floor spill", () => {
		let covered = 0,
			samples = 0;
		for (let x = -4.9; x <= 4.9; x += 0.1)
			for (let y = 0.1; y <= 3.9; y += 0.1) {
				const output = [1, 2, 3, 4, 5, 6].reduce(
					(sum, number) => sum + lightAt(number, x, y),
					0,
				);
				samples++;
				if (output > 0.01) covered++;
			}
		expect(covered / samples).toBeGreaterThan(0.8);
		for (const number of [1, 2, 3, 4, 5, 6])
			for (let coordinate = -6; coordinate <= 6; coordinate += 0.1) {
				for (const [x, y] of [
					[coordinate, -0.01],
					[coordinate, 4.01],
					[-5.01, coordinate],
					[5.01, coordinate],
				])
					expect(lightAt(number, x, y)).toBe(0);
			}
	});
});
