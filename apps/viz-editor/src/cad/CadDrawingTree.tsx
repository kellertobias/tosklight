/**
 * The Elements panel's Drawings tab: every drawing in folders the operator arranges freely.
 *
 * Rows drag onto a folder to go inside it, or onto another row to go before it. Because a desk
 * surface is touched rather than dragged with a mouse, every move is also a button: up, down, and
 * out of the folder it is in.
 */
import { Button } from "@tosklight/ui";
import { type ReactNode, useState } from "react";
import {
	addFolder,
	buildDrawingTree,
	type DrawingLeaf,
	type DrawingNode,
	type DrawingTree,
	moveNode,
	positionOf,
	removeFolder,
	renameFolder,
	toggleFolder,
} from "./drawingTree";

interface Selected {
	id: string;
	isFolder: boolean;
}

function nodeKey(node: DrawingNode): Selected {
	return node.type === "folder"
		? { id: node.folder.id, isFolder: true }
		: { id: node.leaf.id, isFolder: false };
}

/** What every row needs from the tree around it. */
interface RowContext {
	tree: DrawingTree;
	selected: Selected | null;
	renaming: string | null;
	onSelect(selected: Selected): void;
	onChange(tree: DrawingTree): void;
	onRenamed(): void;
	onDragStart(node: Selected): void;
	onDrop(target: DrawingNode): void;
}

function FolderName({
	node,
	context,
}: {
	node: Extract<DrawingNode, { type: "folder" }>;
	context: RowContext;
}) {
	const { folder } = node;
	if (context.renaming !== folder.id)
		return <span className="cad-drawing-tree-name">{folder.name}</span>;
	return (
		<input
			aria-label="Folder name"
			defaultValue={folder.name}
			// Renaming was asked for; the field is where the typing goes.
			// biome-ignore lint/a11y/noAutofocus: explicit rename
			autoFocus
			onClick={(event) => event.stopPropagation()}
			onKeyDown={(event) => {
				if (event.key === "Escape") context.onRenamed();
				if (event.key !== "Enter") return;
				const name = event.currentTarget.value.trim();
				if (name) context.onChange(renameFolder(context.tree, folder.id, name));
				context.onRenamed();
			}}
		/>
	);
}

function TreeRow({
	node,
	depth,
	context,
}: {
	node: DrawingNode;
	depth: number;
	context: RowContext;
}) {
	const key = nodeKey(node);
	const { selected } = context;
	const isSelected = selected?.id === key.id && selected.isFolder === key.isFolder;
	return (
		<li
			role="treeitem"
			aria-selected={isSelected}
			aria-expanded={node.type === "folder" ? !node.folder.collapsed : undefined}
		>
			<div
				className={`cad-drawing-tree-row ${isSelected ? "is-selected" : ""}`.trim()}
				style={{ paddingLeft: `${8 + depth * 18}px` }}
				draggable={context.renaming !== key.id}
				onDragStart={() => context.onDragStart(key)}
				onDragOver={(event) => event.preventDefault()}
				onDrop={(event) => {
					event.preventDefault();
					context.onDrop(node);
				}}
				onClick={() => context.onSelect(key)}
			>
				{node.type === "folder" ? (
					<>
						<button
							type="button"
							className="cad-drawing-tree-disclosure"
							aria-label={`${node.folder.collapsed ? "Open" : "Close"} ${node.folder.name}`}
							onClick={(event) => {
								event.stopPropagation();
								context.onChange(toggleFolder(context.tree, node.folder.id));
							}}
						>
							{node.folder.collapsed ? "▸" : "▾"}
						</button>
						<FolderName node={node} context={context} />
					</>
				) : (
					<>
						<span className="cad-drawing-tree-disclosure" aria-hidden="true" />
						<span className="cad-drawing-tree-name">{node.leaf.name}</span>
						<small className="cad-drawing-tree-detail">{node.leaf.detail}</small>
					</>
				)}
			</div>
			{node.type === "folder" && !node.folder.collapsed && node.children.length ? (
				<ul role="group">
					{node.children.map((child) => (
						<TreeRow
							key={`${child.type}:${nodeKey(child).id}`}
							node={child}
							depth={depth + 1}
							context={context}
						/>
					))}
				</ul>
			) : null}
		</li>
	);
}

