import type { ReactNode } from "react";
import { Button } from "../../components/common";
import { useFiles } from "../../features/files/FilesContext";
import {
	extension,
	FileProperties,
	formatSize,
	formatTime,
	imageExtensions,
	itemIcon,
	pickerSelectionIsValid,
	RasterThumbnail,
	rootIcon,
	textExtensions,
	validItemName,
} from "./fileUtilities";
import type { FileManagerController } from "./useFileManagerController";

function FileManagerToolbar({
	controller,
}: {
	controller: FileManagerController;
}) {
	const { state, navigation, operations, picker } = controller;
	return (
		<>
			<div className="file-toolbar">
				<Button
					className="file-navigation-toggle"
					active={state.sidePanel === "navigation"}
					onClick={() =>
						state.setSidePanel((value) =>
							value === "navigation" ? "none" : "navigation",
						)
					}
				>
					Navigation
				</Button>
				<nav aria-label="Breadcrumb">
					<Button
						variant="ghost"
						onClick={() =>
							navigation.rootId &&
							navigation.navigate({ rootId: navigation.rootId, path: "" })
						}
					>
						{navigation.currentRoot?.label ?? "Location"}
					</Button>
					{controller.breadcrumbs.map((part, index) => (
						<Button
							variant="ghost"
							key={`${part}-${index}`}
							onClick={() =>
								navigation.navigate({
									rootId: navigation.rootId,
									path: controller.breadcrumbs.slice(0, index + 1).join("/"),
								})
							}
						>
							/ {part}
						</Button>
					))}
				</nav>
				<Button
					className="file-info-toggle"
					active={state.sidePanel === "info"}
					onClick={() => {
						state.setPropertiesVisible(true);
						state.setSidePanel((value) => (value === "info" ? "none" : "info"));
					}}
				>
					Info
				</Button>
				{state.operation && (
					<div
						className="file-operation-actions"
						aria-label={`${controller.operationLabel} operation`}
						role="toolbar"
					>
						{(state.operation.kind === "copy" ||
							state.operation.kind === "move") && (
							<Button
								variant="primary"
								disabled={!state.operation.sources.length || state.busy}
								onClick={() => void operations.completeOperation()}
							>
								{controller.operationLabel} Here
							</Button>
						)}
						{state.operation.kind === "rename" && (
							<Button
								variant="primary"
								disabled={
									!state.operation.sources.length ||
									!validItemName(state.operation.renameDraft) ||
									state.busy
								}
								onClick={() => void operations.completeOperation()}
							>
								Rename
							</Button>
						)}
						{state.operation.kind === "delete" && (
							<Button
								variant="primary"
								disabled={!state.operation.sources.length || state.busy}
								onClick={() => void operations.completeOperation()}
							>
								Delete
							</Button>
						)}
						<Button
							disabled={state.busy}
							onClick={() => operations.cancelOperation()}
						>
							Cancel
						</Button>
					</div>
				)}
				{picker && !state.operation && (
					<div className="file-picker-actions">
						<Button
							variant="primary"
							disabled={!controller.pickerValid}
							onClick={() =>
								controller.pickerValid && picker.onSelect(state.selected)
							}
						>
							{picker.selectLabel ?? "Select"}
						</Button>
						{!picker.hideCancel && (
							<Button onClick={picker.onCancel}>
								{picker.cancelLabel ?? "Cancel"}
							</Button>
						)}
					</div>
				)}
			</div>
			{(state.message || state.busy || state.operation) && (
				<div
					className={`file-message ${state.busy ? "is-busy" : ""}`}
					role="status"
				>
					{state.busy
						? "Working…"
						: state.message ||
							`${controller.operationLabel}: ${state.operation?.sources.length ?? 0} source item(s) selected.`}
				</div>
			)}
		</>
	);
}

function TreeFolders({
	controller,
	treeRootId,
	path,
	depth,
}: {
	controller: FileManagerController;
	treeRootId: string;
	path: string;
	depth: number;
}): ReactNode {
	const { state, navigation } = controller;
	const key = `${treeRootId}:${path}`;
	if (!state.treeExpanded.has(key)) return null;
	const children = state.treeChildren[key];
	if (!children)
		return (
			<span className="file-tree-loading" role="status">
				Loading…
			</span>
		);
	return children.map((folder) => {
		const childKey = `${treeRootId}:${folder.path}`;
		const expanded = state.treeExpanded.has(childKey);
		return (
			<div className="file-tree-branch" key={childKey}>
				<Button
					variant="ghost"
					className={
						treeRootId === navigation.rootId &&
						folder.path === navigation.currentPath
							? "active"
							: ""
					}
					style={{ paddingInlineStart: `${0.4 + depth * 0.8}rem` }}
					aria-expanded={expanded}
					onClick={() => {
						navigation.navigate({ rootId: treeRootId, path: folder.path });
						void navigation.loadTreeFolder({
							rootId: treeRootId,
							path: folder.path,
						});
					}}
				>
					<span aria-hidden="true">{expanded ? "▾" : "▸"} 📁</span>{" "}
					{folder.name}
				</Button>
				<TreeFolders
					controller={controller}
					treeRootId={treeRootId}
					path={folder.path}
					depth={depth + 1}
				/>
			</div>
		);
	});
}

