/**
 * How the operator arranges the CAD drawings: folders, and where each drawing sits among them.
 *
 * The arrangement is stored apart from the drawings, so a drawing it does not mention — just
 * placed, or placed in another window — shows at the top level, and a folder deleted with
 * drawings in it hands them to its own parent rather than losing them.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface DrawingFolder {
	id: string;
	name: string;
	/** The folder this one sits in; null is the top level. */
	parentId: string | null;
	/** Position among its siblings, folders and drawings alike. */
	order: number;
	collapsed: boolean;
}

export interface DrawingPlacement {
	parentId: string | null;
	order: number;
}

export interface DrawingTree {
	folders: DrawingFolder[];
	/** Keyed by the drawing's own ID. */
	items: Record<string, DrawingPlacement>;
}

/** A drawing as the tree lists it: a placed venue drawing or a drawn item. */
export interface DrawingLeaf {
	id: string;
	kind: "underlay" | "annotation";
	name: string;
	detail: string;
}

export type DrawingNode =
	| { type: "folder"; folder: DrawingFolder; children: DrawingNode[] }
	| { type: "drawing"; leaf: DrawingLeaf };

export const EMPTY_DRAWING_TREE: DrawingTree = { folders: [], items: {} };

export const drawingTreeSession = {
	get: () => invoke<DrawingTree>("cad_drawing_tree"),
	save: (tree: DrawingTree) =>
		invoke<DrawingTree>("save_cad_drawing_tree", { tree }),
	onDelta: (handler: (tree: DrawingTree) => void): Promise<UnlistenFn> =>
		listen<DrawingTree>("cad-drawing-tree-delta", (event) =>
			handler(event.payload),
		),
};

/** The folder a node sits in, reading an unknown or deleted folder as the top level. */
function parentOf(tree: DrawingTree, id: string, isFolder: boolean): string | null {
	const parentId = isFolder
		? (tree.folders.find((folder) => folder.id === id)?.parentId ?? null)
		: (tree.items[id]?.parentId ?? null);
	return parentId && tree.folders.some((folder) => folder.id === parentId)
		? parentId
		: null;
}

function orderOf(tree: DrawingTree, node: DrawingNode): number {
	return node.type === "folder"
		? node.folder.order
		: (tree.items[node.leaf.id]?.order ?? Number.POSITIVE_INFINITY);
}

function nameOf(node: DrawingNode): string {
	return node.type === "folder" ? node.folder.name : node.leaf.name;
}

/** The folders and drawings, nested and in the order the operator put them. */
export function buildDrawingTree(
	tree: DrawingTree,
	leaves: readonly DrawingLeaf[],
): DrawingNode[] {
	const children = (parentId: string | null): DrawingNode[] =>
		[
			...tree.folders
				.filter((folder) => parentOf(tree, folder.id, true) === parentId)
				.map(
					(folder): DrawingNode => ({
						type: "folder",
						folder,
						children: children(folder.id),
					}),
				),
			...leaves
				.filter((leaf) => parentOf(tree, leaf.id, false) === parentId)
				.map((leaf): DrawingNode => ({ type: "drawing", leaf })),
		].sort(
			(left, right) =>
				orderOf(tree, left) - orderOf(tree, right) ||
				nameOf(left).localeCompare(nameOf(right)),
		);
	return children(null);
}

export function addFolder(
	tree: DrawingTree,
	folder: Pick<DrawingFolder, "id" | "name" | "parentId">,
): DrawingTree {
	const siblings = tree.folders.filter((each) => each.parentId === folder.parentId);
	return {
		...tree,
		folders: [
			...tree.folders,
			{
				...folder,
				order: Math.max(-1, ...siblings.map((each) => each.order)) + 1,
				collapsed: false,
			},
		],
	};
}

export function renameFolder(tree: DrawingTree, id: string, name: string): DrawingTree {
	return {
		...tree,
		folders: tree.folders.map((folder) =>
			folder.id === id ? { ...folder, name } : folder,
		),
	};
}

