import type { StagePosition3d } from "../../api/ServerContext";

export function defaultStagePosition(index: number): StagePosition3d {
	return {
		x: -5.25 + (index % 8) * 1.5,
		y: 1 + Math.floor(index / 8) * 1.6,
		z: 5,
		rotationX: 0,
		rotationY: 0,
		rotationZ: 0,
	};
}

export function migrateStagePosition(
	position: { x: number; y: number; rotation: number } | undefined,
	index: number,
): StagePosition3d {
	if (!position) return defaultStagePosition(index);
	return {
		x: (position.x / 100 - 0.5) * 12,
		y: (position.y / 100) * 8,
		z: 5,
		rotationX: 0,
		rotationY: 0,
		rotationZ: position.rotation,
	};
}