function FileRoots({ controller }: { controller: FileManagerController }) {
	const { state, navigation } = controller;
	return (
		<aside className="file-roots" aria-label="Folder navigation">
			<h3>Locations</h3>
			{state.roots.map((root) => {
				const key = `${root.id}:`;
				const expanded = state.treeExpanded.has(key);
				return (
					<div className="file-tree-root" key={root.id}>
						<Button
							variant="ghost"
							className={
								root.id === navigation.rootId && !navigation.currentPath
									? "active"
									: ""
							}
							aria-expanded={expanded}
							onClick={() => {
								navigation.navigate({ rootId: root.id, path: "" });
								void navigation.loadTreeFolder({ rootId: root.id, path: "" });
							}}
						>
							<span aria-hidden="true">
								{expanded ? "▾" : "▸"} {rootIcon(root)}
							</span>{" "}
							{root.label}
							{root.removable ? " (Removable)" : ""}
						</Button>
						<TreeFolders
							controller={controller}
							treeRootId={root.id}
							path=""
							depth={1}
						/>
					</div>
				);
			})}
			{!state.roots.length && (
				<p>No configured or removable locations are available.</p>
			)}
		</aside>
	);
}

function DirectoryContents({
	controller,
}: {
	controller: FileManagerController;
}) {
	const server = useFiles();
	const { state, navigation, operations, editor, picker } = controller;
	return (
		<main
			className={state.view === "grid" ? "file-grid" : "file-list"}
			aria-label="Directory contents"
		>
			{state.view === "list" && (
				// biome-ignore lint/a11y: The CSS grid keeps row semantics without changing its established DOM layout.
				<div className="file-list-head" role="row">
					<b>Name</b>
					<b>Type</b>
					<b>Size</b>
					<b>Modified</b>
				</div>
			)}
			{state.listing?.entries.map((item) => {
				const value = { rootId: navigation.rootId, entry: item };
				const key = `${value.rootId}:${item.path}`;
				const selectedItem =
					controller.selectedKeys.has(key) || controller.sourceKeys.has(key);
				const pickerAllowed =
					!picker ||
					pickerSelectionIsValid([value], { ...picker, multiple: false });
				return (
					<Button
						variant="ghost"
						key={item.path}
						className={`${selectedItem ? "selected" : ""} ${picker && !pickerAllowed ? "picker-invalid" : ""}`}
						aria-pressed={selectedItem}
						aria-label={`${item.name}, ${item.kind}`}
						onClick={(event) => operations.selectEntry(item, event)}
						onDoubleClick={() => {
							if (item.kind === "folder")
								navigation.navigate({
									rootId: navigation.rootId,
									path: item.path,
								});
							else if (!picker && textExtensions.has(extension(item.name)))
								void editor.openText(value);
						}}
					>
						<span className="file-item-name">
							{state.view === "grid" &&
							item.kind === "file" &&
							imageExtensions.has(extension(item.name)) ? (
								<RasterThumbnail
									rootId={navigation.rootId}
									entry={item}
									load={server.fileThumbnail}
								/>
							) : (
								<span className="file-item-icon" aria-hidden="true">
									{itemIcon(item)}
								</span>
							)}
							<span>{item.name}</span>
						</span>
						{state.view === "list" && (
							<>
								<span>
									{item.kind === "folder"
										? "Folder"
										: extension(item.name).toUpperCase() || "File"}
								</span>
								<span>
									{item.kind === "file" ? formatSize(item.size) : "—"}
								</span>
								<span>{formatTime(item.modified_millis)}</span>
							</>
						)}
					</Button>
				);
			})}
			{state.listing && !state.listing.entries.length && (
				<p className="file-empty-directory">This folder is empty.</p>
			)}
			{!state.listing && navigation.rootId && !state.busy && (
				<p className="file-empty-directory">The directory is unavailable.</p>
			)}
		</main>
	);
}

function SelectionProperties({
	controller,
}: {
	controller: FileManagerController;
}) {
	const { state, details, editor, picker } = controller;
	return (
		<aside className="file-properties" aria-label="Selection properties">
			<h3>Properties</h3>
			{state.selected.length === 1 ? (
				<FileProperties
					selection={state.selected[0]}
					previewUrl={state.previewUrl}
					nativeNote={state.nativeNote}
					noteDraft={state.noteDraft}
					busy={state.busy}
					onNoteDraft={state.setNoteDraft}
					onSaveNote={() => void details.saveNativeNote()}
					onOpenText={picker ? undefined : editor.openText}
				/>
			) : (
				<p>
					{state.selected.length
						? `${state.selected.length} items selected`
						: "Select an item"}
				</p>
			)}
		</aside>
	);
}

export function FileManagerBrowser({
	controller,
}: {
	controller: FileManagerController;
}) {
	return (
		<>
			<FileManagerToolbar controller={controller} />
			<div className="file-columns">
				<FileRoots controller={controller} />
				<DirectoryContents controller={controller} />
				<SelectionProperties controller={controller} />
			</div>
		</>
	);
}
