import { FileDropField, NumberField, TextField } from "@tosklight/ui/controls";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowFrame, WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type DragEvent,
	type MouseEvent,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from "react";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog } from "../../shared/api/queries";
import { ImportPanel } from "./ImportPanel";

const CATALOG_POLL_MS = 15_000;
const MEDIA_FOLDER_COUNT = 199;
const CITP_FOLDER_COUNT = 255;
const FILES_PER_FOLDER = 254;
const DRAG_MEDIA_TYPE = "application/x-tosklight-media-items";

type CatalogItem = CatalogView["folders"][number]["items"][number];

export function LibraryPage() {
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
			onRenameItem={(item, name) =>
				editing.save(() =>
					api.updateLibraryItem(item.id, {
						requestId: requestId(),
						name,
						swap: false,
					}),
				)
			}
			onSetBpm={(item, intrinsicBpm) =>
				editing.save(() =>
					api.updateLibraryItem(item.id, {
						requestId: requestId(),
						intrinsicBpm,
						swap: false,
					}),
				)
			}
			onMoveItems={async (items, folder) => {
				await editing.save(async () => {
					const addresses = allocateFreeAddresses(catalog.data!, folder, items);
					for (const [index, item] of items.entries()) {
						const destination = addresses[index];
						if (!destination) throw new Error("No free media address remains.");
						await api.updateLibraryItem(item.id, {
							requestId: requestId(),
							...destination,
							swap: false,
						});
					}
				});
			}}
			onUpload={async (files, folder) => {
				await editing.save(async () => {
					const addresses = allocateFreeAddresses(
						catalog.data!,
						folder,
						[],
						files.length,
					);
					for (const [index, media] of files.entries()) {
						const destination = addresses[index];
						if (!destination) throw new Error("No free media address remains.");
						await api.uploadLibraryItem(
							destination.folder,
							destination.file,
							requestId(),
							media.name.replace(/\.[^.]+$/u, ""),
							media,
						);
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
	onRenameFolder?: (folder: number, name: string) => Promise<unknown> | void;
	onRenameItem?: (item: CatalogItem, name: string) => Promise<unknown> | void;
	onSetBpm?: (item: CatalogItem, bpm: number | null) => Promise<unknown> | void;
	onMoveItems?: (
		items: CatalogItem[],
		folder: number,
	) => Promise<unknown> | void;
	onUpload?: (
		files: readonly File[],
		folder: number,
	) => Promise<unknown> | void;
	thumbnailUrl?: (folder: number, file: number) => string;
	importPanel?: ReactNode;
}

/** The Media Server's address-first, three-pane CITP library editor. */
export function LibraryBrowserView({
	catalog,
	busy = false,
	failure,
	onDismissFailure,
	onRenameFolder,
	onRenameItem,
	onSetBpm,
	onMoveItems,
	onUpload,
	thumbnailUrl = api.thumbnailUrl,
	importPanel,
}: LibraryBrowserViewProps) {
	const [folder, setFolder] = useState(1);
	const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
	const [focusedId, setFocusedId] = useState<string | null>(null);
	const [folderEditor, setFolderEditor] = useState<number | null>(null);
	const [search, setSearch] = useState("");
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
		setFolderEditor(null);
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

	return (
		<WindowFrame
			title="Library"
			info={{
				primary: "CITP media library",
				secondary:
					"Folders and files keep their desk addresses while you prepare media.",
			}}
			className="media-library-window"
			onSearch={setSearch}
			search={{ value: search, placeholder: "Find media in this folder" }}
		>
			{failure && (
				<button
					type="button"
					className="media-library-error"
					onClick={onDismissFailure}
				>
					{failure} · Dismiss
				</button>
			)}
			<div className="media-library-browser">
				<WindowScrollArea className="media-library-folders">
					{Array.from(
						{ length: CITP_FOLDER_COUNT },
						(_, index) => index + 1,
					).map((number) => {
						const entry = catalog.folders.find(
							(candidate) => candidate.folder === number,
						);
						const writable = number <= MEDIA_FOLDER_COUNT;
						return (
							<button
								type="button"
								key={number}
								className={`media-library-folder ${folder === number ? "is-selected" : ""} ${writable ? "" : "is-reserved"}`}
								onClick={() => setFolder(number)}
								onContextMenu={(event) => {
									event.preventDefault();
									if (writable) {
										setFolder(number);
										setFolderEditor(number);
									}
								}}
								onDragOver={(event) => {
									if (writable) event.preventDefault();
								}}
								onDrop={(event) => {
									if (!writable) return;
									event.preventDefault();
									const files = [...event.dataTransfer.files];
									if (files.length) void onUpload?.(files, number);
									else if (event.dataTransfer.types.includes(DRAG_MEDIA_TYPE))
										void onMoveItems?.(selectedItems, number);
								}}
							>
								<b>{String(number).padStart(3, "0")}</b>
								<span>
									{entry?.name || reservedFolderName(number) || "Empty folder"}
								</span>
								<small>
									{writable ? `${entry?.items.length ?? 0}/254` : "Reserved"}
								</small>
							</button>
						);
					})}
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
							if (!item) return <PoolCard model={slot.card} disabled />;
							return (
								<PoolCard
									model={{
										...slot.card,
										states: selectedIds.has(item.id) ? ["selected"] : [],
									}}
									draggable
									onDragStart={(event) => {
										if (!selectedIds.has(item.id))
											setSelectedIds(new Set([item.id]));
										event.dataTransfer.setData(DRAG_MEDIA_TYPE, item.id);
										event.dataTransfer.effectAllowed = "move";
									}}
									onClick={(event) => choose(item, event)}
								/>
							);
						}}
					/>
				</WindowScrollArea>

				<aside className="media-library-inspector">
					{folderEditor !== null ? (
						<FolderEditor
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
							onRename={onRenameItem}
							onSetBpm={onSetBpm}
							thumbnailUrl={thumbnailUrl}
						/>
					) : folder <= MEDIA_FOLDER_COUNT ? (
						<UploadEditor
							folder={folder}
							busy={busy}
							picker={picker}
							onUpload={onUpload}
							importPanel={importPanel}
						/>
					) : (
						<div className="media-library-reserved-copy">
							<h2>{reservedFolderName(folder)}</h2>
							<p>
								This address range is generated by the Text or Visualizer
								screen.
							</p>
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
		},
	};
}

function ItemEditor({
	folder,
	item,
	busy,
	onRename,
	onSetBpm,
	thumbnailUrl,
}: {
	folder: number;
	item: CatalogItem;
	busy: boolean;
	onRename?: LibraryBrowserViewProps["onRenameItem"];
	onSetBpm?: LibraryBrowserViewProps["onSetBpm"];
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
				if (name !== item.name) void onRename?.(item, name);
				const value = bpm.trim() ? Number(bpm) : null;
				if (value !== item.intrinsicBpm) void onSetBpm?.(item, value);
			}}
		>
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

function reservedFolderName(folder: number) {
	if (folder >= 200 && folder <= 219) return "Text sources";
	if (folder >= 220) return "Visualizers";
	return "";
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
	for (let folder = startFolder; folder <= MEDIA_FOLDER_COUNT; folder += 1) {
		for (let file = 1; file <= FILES_PER_FOLDER; file += 1) {
			if (!occupied.has(`${folder}/${file}`)) addresses.push({ folder, file });
			if (addresses.length === count) return addresses;
		}
	}
	return addresses;
}
