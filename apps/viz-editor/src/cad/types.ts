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
	positionMillimetres: [number, number, number];
	rotationDegrees: [number, number, number];
	sizeMillimetres: [number, number, number];
	outputDirection: [number, number, number];
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
	selectedIds: string[];
	attachments: RigAttachment[];
}

export interface CadSceneDelta {
	sceneRevision: number;
	upserted: CadEntity[];
	removedIds: string[];
	attachments: RigAttachment[];
}

export interface SelectionDelta {
	revision: number;
	selectedIds: string[];
}

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

export function projectPoint(
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
): [number, number, number] {
	switch (view) {
		case "top_down":
			return [delta[0], -delta[1], 0];
		case "left_to_right":
			return [0, delta[0], delta[1]];
		case "right_to_left":
			return [0, -delta[0], delta[1]];
		case "front_to_back":
			return [delta[0], 0, delta[1]];
		case "back_to_front":
			return [-delta[0], 0, delta[1]];
	}
}

export function newTile(view: CadViewDirection = "top_down"): ViewportTile {
	return {
		type: "tile",
		id: crypto.randomUUID(),
		view,
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
		direction: edge === "left" || edge === "right" ? "horizontal" : "vertical",
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
	if (node.id === id) return { ...node, ratio: Math.max(0.15, Math.min(0.85, ratio)) };
	return {
		...node,
		first: setSplitRatio(node.first, id, ratio),
		second: setSplitRatio(node.second, id, ratio),
	};
}