export function toggleFolder(tree: DrawingTree, id: string): DrawingTree {
	return {
		...tree,
		folders: tree.folders.map((folder) =>
			folder.id === id ? { ...folder, collapsed: !folder.collapsed } : folder,
		),
	};
}

/** Delete a folder and hand what it held to its own parent. */
export function removeFolder(tree: DrawingTree, id: string): DrawingTree {
	const parentId = parentOf(tree, id, true);
	return {
		folders: tree.folders
			.filter((folder) => folder.id !== id)
			.map((folder) => (folder.parentId === id ? { ...folder, parentId } : folder)),
		items: Object.fromEntries(
			Object.entries(tree.items).map(([itemId, placement]) => [
				itemId,
				placement.parentId === id ? { ...placement, parentId } : placement,
			]),
		),
	};
}

/** Whether `id` is `ancestorId` or sits somewhere inside it. */
function isWithin(tree: DrawingTree, id: string | null, ancestorId: string): boolean {
	for (let current = id, steps = 0; current && steps <= tree.folders.length; steps++) {
		if (current === ancestorId) return true;
		current = parentOf(tree, current, true);
	}
	return false;
}

/**
 * Move a folder or a drawing to `index` among the children of `parentId`, renumbering that level so
 * the order is exactly what the operator sees. A folder cannot move into itself; that returns the
 * tree unchanged.
 */
export function moveNode(
	tree: DrawingTree,
	leaves: readonly DrawingLeaf[],
	node: { id: string; isFolder: boolean },
	parentId: string | null,
	index: number,
): DrawingTree {
	if (node.isFolder && parentId && isWithin(tree, parentId, node.id)) return tree;
	const findLevel = (nodes: DrawingNode[]): DrawingNode[] | null => {
		if (parentId === null) return nodes;
		for (const each of nodes) {
			if (each.type !== "folder") continue;
			if (each.folder.id === parentId) return each.children;
			const found = findLevel(each.children);
			if (found) return found;
		}
		return null;
	};
	const level = (findLevel(buildDrawingTree(tree, leaves)) ?? []).filter((each) =>
		each.type === "folder"
			? !(node.isFolder && each.folder.id === node.id)
			: !(!node.isFolder && each.leaf.id === node.id),
	);
	const siblings = level.map((each) =>
		each.type === "folder"
			? { id: each.folder.id, isFolder: true }
			: { id: each.leaf.id, isFolder: false },
	);
	siblings.splice(Math.max(0, Math.min(index, siblings.length)), 0, node);
	const folderOrder = new Map<string, number>();
	const items = { ...tree.items };
	siblings.forEach((sibling, order) => {
		if (sibling.isFolder) folderOrder.set(sibling.id, order);
		else items[sibling.id] = { parentId, order };
	});
	return {
		items,
		folders: tree.folders.map((folder) =>
			folderOrder.has(folder.id)
				? {
						...folder,
						order: folderOrder.get(folder.id) ?? folder.order,
						parentId: folder.id === node.id ? parentId : folder.parentId,
					}
				: folder,
		),
	};
}

/** Where a node is: the folder it sits in and its index there. */
export function positionOf(
	tree: DrawingTree,
	leaves: readonly DrawingLeaf[],
	node: { id: string; isFolder: boolean },
): { parentId: string | null; index: number; count: number } {
	const parentId = parentOf(tree, node.id, node.isFolder);
	const search = (nodes: DrawingNode[]): DrawingNode[] | null => {
		for (const each of nodes)
			if (each.type === "folder") {
				if (each.folder.id === parentId) return each.children;
				const found = search(each.children);
				if (found) return found;
			}
		return null;
	};
	const all = buildDrawingTree(tree, leaves);
	const level = parentId === null ? all : (search(all) ?? []);
	return {
		parentId,
		index: level.findIndex((each) =>
			each.type === "folder" ? each.folder.id === node.id : each.leaf.id === node.id,
		),
		count: level.length,
	};
}
