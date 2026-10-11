import type {
	PlaybackButtonAction,
	PlaybackDefinition,
	PlaybackPage,
	PlaybackSurfaceLayout,
	PlaybackSurfaceRow,
} from "../../../api/types";
import type { AssignedPlayback } from "./types";

export type PlaybackFootprint =
	| { type: "normal" }
	| { type: "taller"; upper_button: PlaybackButtonAction }
	| {
			type: "wider";
			right_buttons: PlaybackDefinition["buttons"];
			right_fader: PlaybackDefinition["fader"];
	  };

export type PlaybackFootprintFallbackReason =
	| "upper_neighbor_out_of_range"
	| "upper_neighbor_incompatible"
	| "right_neighbor_out_of_range"
	| "neighbor_occupied"
	| "conflicting_claim";

export type PlaybackFootprintControl =
	| { type: "anchor_button"; number: number }
	| { type: "anchor_fader" }
	| { type: "taller_button" }
	| { type: "right_button"; number: number }
	| { type: "right_fader" };

export interface PlaybackPhysicalControlBinding {
	physicalButton?: number;
	control: PlaybackFootprintControl;
}

export interface PlaybackPhysicalSlotBinding {
	physicalSlot: number;
	anchorSlot: number;
	address: import("../../../api/playbackAddress").PlaybackAddressValue;
	position: "anchor" | "taller_upper" | "wider_right";
	controls: PlaybackPhysicalControlBinding[];
}

export interface PlaybackFootprintGridCell {
	slot: number;
	row: PlaybackSurfaceRow;
	rowIndex: number;
	columnIndex: number;
}

export type PlaybackFootprintCellProjection = PlaybackFootprintGridCell &
	(
		| {
				state: "anchor";
				playback: AssignedPlayback;
				requested: PlaybackFootprint["type"];
				effective: PlaybackFootprint["type"];
				claimedSlots: number[];
				fallbackReason: PlaybackFootprintFallbackReason | null;
				rowStart: number;
				columnStart: number;
				rowSpan: 1 | 2;
				columnSpan: 1 | 2;
		  }
		| {
				state: "claimed";
				anchorSlot: number;
				address: import("../../../api/playbackAddress").PlaybackAddressValue;
				position: "taller_upper" | "wider_right";
		  }
		| { state: "unclaimed" }
	);

export interface PlaybackFootprintProjection {
	cells: PlaybackFootprintCellProjection[];
	anchors: Extract<PlaybackFootprintCellProjection, { state: "anchor" }>[];
	claimed: Extract<PlaybackFootprintCellProjection, { state: "claimed" }>[];
	unclaimed: Extract<PlaybackFootprintCellProjection, { state: "unclaimed" }>[];
	bindings: Map<number, PlaybackPhysicalSlotBinding>;
}

export interface ProjectPlaybackFootprintsOptions {
	page: PlaybackPage | undefined;
	playbackLayout?: PlaybackSurfaceLayout | null;
	columns: number;
	firstSlot: number;
	pageSize: number;
	fallbackButtons?: number;
	fallbackHasFader?: boolean;
}

type Candidate = {
	anchor: PlaybackFootprintGridCell;
	playback: AssignedPlayback;
	requested: PlaybackFootprint["type"];
	target: PlaybackFootprintGridCell | null;
	position: "taller_upper" | "wider_right" | null;
	fallbackReason: PlaybackFootprintFallbackReason | null;
};

/**
 * Resolves show-persisted Playback footprints against one concrete physical surface.
 * This function never changes a Playback definition: unsupported footprints project as
 * ordinary anchor cells so their dormant control assignments can return on another surface.
 */
