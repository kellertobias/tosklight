/** Nominal manual focus for the six existing front Fresnels; no fixture identities or DMX change. */
export type WashPoint = { x: number; y: number; z: number };
const OPENING = 48;
const EDGE = 0.04;
const STAGE = { left: -4.92, right: 4.92, front: 0.08, back: 3.92, height: 0.5 };
// Authored focus for this exact shipped Generic Fresnel at model_scale=1: the native
// ScenePlan resolves the real GLB glass 307.4666mm below its rigging point. The opt-in
// real-show pixel test checks this offset so a changed package requires refocusing the rig.
export const FRONT_WASH_LENS_OFFSET_METRES = 0.307466596364975;
const subtract = (a: WashPoint, b: WashPoint): WashPoint => ({
	x: a.x - b.x,
	y: a.y - b.y,
	z: a.z - b.z,
});
const dot = (a: WashPoint, b: WashPoint) => a.x * b.x + a.y * b.y + a.z * b.z;
const cross = (a: WashPoint, b: WashPoint): WashPoint => ({
	x: a.y * b.z - a.z * b.y,
	y: a.z * b.x - a.x * b.z,
	z: a.x * b.y - a.y * b.x,
});
const radians = (value: number) => value * Math.PI / 180;

function rotate(vector: WashPoint, rotation: WashPoint): WashPoint {
	const [x, y, z] = [rotation.x, rotation.y, rotation.z].map(radians);
	const rz = {
		x: vector.x * Math.cos(z) - vector.y * Math.sin(z),
		y: vector.x * Math.sin(z) + vector.y * Math.cos(z),
		z: vector.z,
	};
	const ry = {
		x: rz.x * Math.cos(y) + rz.z * Math.sin(y),
		y: rz.y,
		z: -rz.x * Math.sin(y) + rz.z * Math.cos(y),
	};
	return {
		x: ry.x,
		y: ry.y * Math.cos(x) - ry.z * Math.sin(x),
		z: ry.y * Math.sin(x) + ry.z * Math.cos(x),
	};
}

export function frontWashPlacement(number: number) {
	if (number < 1 || number > 6) return undefined;
	const location = {
		x: -3.8 + (number - 1) * 7.6 / 5,
		y: -3,
		z: 4.15,
	};
	const rotation = {
		x: Math.atan2(5, location.z - STAGE.height) * 180 / Math.PI,
		y: 0,
		z: 0,
	};
	return { location, rotation };
}

export function frontWashLensOrigin(origin: WashPoint, rotation: WashPoint): WashPoint {
	const offset = rotate({ x: 0, y: 0, z: -FRONT_WASH_LENS_OFFSET_METRES }, rotation);
	return { x: origin.x + offset.x, y: origin.y + offset.y, z: origin.z + offset.z };
}

/** Project each stage edge into the actual installed gate; put the entire penumbra inward. */
export function frontWashAppearance(
	number: number,
	origin: WashPoint,
	rotation: WashPoint,
) {
	if (number < 1 || number > 6) return undefined;
	origin = frontWashLensOrigin(origin, rotation);
	const axis = rotate({ x: 0, y: 0, z: -1 }, rotation);
	const tangent = rotate({ x: 1, y: 0, z: 0 }, rotation);
	const bitangent = cross(axis, tangent);
	const { left, right, front, back, height } = STAGE;
	const point = (x: number, y: number) => ({ x, y, z: height });
	const edges = [
		[point(right, front), point(right, back)],
		[point(left, front), point(left, back)],
		[point(left, front), point(right, front)],
		[point(left, back), point(right, back)],
	];
	const centre = point(0, (front + back) / 2);
	const cuts = edges.map(([a, b], blade) => {
		let normal = cross(subtract(b, a), subtract(origin, a));
		if (dot(normal, subtract(centre, origin)) > 0)
			normal = { x: -normal.x, y: -normal.y, z: -normal.z };
		const u = dot(normal, tangent),
			v = dot(normal, bitangent);
		const length = Math.hypot(u, v);
		if (length < 1e-8) return { insertion: 0, angle: 0 };
		const limit =
			-dot(normal, axis) / (Math.tan(radians(OPENING / 2)) * length);
		const angles = [
			Math.atan2(-v, u),
			Math.atan2(v, -u),
			Math.atan2(u, v),
			Math.atan2(-u, -v),
		];
		const angle = ((angles[blade] * 180 / Math.PI + 540) % 360) - 180;
		return {
			insertion: Math.max(0, Math.min(1, (1 + EDGE - limit) / 2)),
			angle,
		};
	});
	return {
		light_source: { type: "tungsten" as const },
		color_temperature_kelvin: 3200,
		luminous_output_lumens: null,
		gel: { type: "open_white" as const },
		manual_optics: {
			zoom_degrees: OPENING,
			shaper_blades: cuts.map((cut) => cut.insertion) as [
				number, number, number, number,
			],
		},
		shaper_angles_degrees: cuts.map((cut) => cut.angle) as [
			number, number, number, number,
		],
	};
}
