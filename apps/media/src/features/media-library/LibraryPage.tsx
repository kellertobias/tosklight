import { FileDropField, NumberField, TextField } from "@tosklight/ui/controls";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowFrame, WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type DragEvent,
	type MouseEvent,
	type ReactNode,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog } from "../../shared/api/queries";
import { TextSourcesPage } from "../text-sources/TextSourcesPage";
import { VisualizersPage } from "../visualizers/VisualizersPage";
import {
	LibrarySourceToggle,
	type LibrarySourceType,
} from "./GeneratedLibraryBrowserView";
import { ImportPanel } from "./ImportPanel";

const CATALOG_POLL_MS = 15_000;
const MEDIA_FOLDER_COUNT = 199;
const FILES_PER_FOLDER = 254;
const DRAG_MEDIA_TYPE = "application/x-tosklight-media-items";
const DRAG_FOLDER_TYPE = "application/x-tosklight-media-folder";
const FIRST_PARKING_FOLDER = 900;
const LAST_PARKING_FOLDER = 999;

type CatalogItem = CatalogView["folders"][number]["items"][number];

export function LibraryPage({
	mode = "media",
	onModeChange,
}: {
	mode?: LibrarySourceType;
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	if (mode === "text") return <TextSourcesPage onModeChange={onModeChange} />;
	if (mode === "visualizers")
		return <VisualizersPage onModeChange={onModeChange} />;
	return <MediaLibraryPage onModeChange={onModeChange} />;
}

function MediaLibraryPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const catalog = useCatalog(CATALOG_POLL_MS);
	const editing = useEditing(catalog.reload);

	if (!catalog.data) {
		return (
			<WindowFrame title="Library" className="media-library-window">
				<p className={`media-state ${catalog.failure ? "is-error" : ""}`}>
					{catalog.failure?.message ?? "Loading the CITP media library…"}
				</p>
			</WindowFrame>
		);
	}

	return (
		<LibraryBrowserView
			onModeChange={onModeChange}
			catalog={catalog.data}
			busy={editing.busy}
			failure={editing.failure?.message}
			importPanel={<ImportPanel onImported={catalog.reload} />}
			onDismissFailure={editing.dismiss}
			onRenameFolder={(folder, name) =>
				editing.save(() =>
					api.updateLibraryFolder(folder, { requestId: requestId(), name }),
				)
			}
			onSwapFolders={(first, second) =>
				editing.save(() =>
					api.updateLibraryFolder(first, {
						requestId: requestId(),
						swapWith: second,
					}),
				)
			}
			onReorderItem={(item, destination) =>
				editing.save(() =>
					api.updateLibraryItem(item.id, {
						requestId: requestId(),
						...destination,
						swap: true,
					}),
				)
			}
			onUpdateItem={(item, update) =>
				editing.save(async () => {
					try {
						if (update.name !== undefined) {
							await api.updateLibraryItem(item.id, {
								requestId: requestId(),
								name: update.name,
								swap: false,
							});
						}
						if (update.intrinsicBpm !== undefined) {
							await api.updateLibraryItem(item.id, {
								requestId: requestId(),
								intrinsicBpm: update.intrinsicBpm,
								swap: false,
							});
						}
					} catch (error) {
						// Both wire intents belong to one Save action. If the first was
						// accepted, refresh before showing a refusal from the second.
						catalog.reload();
						throw error;
					}
				})
			}
			onMoveItems={async (items, folder) => {
				const currentCatalog = catalog.data;
				if (!currentCatalog) return;
				if (
					items.every((item) =>
						currentCatalog.folders
							.find((entry) => entry.folder === folder)
							?.items.some((candidate) => candidate.id === item.id),
					)
				)
					return;
				await editing.save(async () => {
					const addresses = allocateFreeAddresses(
						currentCatalog,
						folder,
						items,
					);
					try {
						for (const [index, item] of items.entries()) {
							const destination = addresses[index];
							if (!destination)
								throw new Error("No free media address remains.");
							await api.updateLibraryItem(item.id, {
								requestId: requestId(),
								...destination,
								swap: false,
							});
						}
					} catch (error) {
						// A server can accept an earlier move before refusing a later one.
						// Re-read before the operator retries so allocation never uses stale slots.
						catalog.reload();
						throw error;
					}
				});
			}}
			onUpload={async (files, folder) => {
				const currentCatalog = catalog.data;
				if (!currentCatalog) return;
				await editing.save(async () => {
					const addresses = allocateFreeAddresses(
						currentCatalog,
						folder,
						[],
						files.length,
					);
					try {
						for (const [index, media] of files.entries()) {
							const destination = addresses[index];
							if (!destination)
								throw new Error("No free media address remains.");
							await api.uploadLibraryItem(
								destination.folder,
								destination.file,
								requestId(),
								media.name.replace(/\.[^.]+$/u, ""),
								media,
							);
						}
					} catch (error) {
						catalog.reload();
						throw error;
					}
				});
			}}
		/>
	);
}