export function projectPlaybackFootprints({
	page,
	playbackLayout,
	columns,
	firstSlot,
	pageSize,
	fallbackButtons = 3,
	fallbackHasFader = true,
}: ProjectPlaybackFootprintsOptions): PlaybackFootprintProjection {
	const cells = footprintGridCells({
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
		fallbackButtons,
		fallbackHasFader,
	});
	const cellByCoordinate = new Map(
		cells.map((cell) => [coordinate(cell.rowIndex, cell.columnIndex), cell]),
	);
	const playbackAt = (cell: PlaybackFootprintGridCell): AssignedPlayback | null => {
		const definition = page?.assignments[String(cell.slot)];
		return definition && page ? {...definition, address: {page: page.number, position: cell.slot}} : null;
	};
	const isOccupied = (cell: PlaybackFootprintGridCell) => page?.assignments[String(cell.slot)] != null;
	const candidates = cells.flatMap((cell) => {
		const playback = playbackAt(cell);
		return playback
			? [candidateFor(cell, playback, cellByCoordinate, isOccupied)]
			: [];
	});
	const claimsByTarget = new Map<number, Candidate[]>();
	for (const candidate of candidates) {
		if (!candidate.target || candidate.fallbackReason) continue;
		const claims = claimsByTarget.get(candidate.target.slot) ?? [];
		claims.push(candidate);
		claimsByTarget.set(candidate.target.slot, claims);
	}
	for (const claims of claimsByTarget.values()) {
		if (claims.length < 2) continue;
		for (const candidate of claims)
			candidate.fallbackReason = "conflicting_claim";
	}

	const claimedBySlot = new Map<number, Candidate>();
	for (const candidate of candidates) {
		if (candidate.target && !candidate.fallbackReason)
			claimedBySlot.set(candidate.target.slot, candidate);
	}
	const candidateByAnchor = new Map(
		candidates.map((candidate) => [candidate.anchor.slot, candidate]),
	);
	const bindings = new Map<number, PlaybackPhysicalSlotBinding>();
	const projected = cells.map((cell): PlaybackFootprintCellProjection => {
		const claim = claimedBySlot.get(cell.slot);
		if (claim?.position) {
			bindings.set(
				cell.slot,
				claimedBinding(cell, claim.anchor.slot, claim.playback, claim.position),
			);
			return {
				...cell,
				state: "claimed",
				anchorSlot: claim.anchor.slot,
				address: {...claim.playback.address},
				position: claim.position,
			};
		}
		const candidate = candidateByAnchor.get(cell.slot);
		if (!candidate) return { ...cell, state: "unclaimed" };
		const effective = candidate.fallbackReason ? "normal" : candidate.requested;
		const target = effective === "normal" ? null : candidate.target;
		bindings.set(cell.slot, anchorBinding(cell, candidate.playback));
		return {
			...cell,
			state: "anchor",
			playback: candidate.playback,
			requested: candidate.requested,
			effective,
			claimedSlots: target ? [target.slot] : [],
			fallbackReason: candidate.fallbackReason,
			rowStart: effective === "taller" ? cell.rowIndex : cell.rowIndex + 1,
			columnStart: cell.columnIndex + 1,
			rowSpan: effective === "taller" ? 2 : 1,
			columnSpan: effective === "wider" ? 2 : 1,
		};
	});
	const anchors = projected.filter(
		(
			cell,
		): cell is Extract<PlaybackFootprintCellProjection, { state: "anchor" }> =>
			cell.state === "anchor",
	);
	const claimed = projected.filter(
		(
			cell,
		): cell is Extract<PlaybackFootprintCellProjection, { state: "claimed" }> =>
			cell.state === "claimed",
	);
	const unclaimed = projected.filter(
		(
			cell,
		): cell is Extract<
			PlaybackFootprintCellProjection,
			{ state: "unclaimed" }
		> => cell.state === "unclaimed",
	);
	return { cells: projected, anchors, claimed, unclaimed, bindings };
}

export function footprintGridCells({
	playbackLayout,
	columns,
	firstSlot,
	pageSize,
	fallbackButtons = 3,
	fallbackHasFader = true,
}: Omit<ProjectPlaybackFootprintsOptions, "page">) {
	if (playbackLayout) {
		return playbackLayout.rows.flatMap((row, rowIndex) =>
			Array.from(
				{ length: row.count },
				(_, columnIndex) => ({
					slot: row.first_position + columnIndex,
					row,
					rowIndex,
					columnIndex,
				}),
			),
		);
	}
	const safeColumns = Math.max(1, columns);
	return Array.from({ length: pageSize }, (_, index) => {
		const rowIndex = Math.floor(index / safeColumns);
		return {
			slot: firstSlot + index,
			row: {
				first_position: firstSlot + rowIndex * safeColumns,
				count: safeColumns,
				presentation: (fallbackHasFader ? "fader" : "buttons") as PlaybackSurfaceRow["presentation"],
				button_count: fallbackButtons,
			},
			rowIndex,
			columnIndex: index % safeColumns,
		};
	});
}