/** New folder, rename, and the touch moves for whatever is selected. */
function TreeActions({
	tree,
	leaves,
	selected,
	newId,
	onSelect,
	onChange,
	onRename,
}: {
	tree: DrawingTree;
	leaves: readonly DrawingLeaf[];
	selected: Selected | null;
	newId: () => string;
	onSelect(selected: Selected | null): void;
	onChange(tree: DrawingTree): void;
	onRename(id: string): void;
}) {
	const position = selected ? positionOf(tree, leaves, selected) : null;
	const folderId = selected?.isFolder ? selected.id : null;
	const move = (parentId: string | null, index: number) =>
		selected && onChange(moveNode(tree, leaves, selected, parentId, index));
	return (
		<div className="cad-drawing-tree-actions">
			<Button
				onClick={() => {
					const id = newId();
					onChange(addFolder(tree, { id, name: "New folder", parentId: folderId }));
					onSelect({ id, isFolder: true });
					onRename(id);
				}}
			>
				New folder
			</Button>
			<Button disabled={!folderId} onClick={() => folderId && onRename(folderId)}>
				Rename
			</Button>
			<Button
				aria-label="Move up"
				disabled={!position || position.index <= 0}
				onClick={() => position && move(position.parentId, position.index - 1)}
			>
				↑
			</Button>
			<Button
				aria-label="Move down"
				disabled={!position || position.index >= position.count - 1}
				onClick={() => position && move(position.parentId, position.index + 1)}
			>
				↓
			</Button>
			<Button
				aria-label="Move out of folder"
				disabled={!position?.parentId}
				onClick={() => {
					if (!position?.parentId) return;
					const outer = positionOf(tree, leaves, { id: position.parentId, isFolder: true });
					move(outer.parentId, outer.index + 1);
				}}
			>
				⇤
			</Button>
			<Button
				disabled={!folderId}
				onClick={() => {
					if (!folderId) return;
					onChange(removeFolder(tree, folderId));
					onSelect(null);
				}}
			>
				Delete folder
			</Button>
		</div>
	);
}

export function CadDrawingTree({
	tree,
	leaves,
	selected,
	onSelect,
	onChange,
	newId = () => crypto.randomUUID(),
	children,
}: {
	tree: DrawingTree;
	leaves: readonly DrawingLeaf[];
	selected: Selected | null;
	onSelect(selected: Selected | null): void;
	onChange(tree: DrawingTree): void;
	newId?: () => string;
	/** What the selected drawing can be changed by, shown under the tree. */
	children?: ReactNode;
}) {
	const [renaming, setRenaming] = useState<string | null>(null);
	const [dragging, setDragging] = useState<Selected | null>(null);
	const nodes = buildDrawingTree(tree, leaves);

	function dropOn(node: Selected, parentId: string | null, index: number) {
		setDragging(null);
		onChange(moveNode(tree, leaves, node, parentId, index));
	}

	const context: RowContext = {
		tree,
		selected,
		renaming,
		onSelect,
		onChange,
		onRenamed: () => setRenaming(null),
		onDragStart: setDragging,
		onDrop: (target) => {
			if (!dragging) return;
			if (target.type === "folder")
				return dropOn(dragging, target.folder.id, target.children.length);
			const at = positionOf(tree, leaves, nodeKey(target));
			dropOn(dragging, at.parentId, at.index);
		},
	};

	return (
		<div className="cad-drawing-tree">
			<TreeActions
				tree={tree}
				leaves={leaves}
				selected={selected}
				newId={newId}
				onSelect={onSelect}
				onChange={onChange}
				onRename={setRenaming}
			/>
			{nodes.length ? (
				<ul
					className="cad-drawing-tree-list"
					role="tree"
					aria-label="Drawings"
					onDragOver={(event) => event.preventDefault()}
					onDrop={(event) => {
						// A drop on empty space below the rows goes to the end of the top level.
						if (event.target === event.currentTarget && dragging)
							dropOn(dragging, null, nodes.length);
					}}
				>
					{nodes.map((node) => (
						<TreeRow
							key={`${node.type}:${nodeKey(node).id}`}
							node={node}
							depth={0}
							context={context}
						/>
					))}
				</ul>
			) : (
				<p>No drawings yet. Place a DXF or SVG, or draw on a view with the title tools.</p>
			)}
			{children}
		</div>
	);
}