export interface LibraryBrowserViewProps {
	catalog: CatalogView;
	busy?: boolean;
	failure?: string;
	onDismissFailure?: () => void;
	onRenameFolder?: (folder: number, name: string) => void;
	onSwapFolders?: (first: number, second: number) => void;
	onUpdateItem?: (
		item: CatalogItem,
		update: { name?: string; intrinsicBpm?: number | null },
	) => void;
	onMoveItems?: (items: CatalogItem[], folder: number) => void;
	onReorderItem?: (
		item: CatalogItem,
		destination: { folder: number; file: number },
	) => void;
	onUpload?: (files: readonly File[], folder: number) => void;
	thumbnailUrl?: (folder: number, file: number) => string;
	importPanel?: ReactNode;
	onModeChange?: (mode: LibrarySourceType) => void;
}

/** The Media Server's address-first, three-pane CITP library editor. */
export function LibraryBrowserView({
	catalog,
	busy = false,
	failure,
	onDismissFailure,
	onRenameFolder,
	onSwapFolders,
	onUpdateItem,
	onMoveItems,
	onReorderItem,
	onUpload,
	thumbnailUrl = api.thumbnailUrl,
	importPanel,
	onModeChange,
}: LibraryBrowserViewProps) {
	const [folder, setFolder] = useState(1);
	const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
	const [focusedId, setFocusedId] = useState<string | null>(null);
	const [folderEditor, setFolderEditor] = useState<number | null>(null);
	const [search, setSearch] = useState("");
	const [dropFailure, setDropFailure] = useState<string | null>(null);
	const picker = useRef<HTMLInputElement>(null);
	const selectedFolder = catalog.folders.find(
		(entry) => entry.folder === folder,
	);
	const visibleItems = useMemo(() => {
		const needle = search.trim().toLowerCase();
		return (selectedFolder?.items ?? []).filter(
			(item) => !needle || item.name.toLowerCase().includes(needle),
		);
	}, [search, selectedFolder]);
	const focused = selectedFolder?.items.find((item) => item.id === focusedId);

	useEffect(() => {
		setSelectedIds(new Set());
		setFocusedId(null);
	}, [folder]);

	const choose = (item: CatalogItem, event: MouseEvent<HTMLButtonElement>) => {
		setFocusedId(item.id);
		setFolderEditor(null);
		setSelectedIds((current) => {
			if (event.metaKey || event.ctrlKey) {
				const next = new Set(current);
				if (next.has(item.id)) next.delete(item.id);
				else next.add(item.id);
				return next;
			}
			return new Set([item.id]);
		});
	};

	const selectedItems = (selectedFolder?.items ?? []).filter((item) =>
		selectedIds.has(item.id),
	);
	const folders = [
		...Array.from({ length: MEDIA_FOLDER_COUNT }, (_, index) => index + 1),
		...Array.from(
			{ length: LAST_PARKING_FOLDER - FIRST_PARKING_FOLDER + 1 },
			(_, index) => FIRST_PARKING_FOLDER + index,
		),
	];

	const dropOnFolder = (
		event: DragEvent<HTMLButtonElement>,
		number: number,
	) => {
		if (!isStorageFolder(number) || busy) return;
		event.preventDefault();
		const draggedFolder = Number(
			typeof event.dataTransfer.getData === "function"
				? event.dataTransfer.getData(DRAG_FOLDER_TYPE)
				: "",
		);
		if (isStorageFolder(draggedFolder)) {
			if (draggedFolder !== number) void onSwapFolders?.(draggedFolder, number);
			return;
		}
		const files = [...event.dataTransfer.files];
		if (files.length) {
			if (!isPlayableFolder(number)) {
				setDropFailure(
					"Upload into a playable folder, then park the imported media.",
				);
				return;
			}
			if (!files.every(isAcceptedMediaFile)) {
				setDropFailure("Only video and image files can be uploaded.");
				return;
			}
			setDropFailure(null);
			void onUpload?.(files, number);
			return;
		}
		if (event.dataTransfer.types.includes(DRAG_MEDIA_TYPE)) {
			const ids = draggedItemIds(event.dataTransfer.getData(DRAG_MEDIA_TYPE));
			const byId = new Map(
				catalog.folders.flatMap((entry) =>
					entry.items.map((item) => [item.id, item] as const),
				),
			);
			const items = ids.flatMap((id) => {
				const item = byId.get(id);
				return item ? [item] : [];
			});
			if (items.length) void onMoveItems?.(items, number);
		}
	};
	const dropOnFile = (event: DragEvent<HTMLButtonElement>, file: number) => {
		event.preventDefault();
		const ids = draggedItemIds(event.dataTransfer.getData(DRAG_MEDIA_TYPE));
		if (ids.length !== 1) return;
		const dragged = catalog.folders
			.flatMap((entry) => entry.items)
			.find((candidate) => candidate.id === ids[0]);
		const sourceFolder = catalog.folders.find((entry) =>
			entry.items.some((candidate) => candidate.id === dragged?.id),
		)?.folder;
		if (!dragged || (sourceFolder === folder && dragged.file === file)) return;
		void onReorderItem?.(dragged, { folder, file });
	};

	return (
		<WindowFrame
			title="Library"
			toolbar={<LibrarySourceToggle value="media" onChange={onModeChange} />}
			info={{
				primary: "CITP media library",
				secondary:
					"Folders and files keep their desk addresses while you prepare media.",
			}}
			className="media-library-window"
			onSearch={setSearch}
			search={{ value: search, placeholder: "Find media in this folder" }}
		>
			{(dropFailure ?? failure) && (
				<button
					type="button"
					className="media-library-error"
					onClick={() => {
						setDropFailure(null);
						onDismissFailure?.();
					}}
				>
					{dropFailure ?? failure} · Dismiss
				</button>
			)}
			<div className="media-catalog-browser">
				<WindowScrollArea className="media-library-folders">
					<div className="media-library-pool-heading">
						<span>Folders</span>
						<small>001–199 · Parking 900–999</small>
					</div>
					<div className="media-library-folder-pool">
						{folders.map((number) => {
							const entry = catalog.folders.find(
								(candidate) => candidate.folder === number,
							);
							const writable = isStorageFolder(number);
							return (
								<PoolCard
									key={number}
									model={{
										number: String(number).padStart(3, "0"),
										primary:
											entry?.name ||
											(number >= FIRST_PARKING_FOLDER
												? "Parking"
												: "Empty folder"),
										secondary: writable
											? `${entry?.items.length ?? 0}/254`
											: "Reserved",
										color:
											number >= FIRST_PARKING_FOLDER
												? DEFAULT_POOL_COLOR_PALETTE.macro
												: DEFAULT_POOL_COLOR_PALETTE.group,
										states:
											folder === number
												? ["selected"]
												: writable
													? []
													: ["disabled"],
									}}
									className={`media-library-folder ${number >= FIRST_PARKING_FOLDER ? "is-parking" : ""}`}
									data-folder={number}
									onClick={() => {
										setFolderEditor(null);
										setFolder(number);
									}}
									onContextMenu={(event) => {
										event.preventDefault();
										if (writable) {
											setFolder(number);
											setFolderEditor(number);
										}
									}}
									draggable={writable && !busy}
									onDragStart={(event) => {
										event.dataTransfer.setData(
											DRAG_FOLDER_TYPE,
											String(number),
										);
										event.dataTransfer.effectAllowed = "move";
									}}
									onDragOver={(event) => {
										if (writable && !busy) event.preventDefault();
									}}
									onDrop={(event) => dropOnFolder(event, number)}
								/>
							);
						})}
					</div>
				</WindowScrollArea>

				<WindowScrollArea className="media-library-pool">
					<div className="media-library-pool-heading">
						<span>Folder {String(folder).padStart(3, "0")}</span>
						<small>
							{selectedIds.size
								? `${selectedIds.size} selected`
								: `${visibleItems.length} media`}
						</small>
					</div>
					<PoolGrid
						slots={visibleItems.map((item) =>
							itemSlot(item, folder, thumbnailUrl),
						)}
						slotCount={FILES_PER_FOLDER}
						columns={5}
						minimumCardWidth={92}
						emptySlot={(index) => ({
							id: `empty-${index + 1}`,
							position: index,
							card: { number: index + 1, primary: "", states: ["empty"] },
						})}
						renderSlot={(slot) => {
							const item = visibleItems.find(
								(candidate) => candidate.file - 1 === slot.position,
							);
							if (!item)
								return (
									<PoolCard
										model={slot.card}
										onDragOver={(event) => event.preventDefault()}
										onDrop={(event) => dropOnFile(event, slot.position + 1)}
									/>
								);
							return (
								<PoolCard
									model={{
										...slot.card,
										states: selectedIds.has(item.id) ? ["selected"] : [],
									}}
									draggable={!busy}
									onDragStart={(event) => {
										const ids = selectedIds.has(item.id)
											? selectedItems.map((selected) => selected.id)
											: [item.id];
										if (!selectedIds.has(item.id))
											setSelectedIds(new Set([item.id]));
										event.dataTransfer.setData(
											DRAG_MEDIA_TYPE,
											JSON.stringify(ids),
										);
										event.dataTransfer.effectAllowed = "move";
									}}
									onClick={(event) => choose(item, event)}
									onDragOver={(event) => {
										event.preventDefault();
									}}
									onDrop={(event) => {
										dropOnFile(event, item.file);
									}}
								/>
							);
						}}
					/>
				</WindowScrollArea>

				<aside className="media-library-inspector">
					{folderEditor !== null ? (
						<FolderEditor
							key={folderEditor}
							folder={folderEditor}
							name={
								catalog.folders.find((entry) => entry.folder === folderEditor)
									?.name ?? ""
							}
							busy={busy}
							onSave={onRenameFolder}
						/>
					) : focused ? (
						<ItemEditor
							folder={folder}
							item={focused}
							busy={busy}
							onUpdate={onUpdateItem}
							thumbnailUrl={thumbnailUrl}
						/>
					) : isPlayableFolder(folder) ? (
						<UploadEditor
							folder={folder}
							busy={busy}
							picker={picker}
							onUpload={onUpload}
							importPanel={importPanel}
						/>
					) : folder >= FIRST_PARKING_FOLDER ? (
						<div className="media-library-reserved-copy">
							<h2>Parking folder {folder}</h2>
							<p>
								Drop existing media here to take it out of playback without
								deleting it.
							</p>
						</div>
					) : (
						<div className="media-library-reserved-copy">
							<h2>Folder unavailable</h2>
						</div>
					)}
				</aside>
			</div>
		</WindowFrame>
	);
}

