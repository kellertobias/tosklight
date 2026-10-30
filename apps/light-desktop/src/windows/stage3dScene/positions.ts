import type { StagePosition3d } from "../../features/server/contracts";

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


/** One physical placement, preserving layout overrides and the old absent-marker heuristic. */
export function physicalStagePosition(
	instance: {
		id: string;
		location?: { x: number; y: number; z: number };
		rotation?: { x: number; y: number; z: number };
		stage_position_set?: boolean;
	},
	positions3d: Record<string, StagePosition3d>,
	legacyPosition: { x: number; y: number; rotation: number } | undefined,
	index: number,
): StagePosition3d {
	const override = positions3d[instance.id];
	if (override) return override;
	const location = instance.location;
	const placed = instance.stage_position_set ?? Boolean(location && (location.x || location.y || location.z));
	if (!placed || !location) return migrateStagePosition(legacyPosition, index);
	return {
		x: location.x / 1000,
		y: location.y / 1000,
		z: location.z / 1000,
		rotationX: instance.rotation?.x ?? 0,
		rotationY: instance.rotation?.y ?? 0,
		rotationZ: instance.rotation?.z ?? 0,
	};
}
