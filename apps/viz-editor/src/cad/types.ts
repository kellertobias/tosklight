export type CadViewDirection =
	| "top_down"
	| "left_to_right"
	| "right_to_left"
	| "front_to_back"
	| "back_to_front";

export interface CadEntity {
	id: string;
	name: string;
	fixtureNumber: number | null;
	kind: string;
	fixtureType: string;
	drawingId: string;
	positionMillimetres: [number, number, number];
	rotationDegrees: [number, number, number];
	sizeMillimetres: [number, number, number];
	outputDirection: [number, number, number];
}

export type CadProjectionView = "top" | "left" | "right" | "front" | "back";

export interface CadProjection {
	view: CadProjectionView;
	svg: string;
	viewBoxMillimetres: [number, number, number, number];
	originMillimetres: [number, number];
}

export interface CadDrawing {
	id: string;
	projections: CadProjection[];
}

export interface EntityTransform {
	id: string;
	positionMillimetres: [number, number, number];
	rotationDegrees: [number, number, number];
}

export interface RigAttachment {
	fixtureId: string;
	trussMemberId: string;
	mountingPointId: string;
	localTransform: EntityTransform;
}

export interface CadSceneSnapshot {
	showId: string;
	sceneRevision: number;
	selectionRevision: number;
	entities: CadEntity[];
	drawings: CadDrawing[];
	selectedIds: string[];
	attachments: RigAttachment[];
}

export interface CadSceneDelta {
	sceneRevision: number;
	upserted: CadEntity[];
	drawings: CadDrawing[];
	removedIds: string[];
	attachments: RigAttachment[];
}

export interface SelectionDelta {
	revision: number;
	selectedIds: string[];
}

export type SelectionChange =
	| { type: "replace"; ids: readonly string[] }
	| { type: "add"; ids: readonly string[] }
	| { type: "toggle"; ids: readonly string[] };

export interface CadTransformOutcome {
	sceneRevision: number;
	transforms: EntityTransform[];
	attachments: RigAttachment[];
}

export interface TileCamera {
	pan: [number, number];
	zoom: number;
}

export interface ViewportTile {
	type: "tile";
	id: string;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
}

export interface SplitTile {
	type: "split";
	id: string;
	direction: "horizontal" | "vertical";
	ratio: number;
	first: TileNode;
	second: TileNode;
}

export type TileNode = ViewportTile | SplitTile;

export type TileEdge = "left" | "right" | "top" | "bottom";

export const CAD_VIEW_LABELS: Record<CadViewDirection, string> = {
	top_down: "Top down",
	left_to_right: "Left to right",
	right_to_left: "Right to left",
	front_to_back: "Front to back",
	back_to_front: "Back to front",
};

export type WorldAxis = "x" | "y" | "z";

export interface ViewAxes {
	horizontal: { axis: WorldAxis; sign: 1 | -1 };
	vertical: { axis: WorldAxis; sign: 1 | -1 };
}

export function viewAxes(
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): ViewAxes {
	const base = baseViewAxes(view);
	if (view !== "top_down") return base;
	switch (normaliseQuarterTurns(rotationQuarterTurns)) {
		case 0:
			return base;
		case 1:
			return {
				horizontal: base.vertical,
				vertical: negateAxis(base.horizontal),
			};
		case 2:
			return {
				horizontal: negateAxis(base.horizontal),
				vertical: negateAxis(base.vertical),
			};
		case 3:
			return {
				horizontal: negateAxis(base.vertical),
				vertical: base.horizontal,
			};
	}
}