function itemSlot(
	item: CatalogItem,
	folder: number,
	thumbnailUrl: (folder: number, file: number) => string,
): PoolSlotViewModel<string> {
	return {
		id: item.id,
		position: item.file - 1,
		card: {
			number: item.file,
			primary: item.name,
			secondary: item.intrinsicBpm ? `${item.intrinsicBpm} BPM` : undefined,
			image: { src: thumbnailUrl(folder, item.file), alt: "" },
			color: DEFAULT_POOL_COLOR_PALETTE.preset.mixed,
		},
	};
}

function ItemEditor({
	folder,
	item,
	busy,
	onUpdate,
	thumbnailUrl,
}: {
	folder: number;
	item: CatalogItem;
	busy: boolean;
	onUpdate?: LibraryBrowserViewProps["onUpdateItem"];
	thumbnailUrl: (folder: number, file: number) => string;
}) {
	const [name, setName] = useState(item.name);
	const [bpm, setBpm] = useState(item.intrinsicBpm?.toString() ?? "");
	useEffect(() => {
		setName(item.name);
		setBpm(item.intrinsicBpm?.toString() ?? "");
	}, [item]);
	return (
		<form
			className="media-library-editor"
			onSubmit={(event) => {
				event.preventDefault();
				const value = bpm.trim() ? Number(bpm) : null;
				const update: { name?: string; intrinsicBpm?: number | null } = {};
				if (name !== item.name) update.name = name;
				if (value !== item.intrinsicBpm) update.intrinsicBpm = value;
				if (Object.keys(update).length) void onUpdate?.(item, update);
			}}
		>
			<p className="media-library-eyebrow">Media</p>
			<img src={thumbnailUrl(folder, item.file)} alt={`${item.name} preview`} />
			<p className="media-library-address">
				{String(folder).padStart(3, "0")} / {String(item.file).padStart(3, "0")}
			</p>
			<TextField
				label="Media name"
				value={name}
				onChange={(event) => setName(event.target.value)}
				required
			/>
			<NumberField
				label="BPM"
				value={bpm}
				min={1}
				step={0.01}
				placeholder="Not set"
				onChange={(event) => setBpm(event.target.value)}
			/>
			<button type="submit" className="ui-button primary" disabled={busy}>
				Save media
			</button>
		</form>
	);
}

