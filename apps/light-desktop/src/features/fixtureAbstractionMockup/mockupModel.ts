export type Recipe = {
	red: number; green: number; blue: number; white: number;
	amber: number; uv: number; temperature: number; tint: number;
	wheel1: number; wheel2: number;
};
export const initialRecipe: Recipe = {
	red: 100, green: 24, blue: 8, white: 0, amber: 0, uv: 0,
	temperature: 6500, tint: 0, wheel1: 0, wheel2: 0,
};
export const fixtureOptions = [
	{ value: "mixed", label: "Mixed selection · A7 / ROOT PAR / AURO" },
	{ value: "rgbw", label: "Generic RGBW" },
	{ value: "rgbwauv", label: "Generic RGBWAUV" },
	{ value: "rgbal", label: "RGBAL profile" },
	{ value: "cmy", label: "CMY + two wheels" },
	{ value: "wheel", label: "Color wheel only" },
];
export const wheelColors = ["Auto", "Open", "Red", "Blue", "Amber"];
export const wheelCorrections = ["Auto", "Open", "CTO", "CTB"];
const clamp = (v: number) => Math.max(0, Math.min(1, v));
const decode = (v: number) => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4;
const encode = (v: number) => v <= .0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - .055;

/** Reference UI preview; never a measured fixture solver. Keep the base color separately. */
export function recipePreview(recipe: Recipe) {
	const coloredGain = Math.min(1, 2 * (1 - recipe.white / 100));
	const whiteGain = Math.min(1, 2 * recipe.white / 100);
	const warm = clamp((6500 - recipe.temperature) / 4500);
	const cool = clamp((recipe.temperature - 6500) / 13500);
	const white = [1 - cool * .35, clamp(1 - warm * .27 + recipe.tint * 8), 1 - warm * .7];
	const base = [recipe.red, recipe.green, recipe.blue].map((v, i) => decode(v / 100) + [1, .3, .015][i] * recipe.amber / 100);
	const linear = base.map((v, i) => coloredGain * v + whiteGain * white[i]);
	const hex = (rgb: number[]) => `#${rgb.map(v => Math.round(clamp(encode(v / Math.max(1, ...rgb))) * 255).toString(16).padStart(2, "0")).join("")}`;
	const xyz = [base[0] * .4124564 + base[1] * .3575761 + base[2] * .1804375,
		base[0] * .2126729 + base[1] * .7151522 + base[2] * .072175,
		base[0] * .0193339 + base[1] * .119192 + base[2] * .9503041];
	return { hex: hex(linear), baseHex: hex(base), xyz, coloredGain, whiteGain };
}

export type Vec3 = { x: number; y: number; z: number };
export const points = [
	{ id: "origin", name: "Origin", x: 0, y: 0, z: 0 },
	{ id: "point-stage-center", name: "Stage center", x: 0, y: 2, z: 0 },
	{ id: "point-performer", name: "Performer", x: 1.5, y: 1, z: .8 },
	{ id: "point-truss", name: "Moving truss", x: -2, y: -2, z: 5 },
];
export type PositionIntent = { type: "angles"; pan: number; tilt: number }
	| { type: "target"; point: string; offset: Vec3 };
export function resolveTarget(point: string, offset: Vec3, lift: number, performer: number): Vec3 {
	const base = points.find(p => p.id === point) ?? points[0];
	return { x: base.x + offset.x + (point === "point-performer" ? performer : 0),
		y: base.y + offset.y, z: base.z + offset.z + (point === "point-truss" ? lift : 0) };
}
export function resolveAngles(target: Vec3, mount: Vec3, yaw: number) {
	const dx = target.x - mount.x, dy = target.y - mount.y, dz = target.z - mount.z;
	return { pan: Math.atan2(dx, dy) * 180 / Math.PI - yaw,
		tilt: Math.atan2(Math.hypot(dx, dy), -dz) * 180 / Math.PI };
}
