import type { CadViewDirection } from "./types";
export function normalOf(
	points: [
		[number, number, number],
		[number, number, number],
		[number, number, number],
	],
): [number, number, number] {
	const first = points[1].map((value, index) => value - points[0][index]);
	const second = points[2].map((value, index) => value - points[0][index]);
	const cross: [number, number, number] = [
		first[1] * second[2] - first[2] * second[1],
		first[2] * second[0] - first[0] * second[2],
		first[0] * second[1] - first[1] * second[0],
	];
	const length = Math.hypot(...cross) || 1;
	return cross.map((value) => value / length) as [number, number, number];
}

export function dot3(
	first: readonly [number, number, number],
	second: readonly [number, number, number],
) {
	return first[0] * second[0] + first[1] * second[1] + first[2] * second[2];
}

export function cameraVector(view: CadViewDirection): [number, number, number] {
	switch (view) {
		case "top_down":
			return [0, 1, 0];
		case "left_to_right":
			return [-1, 0, 0];
		case "right_to_left":
			return [1, 0, 0];
		case "front_to_back":
			return [0, 0, 1];
		case "back_to_front":
			return [0, 0, -1];
	}
}

export function point3Key(point: readonly [number, number, number]) {
	return point.map((value) => Math.round(value * 20)).join(",");
}
