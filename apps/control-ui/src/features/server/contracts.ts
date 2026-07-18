import type { Cue } from "../../api/types";
import type { DeskModel } from "../../types";

export interface StoredDeskLayout {
	desks: DeskModel[];
	activeDeskId: string;
	windowSettings?: Partial<import("../../types").WindowSettings>;
}

export interface CommandChoiceOption {
	id: string;
	label: string;
	command: string;
}

export interface PendingCommandChoice {
	type: "cue_move_copy";
	operation: "copy" | "move";
	command: string;
	options: CommandChoiceOption[];
	cancel_label: string;
}

export interface StagePosition3d {
	x: number;
	y: number;
	z: number;
	rotationX: number;
	rotationY: number;
	rotationZ: number;
}

export interface StoredStageLayout {
	version?: 2;
	positions: Record<string, { x: number; y: number; rotation: number }>;
	positions3d?: Record<string, StagePosition3d>;
	camera3d?: {
		position: [number, number, number];
		target: [number, number, number];
	};
}

export function deskLayoutScopeKey(
	showId: string | null | undefined,
	userId: string | null | undefined,
) {
	return showId && userId ? `${showId}:${userId}` : null;
}

export function cueOnlyRestoration(cues: Cue[]): {
	changes: Cue["changes"];
	group_changes: NonNullable<Cue["group_changes"]>;
} {
	const cueOnly = cues.at(-1);
	if (!cueOnly?.cue_only) return { changes: [], group_changes: [] };
	const fixtureState = new Map<string, Cue["changes"][number]["value"]>();
	const groupState = new Map<
		string,
		NonNullable<Cue["group_changes"]>[number]["value"]
	>();
	for (const cue of cues.slice(0, -1)) {
		for (const change of cue.changes) {
			const key = `${change.fixture_id}\u0000${change.attribute}`;
			if (change.value == null) fixtureState.delete(key);
			else fixtureState.set(key, change.value);
		}
		for (const change of cue.group_changes ?? []) {
			const key = `${change.group_id}\u0000${change.attribute}`;
			if (change.value == null) groupState.delete(key);
			else groupState.set(key, change.value);
		}
	}
	const changes = cueOnly.changes
		.filter((change) => !change.automatic_restore)
		.map((change) => ({
			fixture_id: change.fixture_id,
			attribute: change.attribute,
			value:
				fixtureState.get(`${change.fixture_id}\u0000${change.attribute}`) ??
				null,
			automatic_restore: true,
		}));
	const group_changes = (cueOnly.group_changes ?? [])
		.filter((change) => !change.automatic_restore)
		.map((change) => ({
			group_id: change.group_id,
			attribute: change.attribute,
			value:
				groupState.get(`${change.group_id}\u0000${change.attribute}`) ?? null,
			automatic_restore: true,
		}));
	return { changes, group_changes };
}