function FolderEditor({
	folder,
	name,
	busy,
	onSave,
}: {
	folder: number;
	name: string;
	busy: boolean;
	onSave?: LibraryBrowserViewProps["onRenameFolder"];
}) {
	const [next, setNext] = useState(name);
	return (
		<form
			className="media-library-editor"
			onSubmit={(event) => {
				event.preventDefault();
				void onSave?.(folder, next);
			}}
		>
			<p className="media-library-eyebrow">
				Folder {String(folder).padStart(3, "0")}
			</p>
			<h2>Configure folder</h2>
			<TextField
				label="Folder name"
				value={next}
				onChange={(event) => setNext(event.target.value)}
				placeholder="Empty folder"
			/>
			<button type="submit" className="ui-button primary" disabled={busy}>
				Save folder
			</button>
		</form>
	);
}

function UploadEditor({
	folder,
	busy,
	picker,
	onUpload,
	importPanel,
}: {
	folder: number;
	busy: boolean;
	picker: React.RefObject<HTMLInputElement | null>;
	onUpload?: LibraryBrowserViewProps["onUpload"];
	importPanel?: ReactNode;
}) {
	return (
		<div className="media-library-editor">
			<p className="media-library-eyebrow">
				Folder {String(folder).padStart(3, "0")}
			</p>
			<h2>Add media</h2>
			<p>
				Files take the first free slots. If this folder fills up, allocation
				continues in the next media folder.
			</p>
			<input
				ref={picker}
				hidden
				type="file"
				multiple
				accept="video/*,image/*"
				onChange={(event) => {
					const files = [...(event.target.files ?? [])];
					event.currentTarget.value = "";
					if (files.length) void onUpload?.(files, folder);
				}}
			/>
			<FileDropField
				label="Media files"
				constraints={{ mimeTypes: ["video/*", "image/*"], multiple: true }}
				disabled={busy}
				onFiles={(files) => void onUpload?.(files, folder)}
				onOpenPicker={() => picker.current?.click()}
			/>
			{importPanel}
		</div>
	);
}