function candidateFor(
	anchor: PlaybackFootprintGridCell,
	playback: AssignedPlayback,
	cells: ReadonlyMap<string, PlaybackFootprintGridCell>,
	isOccupied: (cell: PlaybackFootprintGridCell) => boolean,
): Candidate {
	const requested = footprintOf(playback).type;
	if (requested === "normal")
		return {
			anchor,
			playback,
			requested,
			target: null,
			position: null,
			fallbackReason: null,
		};
	const target =
		requested === "taller"
			? (cells.get(coordinate(anchor.rowIndex - 1, anchor.columnIndex)) ?? null)
			: (cells.get(coordinate(anchor.rowIndex, anchor.columnIndex + 1)) ??
				null);
	if (!target)
		return {
			anchor,
			playback,
			requested,
			target: null,
			position: null,
			fallbackReason:
				requested === "taller"
					? "upper_neighbor_out_of_range"
					: "right_neighbor_out_of_range",
		};
	if (requested === "taller" && target.row.button_count < 1)
		return {
			anchor,
			playback,
			requested,
			target,
			position: "taller_upper",
			fallbackReason: "upper_neighbor_incompatible",
		};
	return {
		anchor,
		playback,
		requested,
		target,
		position: requested === "taller" ? "taller_upper" : "wider_right",
		fallbackReason: isOccupied(target) ? "neighbor_occupied" : null,
	};
}

function footprintOf(playback: PlaybackDefinition): PlaybackFootprint {
	const value = (playback as PlaybackDefinition & { footprint?: unknown })
		.footprint;
	if (!value || typeof value !== "object") return { type: "normal" };
	const type = (value as { type?: unknown }).type;
	if (type === "taller")
		return {
			type,
			upper_button: (value as { upper_button: PlaybackButtonAction })
				.upper_button,
		};
	if (type === "wider")
		return {
			type,
			right_buttons: (value as { right_buttons: PlaybackDefinition["buttons"] })
				.right_buttons,
			right_fader: (value as { right_fader: PlaybackDefinition["fader"] })
				.right_fader,
		};
	return { type: "normal" };
}

function anchorBinding(
	cell: PlaybackFootprintGridCell,
	playback: AssignedPlayback,
): PlaybackPhysicalSlotBinding {
	const controls: PlaybackPhysicalControlBinding[] = Array.from(
		{ length: Math.min(cell.row.button_count, playback.button_count ?? 3) },
		(_, index) => ({
			physicalButton: index + 1,
			control: { type: "anchor_button", number: index + 1 },
		}),
	);
	if ((cell.row.presentation !== "buttons") && (playback.has_fader ?? true))
		controls.push({ control: { type: "anchor_fader" } });
	return {
		physicalSlot: cell.slot,
		anchorSlot: cell.slot,
		address: {...playback.address},
		position: "anchor",
		controls,
	};
}

function claimedBinding(
	cell: PlaybackFootprintGridCell,
	anchorSlot: number,
	playback: AssignedPlayback,
	position: "taller_upper" | "wider_right",
): PlaybackPhysicalSlotBinding {
	const controls: PlaybackPhysicalControlBinding[] =
		position === "taller_upper"
			? [{ physicalButton: 1, control: { type: "taller_button" } }]
			: Array.from(
					{
						length: Math.min(cell.row.button_count, playback.button_count ?? 3),
					},
					(_, index) => ({
						physicalButton: index + 1,
						control: { type: "right_button" as const, number: index + 1 },
					}),
				);
	if (position === "wider_right" && (cell.row.presentation !== "buttons"))
		controls.push({ control: { type: "right_fader" } });
	return {
		physicalSlot: cell.slot,
		anchorSlot,
		address: {...playback.address},
		position,
		controls,
	};
}

function coordinate(row: number, column: number) {
	return `${row}:${column}`;
}
