/**
 * The Elements panel's Drawings tab: every drawing in folders the operator arranges freely.
 *
 * Rows drag onto a folder to go inside it, or onto another row to go before it. The selected row also
 * carries its moves as buttons — up, down, and out of the folder it is in — and a folder its rename and
 * delete. New folders come from the side panel's **+**.
 */
import { Button } from "@tosklight/ui";
import { type ReactNode, useEffect, useRef, useState } from "react";
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
	/** The selected row's own buttons. */
	rowActions: ReactNode;
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
				{isSelected && context.rowActions ? context.rowActions : null}
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

/** The selected row's moves, and a folder's rename and delete. */
function RowActions({
	tree,
	leaves,
	selected,
	onSelect,
	onChange,
	onRename,
}: {
	tree: DrawingTree;
	leaves: readonly DrawingLeaf[];
	selected: Selected;
	onSelect(selected: Selected | null): void;
	onChange(tree: DrawingTree): void;
	onRename(id: string): void;
}) {
	const position = positionOf(tree, leaves, selected);
	const move = (parentId: string | null, index: number) =>
		onChange(moveNode(tree, leaves, selected, parentId, index));
	const stop = (event: { stopPropagation(): void }) => event.stopPropagation();
	return (
		<span className="cad-drawing-tree-row-actions" onClick={stop} onPointerDown={stop}>
			{selected.isFolder ? (
				<Button size="compact" aria-label="Rename folder" onClick={() => onRename(selected.id)}>
					✎
				</Button>
			) : null}
			<Button
				size="compact"
				aria-label="Move up"
				disabled={position.index <= 0}
				onClick={() => move(position.parentId, position.index - 1)}
			>
				↑
			</Button>
			<Button
				size="compact"
				aria-label="Move down"
				disabled={position.index >= position.count - 1}
				onClick={() => move(position.parentId, position.index + 1)}
			>
				↓
			</Button>
			<Button
				size="compact"
				aria-label="Move out of folder"
				disabled={!position.parentId}
				onClick={() => {
					if (!position.parentId) return;
					const outer = positionOf(tree, leaves, { id: position.parentId, isFolder: true });
					move(outer.parentId, outer.index + 1);
				}}
			>
				⇤
			</Button>
			{selected.isFolder ? (
				<Button
					size="compact"
					aria-label="Delete folder"
					onClick={() => {
						onChange(removeFolder(tree, selected.id));
						onSelect(null);
					}}
				>
					✕
				</Button>
			) : null}
		</span>
	);
}

export function CadDrawingTree({
	tree,
	leaves,
	selected,
	onSelect,
	onChange,
	newId = () => crypto.randomUUID(),
	newFolderRequest = 0,
	children,
}: {
	tree: DrawingTree;
	leaves: readonly DrawingLeaf[];
	selected: Selected | null;
	onSelect(selected: Selected | null): void;
	onChange(tree: DrawingTree): void;
	newId?: () => string;
	/** Counts presses of the side panel's New folder; each adds one where the selection is. */
	newFolderRequest?: number;
	/** What the selected drawing can be changed by, shown under the tree. */
	children?: ReactNode;
}) {
	const [renaming, setRenaming] = useState<string | null>(null);
	const handledFolderRequest = useRef(newFolderRequest);
	useEffect(() => {
		if (newFolderRequest === handledFolderRequest.current) return;
		handledFolderRequest.current = newFolderRequest;
		const id = newId();
		const parentId = selected?.isFolder ? selected.id : null;
		onChange(addFolder(tree, { id, name: "New folder", parentId }));
		onSelect({ id, isFolder: true });
		setRenaming(id);
	});
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
		rowActions: selected ? (
			<RowActions
				tree={tree}
				leaves={leaves}
				selected={selected}
				onSelect={onSelect}
				onChange={onChange}
				onRename={setRenaming}
			/>
		) : null,
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
				<p>No drawings yet. Import a DXF or SVG with +, or draw on a view with the title tools.</p>
			)}
			{children}
		</div>
	);
}