function isPlayableFolder(folder: number) {
	return folder >= 1 && folder <= MEDIA_FOLDER_COUNT;
}

function isStorageFolder(folder: number) {
	return (
		isPlayableFolder(folder) ||
		(folder >= FIRST_PARKING_FOLDER && folder <= LAST_PARKING_FOLDER)
	);
}

export function isAcceptedMediaFile(file: Pick<File, "type">) {
	return file.type.startsWith("video/") || file.type.startsWith("image/");
}

export function draggedItemIds(payload: string): string[] {
	try {
		const parsed: unknown = JSON.parse(payload);
		if (!Array.isArray(parsed)) return [];
		return parsed.filter(
			(id, index): id is string =>
				typeof id === "string" && parsed.indexOf(id) === index,
		);
	} catch {
		return [];
	}
}

export function allocateFreeAddresses(
	catalog: CatalogView,
	startFolder: number,
	moving: readonly CatalogItem[] = [],
	count = moving.length,
) {
	const movingIds = new Set(moving.map((item) => item.id));
	const occupied = new Set(
		catalog.folders.flatMap((folder) =>
			folder.items
				.filter((item) => !movingIds.has(item.id))
				.map((item) => `${folder.folder}/${item.file}`),
		),
	);
	const addresses: Array<{ folder: number; file: number }> = [];
	const lastFolder =
		startFolder >= FIRST_PARKING_FOLDER
			? LAST_PARKING_FOLDER
			: MEDIA_FOLDER_COUNT;
	for (let folder = startFolder; folder <= lastFolder; folder += 1) {
		for (let file = 1; file <= FILES_PER_FOLDER; file += 1) {
			if (!occupied.has(`${folder}/${file}`)) addresses.push({ folder, file });
			if (addresses.length === count) return addresses;
		}
	}
	return addresses;
}