function baseViewAxes(view: CadViewDirection): ViewAxes {
	switch (view) {
		case "top_down":
			return {
				horizontal: { axis: "x", sign: 1 },
				vertical: { axis: "y", sign: -1 },
			};
		case "left_to_right":
			return {
				horizontal: { axis: "y", sign: 1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "right_to_left":
			return {
				horizontal: { axis: "y", sign: -1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "front_to_back":
			return {
				horizontal: { axis: "x", sign: 1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "back_to_front":
			return {
				horizontal: { axis: "x", sign: -1 },
				vertical: { axis: "z", sign: 1 },
			};
	}
}

function negateAxis(value: ViewAxes["horizontal"]): ViewAxes["horizontal"] {
	return { ...value, sign: value.sign === 1 ? -1 : 1 };
}

export function applySelectionChange(
	selectedIds: readonly string[],
	change: SelectionChange,
): string[] {
	if (change.type === "replace") return [...new Set(change.ids)];
	const next = new Set(selectedIds);
	for (const id of change.ids) {
		if (change.type === "toggle" && next.has(id)) next.delete(id);
		else next.add(id);
	}
	return [...next];
}

export function projectPoint(
	point: readonly [number, number, number],
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): [number, number] {
	const projected = projectPointUnrotated(point, view);
	if (view !== "top_down") return projected;
	return rotatePlane(projected, rotationQuarterTurns);
}

function projectPointUnrotated(
	point: readonly [number, number, number],
	view: CadViewDirection,
): [number, number] {
	switch (view) {
		case "top_down":
			return [point[0], -point[1]];
		case "left_to_right":
			return [point[1], point[2]];
		case "right_to_left":
			return [-point[1], point[2]];
		case "front_to_back":
			return [point[0], point[2]];
		case "back_to_front":
			return [-point[0], point[2]];
	}
}

export function planeDelta(
	delta: readonly [number, number],
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): [number, number, number] {
	const resolved =
		view === "top_down"
			? rotatePlane(delta, -normaliseQuarterTurns(rotationQuarterTurns))
			: delta;
	switch (view) {
		case "top_down":
			return [resolved[0], negate(resolved[1]), 0];
		case "left_to_right":
			return [0, resolved[0], resolved[1]];
		case "right_to_left":
			return [0, negate(resolved[0]), resolved[1]];
		case "front_to_back":
			return [resolved[0], 0, resolved[1]];
		case "back_to_front":
			return [negate(resolved[0]), 0, resolved[1]];
	}
}

function rotatePlane(
	point: readonly [number, number],
	rotationQuarterTurns: number,
): [number, number] {
	switch (normaliseQuarterTurns(rotationQuarterTurns)) {
		case 0:
			return [point[0], point[1]];
		case 1:
			return [point[1], -point[0]];
		case 2:
			return [-point[0], -point[1]];
		case 3:
			return [-point[1], point[0]];
	}
}

export function normaliseQuarterTurns(value: number): 0 | 1 | 2 | 3 {
	return (((Math.round(value) % 4) + 4) % 4) as 0 | 1 | 2 | 3;
}

function negate(value: number): number {
	return value === 0 ? 0 : -value;
}

export function newTile(view: CadViewDirection = "top_down"): ViewportTile {
	return {
		type: "tile",
		id: crypto.randomUUID(),
		view,
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.08 },
	};
}

export function mapTile(
	node: TileNode,
	id: string,
	change: (tile: ViewportTile) => TileNode,
): TileNode {
	if (node.type === "tile") return node.id === id ? change(node) : node;
	return {
		...node,
		first: mapTile(node.first, id, change),
		second: mapTile(node.second, id, change),
	};
}

export function splitTile(
	node: TileNode,
	id: string,
	direction: "horizontal" | "vertical",
): TileNode {
	return splitTileAtEdge(
		node,
		id,
		direction === "horizontal" ? "right" : "bottom",
	);
}

export function splitTileAtEdge(
	node: TileNode,
	id: string,
	edge: TileEdge,
): TileNode {
	return mapTile(node, id, (tile) => {
		const adjacent = newTile(tile.view);
		const adjacentFirst = edge === "left" || edge === "top";
		return {
			type: "split",
			id: crypto.randomUUID(),
			direction:
				edge === "left" || edge === "right" ? "horizontal" : "vertical",
			ratio: 0.5,
			first: adjacentFirst ? adjacent : tile,
			second: adjacentFirst ? tile : adjacent,
		};
	});
}

export function setSplitRatio(
	node: TileNode,
	id: string,
	ratio: number,
): TileNode {
	if (node.type === "tile") return node;
	if (node.id === id)
		return { ...node, ratio: Math.max(0.15, Math.min(0.85, ratio)) };
	return {
		...node,
		first: setSplitRatio(node.first, id, ratio),
		second: setSplitRatio(node.second, id, ratio),
	};
}
