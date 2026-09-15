import { describe, expect, it } from "vitest";
import {
	addFolder,
	buildDrawingTree,
	type DrawingLeaf,
	type DrawingNode,
	EMPTY_DRAWING_TREE,
	moveNode,
	positionOf,
	removeFolder,
	renameFolder,
	toggleFolder,
} from "./drawingTree";

const leaves: DrawingLeaf[] = [
	{ id: "plan", kind: "underlay", name: "Ground plan", detail: "Top down · DXF" },
	{ id: "section", kind: "underlay", name: "Section", detail: "Front to back · SVG" },
	{ id: "foh", kind: "annotation", name: "Text “FOH”", detail: "Top down" },
];

/** The tree as names, nested, so a test reads like the panel. */
function outline(nodes: DrawingNode[]): unknown[] {
	return nodes.map((node) =>
		node.type === "folder"
			? { [node.folder.name]: outline(node.children) }
			: node.leaf.name,
	);
}

describe("the drawings tree", () => {
	it("lists every drawing at the top level when nothing has been arranged", () => {
		expect(outline(buildDrawingTree(EMPTY_DRAWING_TREE, leaves))).toEqual([
			"Ground plan",
			"Section",
			"Text “FOH”",
		]);
	});

	it("nests drawings in folders and keeps the order the operator put them in", () => {
		let tree = addFolder(EMPTY_DRAWING_TREE, { id: "venue", name: "Venue", parentId: null });
		tree = addFolder(tree, { id: "floor", name: "Floor", parentId: "venue" });
		tree = moveNode(tree, leaves, { id: "plan", isFolder: false }, "floor", 0);
		tree = moveNode(tree, leaves, { id: "foh", isFolder: false }, null, 0);
		expect(outline(buildDrawingTree(tree, leaves))).toEqual([
			"Text “FOH”",
			{ Venue: [{ Floor: ["Ground plan"] }] },
			"Section",
		]);
		expect(positionOf(tree, leaves, { id: "plan", isFolder: false })).toEqual({
			parentId: "floor",
			index: 0,
			count: 1,
		});
	});

	it("refuses to put a folder inside itself", () => {
		let tree = addFolder(EMPTY_DRAWING_TREE, { id: "venue", name: "Venue", parentId: null });
		tree = addFolder(tree, { id: "floor", name: "Floor", parentId: "venue" });
		expect(moveNode(tree, leaves, { id: "venue", isFolder: true }, "floor", 0)).toBe(tree);
	});

	it("renames and collapses a folder, and hands a deleted folder's drawings to its parent", () => {
		let tree = addFolder(EMPTY_DRAWING_TREE, { id: "venue", name: "Venue", parentId: null });
		tree = addFolder(tree, { id: "floor", name: "Floor", parentId: "venue" });
		tree = moveNode(tree, leaves, { id: "plan", isFolder: false }, "floor", 0);
		tree = toggleFolder(renameFolder(tree, "venue", "Hall"), "venue");
		expect(tree.folders.find((folder) => folder.id === "venue")).toMatchObject({
			name: "Hall",
			collapsed: true,
		});
		tree = removeFolder(tree, "floor");
		expect(outline(buildDrawingTree(tree, leaves))).toEqual([
			{ Hall: ["Ground plan"] },
			"Section",
			"Text “FOH”",
		]);
	});

	it("shows a drawing whose folder no longer exists at the top level", () => {
		const tree = { folders: [], items: { plan: { parentId: "gone", order: 0 } } };
		expect(outline(buildDrawingTree(tree, leaves))).toContain("Ground plan");
	});
});
