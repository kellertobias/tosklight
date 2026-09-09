import { Button, FileDropField, NumberField, SwitchField, TextField } from "@tosklight/ui/controls";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	ButtonGrid,
	WindowFrame,
	WindowScrollArea,
} from "@tosklight/ui/window-kit";
import {
	type DragEvent,
	type MouseEvent,
	type ReactNode,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { MediaErrorToast } from "../../app/ToastContext";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	CatalogView,
	LibraryNoteTargetView,
} from "../../shared/api/generated/media-wire";
import { useCatalog, useFolderPresentations } from "../../shared/api/queries";
import { useMainOutputAspectRatio } from "../../shared/output/useMainOutputAspectRatio";
import { TextSourcesPage } from "../text-sources/TextSourcesPage";
import { VisualizersPage } from "../visualizers/VisualizersPage";
import { EffectsPage } from "../effects/EffectsPage";
import type { FolderPresentation } from "./FolderPresentationEditor";
import {
	type LibrarySourceType,
	librarySourceGroups,
} from "./GeneratedLibraryBrowserView";
import { ImportPanel } from "./ImportPanel";
import {
	EmptySlotEditor,
	FolderEditor,
	isPlayableFolder,
	LibraryNotesEditor,
	UploadEditor,
} from "./LibrarySecondaryEditors";

const CATALOG_POLL_MS = 15_000;
const MEDIA_FOLDER_COUNT = 199;
const FILES_PER_FOLDER = 254;
const DRAG_MEDIA_TYPE = "application/x-tosklight-media-items";
const DRAG_FOLDER_TYPE = "application/x-tosklight-media-folder";
const FIRST_PARKING_FOLDER = 900;
const LAST_PARKING_FOLDER = 999;

export type CatalogItem = CatalogView["folders"][number]["items"][number];

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
	if (mode === "effects") return <EffectsPage onModeChange={onModeChange} />;
	return <MediaLibraryPage onModeChange={onModeChange} />;
}

function MediaLibraryPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const catalog = useCatalog(CATALOG_POLL_MS);
	const folderPresentations = useFolderPresentations();
	const editing = useEditing(catalog.reload);
	const previewAspectRatio = useMainOutputAspectRatio();

	if (!catalog.data) {
		return (
			<WindowFrame title="Library" className="media-library-window">
				<p className={`media-state ${catalog.failure ? "is-error" : ""}`}>
					{catalog.failure?.message ?? "Loading the CITP media library…"}
				</p>
			</WindowFrame>
		);
	}

	const actions = useLibraryActions(catalog, folderPresentations, editing);
	return <LibraryBrowserView {...actions} catalog={catalog.data}
		folderPresentations={folderPresentations.data?.folders}
		previewAspectRatio={previewAspectRatio} onModeChange={onModeChange}
		busy={editing.busy} failure={editing.failure?.message}
		importPanel={<ImportPanel onImported={catalog.reload} />}
		onDismissFailure={editing.dismiss} />;
}

function useLibraryActions(
	catalog: ReturnType<typeof useCatalog>,
	folderPresentations: ReturnType<typeof useFolderPresentations>,
	editing: ReturnType<typeof useEditing>,
): Partial<LibraryBrowserViewProps> {
	const refreshPresentations = async (action: Promise<unknown>) => {
		await action;
		folderPresentations.reload();
	};
	const guarded = async (action: () => Promise<void>) => {
		try { await action(); } catch (error) { catalog.reload(); throw error; }
	};
	return {
		onRenameFolder: (folder, name) => editing.save(() => refreshPresentations(
			api.updateFolderPresentation(folder, { requestId: requestId(), name }))),
		onSetFolderIcon: (folder, icon) => editing.save(() => refreshPresentations(
			api.updateFolderPresentation(folder, { requestId: requestId(), icon }))),
		onSetFolderPicture: (folder, picture) => editing.save(() => refreshPresentations(
			api.uploadFolderPicture(folder, requestId(), picture))),
		onRemoveFolderPicture: (folder) => editing.save(() => refreshPresentations(
			api.removeFolderPicture(folder, requestId()))),
		onSwapFolders: (first, second) => editing.save(() => api.updateLibraryFolder(
			first, { requestId: requestId(), swapWith: second })),
		onCompactFolder: (folder) => editing.save(() => api.updateLibraryFolder(
			folder, { requestId: requestId(), compact: true })),
		onReorderItem: (item, destination) => editing.save(() => api.updateLibraryItem(
			item.id, { requestId: requestId(), ...destination, swap: true })),
		onUpdateItem: (item, update) => editing.save(() => guarded(async () => {
			if (update.name !== undefined) await api.updateLibraryItem(item.id,
				{ requestId: requestId(), name: update.name, swap: false });
			if (update.intrinsicBpm !== undefined) await api.updateLibraryItem(item.id,
				{ requestId: requestId(), intrinsicBpm: update.intrinsicBpm, swap: false });
			if (update.enabled !== undefined) await api.updateLibraryItem(item.id,
				{ requestId: requestId(), enabled: update.enabled, swap: false });
		})),
		onDeleteItem: (item) => editing.save(() => api.deleteLibraryItem(
			item.id, { requestId: requestId() })),
		onSetItemsEnabled: (items, enabled) => editing.save(() => api.updateLibraryItems(
			{ requestId: requestId(), ids: items.map((item) => item.id), enabled })),
		onDeleteItems: (items) => editing.save(() => api.deleteLibraryItems(
			{ requestId: requestId(), ids: items.map((item) => item.id) })),
		onRetryThumbnail: (item) => editing.save(() => api.retryLibraryThumbnail(
			item.id, { requestId: requestId() })),
		onUploadCustomThumbnail: (item, image) => editing.save(() =>
			api.uploadLibraryThumbnail(item.id, requestId(), image)),
		onMoveItems: async (items, folder) => {
			const current = catalog.data;
			if (!current || items.every((item) => current.folders
				.find((entry) => entry.folder === folder)?.items
				.some((candidate) => candidate.id === item.id))) return;
			await editing.save(() => guarded(async () => {
				const addresses = allocateFreeAddresses(current, folder, items);
				for (const [index, item] of items.entries()) {
					const destination = addresses[index];
					if (!destination) throw new Error("No free media address remains.");
					await api.updateLibraryItem(item.id,
						{ requestId: requestId(), ...destination, swap: false });
				}
			}));
		},
		onUpload: async (files, folder) => {
			const current = catalog.data;
			if (!current) return;
			await editing.save(() => guarded(async () => {
				const addresses = allocateFreeAddresses(current, folder, [], files.length);
				for (const [index, media] of files.entries()) {
					const destination = addresses[index];
					if (!destination) throw new Error("No free media address remains.");
					await api.uploadLibraryItem(destination.folder, destination.file,
						requestId(), media.name.replace(/\.[^.]+$/u, ""), media);
				}
			}));
		},
		onUploadAt: (file, destination, name, replace) => editing.save(() =>
			api.uploadLibraryItem(destination.folder, destination.file,
				requestId(), name, file, replace)),
		onUpdateNotes: (targets, note) => editing.save(() => api.updateLibraryNotes(
			{ requestId: requestId(), targets, note })),
	};
}

export interface LibraryBrowserViewProps {
	catalog: CatalogView;
	folderPresentations?: FolderPresentation[];
	busy?: boolean;
	failure?: string;
	onDismissFailure?: () => void;
	onRenameFolder?: (folder: number, name: string) => void;
	onSetFolderIcon?: (folder: number, icon: string) => void;
	onSetFolderPicture?: (folder: number, picture: File) => void;
	onRemoveFolderPicture?: (folder: number) => void;
	onSwapFolders?: (first: number, second: number) => void;
	onCompactFolder?: (folder: number) => void;
	onUpdateItem?: (
		item: CatalogItem,
		update: {
			name?: string;
			intrinsicBpm?: number | null;
			enabled?: boolean;
		},
	) => void;
	onDeleteItem?: (item: CatalogItem) => void;
	onSetItemsEnabled?: (items: CatalogItem[], enabled: boolean) => void;
	onDeleteItems?: (items: CatalogItem[]) => void;
	onRetryThumbnail?: (item: CatalogItem) => void;
	onUploadCustomThumbnail?: (item: CatalogItem, image: File) => void;
	onMoveItems?: (items: CatalogItem[], folder: number) => void;
	onReorderItem?: (
		item: CatalogItem,
		destination: { folder: number; file: number },
	) => void;
	onUpload?: (files: readonly File[], folder: number) => void;
	onUploadAt?: (
		file: File,
		destination: { folder: number; file: number },
		name: string,
		replace: boolean,
	) => void;
	onUpdateNotes?: (targets: LibraryNoteTargetView[], note: string) => void;
	thumbnailUrl?: (folder: number, file: number) => string;
	previewAspectRatio?: number;
	importPanel?: ReactNode;
	onModeChange?: (mode: LibrarySourceType) => void;
}

/** The Media Server's address-first, three-pane CITP library editor. */
export function LibraryBrowserView({ catalog, folderPresentations = [], busy = false, failure,
onDismissFailure, onRenameFolder, onSetFolderIcon, onSetFolderPicture, onRemoveFolderPicture,
onSwapFolders, onCompactFolder, onUpdateItem, onDeleteItem, onSetItemsEnabled,
onDeleteItems, onRetryThumbnail, onUploadCustomThumbnail, onMoveItems, onReorderItem,
onUpload, onUploadAt, onUpdateNotes, thumbnailUrl = api.thumbnailUrl, previewAspectRatio = 16 / 9,
importPanel, onModeChange, }: LibraryBrowserViewProps) { const [folder, setFolder] = useState(1); const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
const [selectedFolders, setSelectedFolders] = useState<Set<number>>( new Set(), ); const [focusedId, setFocusedId] = useState<string | null>(null); const [emptyFile, setEmptyFile] = useState<number | null>(null);
const [folderEditor, setFolderEditor] = useState<number | null>(null); const [search, setSearch] = useState(""); const [dropFailure, setDropFailure] = useState<string | null>(null); const picker = useRef<HTMLInputElement>(null); const selectionAnchorId = useRef<string | null>(null);
const rangeBaseIds = useRef<Set<string>>(new Set()); const selectedFolder = catalog.folders.find( (entry) => entry.folder === folder, ); const visibleItems = useMemo(() => {
const needle = search.trim().toLowerCase(); return (selectedFolder?.items ?? []).filter( (item) => !needle || item.name.toLowerCase().includes(needle), ); }, [search, selectedFolder]);
const focused = selectedFolder?.items.find((item) => item.id === focusedId);  useEffect(() => { setSelectedIds(new Set()); setFocusedId(null);
setEmptyFile(null); selectionAnchorId.current = null; rangeBaseIds.current = new Set(); }, [folder]);
const choose = (item: CatalogItem, event: MouseEvent<HTMLButtonElement>) => { setSelectedFolders(new Set()); setFocusedId(item.id); setEmptyFile(null); setFolderEditor(null);
setSelectedIds((current) => { if (event.shiftKey && selectionAnchorId.current) { const anchor = selectedFolder?.items.find( (candidate) => candidate.id === selectionAnchorId.current, );
if (anchor) { const first = Math.min(anchor.file, item.file); const last = Math.max(anchor.file, item.file); const next = new Set(rangeBaseIds.current); for (const candidate of selectedFolder?.items ?? []) {
if (candidate.file >= first && candidate.file <= last) next.add(candidate.id); } return next; }
} if (event.metaKey || event.ctrlKey) { const next = new Set(current); if (next.has(item.id)) next.delete(item.id); else next.add(item.id);
selectionAnchorId.current = item.id; rangeBaseIds.current = new Set(next); return next; } selectionAnchorId.current = item.id;
rangeBaseIds.current = new Set(); return new Set([item.id]); }); };
const selectedItems = (selectedFolder?.items ?? []).filter((item) => selectedIds.has(item.id), ); const selectedFolderEntries = [...selectedFolders] .sort((left, right) => left - right)
.map( (number) => catalog.folders.find((entry) => entry.folder === number) ?? { folder: number, name: null,
items: [], }, ); const folders = [ ...Array.from({ length: MEDIA_FOLDER_COUNT }, (_, index) => index + 1),
...Array.from( { length: LAST_PARKING_FOLDER - FIRST_PARKING_FOLDER + 1 }, (_, index) => FIRST_PARKING_FOLDER + index, ), ];
 const dropOnFolder = ( event: DragEvent<HTMLButtonElement>, number: number, ) => {
if (!isStorageFolder(number) || busy) return; event.preventDefault(); const draggedFolder = Number( typeof event.dataTransfer.getData === "function" ? event.dataTransfer.getData(DRAG_FOLDER_TYPE)
: "", ); if (isStorageFolder(draggedFolder)) { if (draggedFolder !== number) void onSwapFolders?.(draggedFolder, number); return;
} const files = [...event.dataTransfer.files]; if (files.length) { if (!isPlayableFolder(number)) { setDropFailure(
"Upload into a playable folder, then park the imported media.", ); return; } if (!files.every(isAcceptedMediaFile)) {
setDropFailure("Only video and image files can be uploaded."); return; } setDropFailure(null); void onUpload?.(files, number);
return; } if (event.dataTransfer.types.includes(DRAG_MEDIA_TYPE)) { const ids = draggedItemIds(event.dataTransfer.getData(DRAG_MEDIA_TYPE)); const byId = new Map(
catalog.folders.flatMap((entry) => entry.items.map((item) => [item.id, item] as const), ), ); const items = ids.flatMap((id) => {
const item = byId.get(id); return item ? [item] : []; }); if (items.length) void onMoveItems?.(items, number); }
}; const dropOnFile = (event: DragEvent<HTMLButtonElement>, file: number) => { event.preventDefault(); const ids = draggedItemIds(event.dataTransfer.getData(DRAG_MEDIA_TYPE)); if (ids.length !== 1) return;
const dragged = catalog.folders .flatMap((entry) => entry.items) .find((candidate) => candidate.id === ids[0]); const sourceFolder = catalog.folders.find((entry) => entry.items.some((candidate) => candidate.id === dragged?.id),
)?.folder; if (!dragged || (sourceFolder === folder && dragged.file === file)) return; void onReorderItem?.(dragged, { folder, file }); };
return ( <WindowFrame title="Library" groups={librarySourceGroups({ value: "media",
onChange: onModeChange, actions: [ { id: "new-media", label: "New media",
onPress: () => { const destination = allocateFreeAddresses( catalog, folder, [],
1, )[0]; if (!destination) { setDropFailure("No free media address remains."); return;
} setFolder(destination.folder); setSelectedFolders(new Set()); setFocusedId(null); setFolderEditor(null);
setEmptyFile(destination.file); }, }, ], })}
info={{ primary: "CITP media library", secondary: "Folders and files keep their desk addresses while you prepare media.", }}
className="media-library-window" search={{ value: search, onSearch: setSearch, placeholder: "Find media in this folder",
}} > {(dropFailure ?? failure) && ( <MediaErrorToast message={dropFailure ?? failure ?? "Library operation failed"}
onDismiss={() => { setDropFailure(null); onDismissFailure?.(); }} />
)} <div className="media-catalog-browser"> <WindowScrollArea className="media-library-folders"> <div className="media-library-pool-heading"> <span>Folders</span>
<small>001–199 · Parking 900–999</small> </div> <ButtonGrid className="media-library-folder-pool" minimum={68}> {folders.map((number) => { const entry = catalog.folders.find(
(candidate) => candidate.folder === number, ); const presentation = folderPresentations.find( (candidate) => candidate.folder === number, );
const writable = isStorageFolder(number); const empty = number < FIRST_PARKING_FOLDER && (entry?.items.length ?? 0) === 0; const selected =
selectedFolders.has(number) || (!selectedFolders.size && folder === number); return ( <PoolCard key={number}
model={{ number: String(number).padStart(3, "0"), primary: presentation?.name || entry?.name ||
(number >= FIRST_PARKING_FOLDER ? "Parking" : "Empty folder"), secondary: writable ? `${entry?.items.length ?? 0}/254`
: "Reserved", color: number >= FIRST_PARKING_FOLDER ? DEFAULT_POOL_COLOR_PALETTE.macro : DEFAULT_POOL_COLOR_PALETTE.group,
states: [ ...(selected ? (["selected"] as const) : []), ...(empty ? (["empty"] as const) : writable
? [] : (["disabled"] as const)), ], icon: presentation?.icon || entry?.icon || "▣", image: presentation?.pictureUrl
? { src: presentation.pictureUrl, alt: `${presentation.name ?? entry?.name ?? `Folder ${number}`} preview`, } : undefined,
}} className={`media-library-folder ${number >= FIRST_PARKING_FOLDER ? "is-parking" : ""}`} data-folder={number} onClick={(event) => { setFolder(number);
setFocusedId(null); setEmptyFile(null); setSelectedIds(new Set()); selectionAnchorId.current = null; rangeBaseIds.current = new Set();
if (writable && (event.metaKey || event.ctrlKey)) { setSelectedFolders((current) => { const next = new Set(current); if (next.has(number)) next.delete(number); else next.add(number);
return next; }); setFolderEditor(null); } else { setSelectedFolders(
writable ? new Set([number]) : new Set(), ); setFolderEditor(writable ? number : null); } }}
onContextMenu={(event) => { event.preventDefault(); if (writable) { setFolder(number); setSelectedFolders(new Set([number]));
setSelectedIds(new Set()); selectionAnchorId.current = null; rangeBaseIds.current = new Set(); setFolderEditor(number); }
}} draggable={writable && !busy} onDragStart={(event) => { event.dataTransfer.setData( DRAG_FOLDER_TYPE,
String(number), ); event.dataTransfer.effectAllowed = "move"; }} onDragOver={(event) => {
if (writable && !busy) event.preventDefault(); }} onDrop={(event) => dropOnFolder(event, number)} /> );
})} </ButtonGrid> </WindowScrollArea>  <WindowScrollArea className="media-library-pool">
<div className="media-library-pool-heading"> <span>Folder {String(folder).padStart(3, "0")}</span> <small> {selectedIds.size ? `${selectedIds.size} selected`
: `${visibleItems.length} media`} </small> </div> <PoolGrid className="media-file-pool-grid media-library-file-pool-grid"
slots={visibleItems.map((item) => itemSlot(item, folder, catalog.revision, thumbnailUrl), )} slotCount={FILES_PER_FOLDER} minimumCardWidth={112}
emptySlot={(index) => ({ id: `empty-${index + 1}`, position: index, card: { number: index + 1, primary: "", states: ["empty"] }, })}
renderSlot={(slot) => { const item = visibleItems.find( (candidate) => candidate.file - 1 === slot.position, ); if (!item)
return ( <PoolCard model={slot.card} onClick={() => { setFocusedId(null);
setSelectedIds(new Set()); selectionAnchorId.current = null; rangeBaseIds.current = new Set(); setSelectedFolders(new Set()); setFolderEditor(null);
setEmptyFile(slot.position + 1); }} onDragOver={(event) => event.preventDefault()} onDrop={(event) => dropOnFile(event, slot.position + 1)} />
); return ( <PoolCard model={{ ...slot.card,
states: [ ...(selectedIds.has(item.id) ? ["selected" as const] : []), ...(item.enabled === false ? ["disabled" as const] : []),
], }} draggable={!busy} onDragStart={(event) => { const ids = selectedIds.has(item.id)
? selectedItems.map((selected) => selected.id) : [item.id]; if (!selectedIds.has(item.id)) setSelectedIds(new Set([item.id])); event.dataTransfer.setData(
DRAG_MEDIA_TYPE, JSON.stringify(ids), ); event.dataTransfer.effectAllowed = "move"; }}
onClick={(event) => choose(item, event)} onDragOver={(event) => { event.preventDefault(); }} onDrop={(event) => {
dropOnFile(event, item.file); }} /> ); }}
/> {visibleItems.length === 0 && ( <div className="media-library-empty-folder" role="status"> <strong>This folder is empty</strong> <span>Drop media here or choose files to import.</span>
</div> )} </WindowScrollArea>  <aside className="media-library-inspector">
{selectedFolderEntries.length > 1 ? ( <LibraryNotesEditor key={`folders-${[...selectedFolders].sort().join("-")}`} label={`${selectedFolderEntries.length} folders`} notes={selectedFolderEntries.map((entry) => entry.note ?? "")}
targets={selectedFolderEntries.map((entry) => ({ kind: "folder", folder: entry.folder, }))} busy={busy}
onSave={onUpdateNotes} /> ) : selectedItems.length > 1 ? ( <MultiItemEditor key={`items-${selectedItems
.map((item) => item.id) .sort() .join("-")}`} items={selectedItems} busy={busy}
onUpdateNotes={onUpdateNotes} onSetEnabled={onSetItemsEnabled} onDelete={onDeleteItems} /> ) : folderEditor !== null ? (
<FolderEditor key={folderEditor} folder={folderEditor} name={ catalog.folders.find((entry) => entry.folder === folderEditor)
?.name ?? "" } icon={ catalog.folders.find((entry) => entry.folder === folderEditor) ?.icon ?? ""
} pictureUrl={ folderPresentations.find( (candidate) => candidate.folder === folderEditor, )?.pictureUrl ?? null
} busy={busy} note={ catalog.folders.find((entry) => entry.folder === folderEditor) ?.note ?? ""
} onSave={onRenameFolder} onSetIcon={onSetFolderIcon} onSetPicture={onSetFolderPicture} onRemovePicture={onRemoveFolderPicture}
onUpload={onUpload} onUpdateNotes={onUpdateNotes} onCompact={onCompactFolder} /> ) : focused ? (
<ItemEditor folder={folder} item={focused} busy={busy} onUpdate={onUpdateItem}
onReplace={onUploadAt} thumbnailUrl={thumbnailUrl} thumbnailRevision={catalog.revision} previewAspectRatio={previewAspectRatio} onUpdateNotes={onUpdateNotes}
onDelete={onDeleteItem} onRetryThumbnail={onRetryThumbnail} onUploadCustomThumbnail={onUploadCustomThumbnail} /> ) : emptyFile !== null && isPlayableFolder(folder) ? (
<EmptySlotEditor folder={folder} file={emptyFile} busy={busy} onUpload={onUploadAt}
/> ) : isPlayableFolder(folder) ? ( <UploadEditor folder={folder} busy={busy}
picker={picker} onUpload={onUpload} importPanel={importPanel} /> ) : folder >= FIRST_PARKING_FOLDER ? (
<div className="media-library-reserved-copy"> <h2>Parking folder {folder}</h2> <p> Drop existing media here to take it out of playback without deleting it.
</p> </div> ) : ( <div className="media-library-reserved-copy"> <h2>Folder unavailable</h2>
</div> )} </aside> </div> </WindowFrame>
); }
function itemSlot(
	item: CatalogItem,
	folder: number,
	revision: number,
	thumbnailUrl: (folder: number, file: number) => string,
): PoolSlotViewModel<string> {
	return {
		id: item.id,
		position: item.file - 1,
		card: {
			number: item.file,
			primary: item.name,
			secondary:
				item.enabled === false
					? "Disabled"
					: item.intrinsicBpm
						? `${item.intrinsicBpm} BPM`
						: undefined,
			image: {
				src: versionedThumbnailUrl(thumbnailUrl(folder, item.file), revision),
				alt: "",
			},
			color: DEFAULT_POOL_COLOR_PALETTE.preset.mixed,
			states: item.enabled === false ? ["disabled"] : [],
		},
	};
}

function MultiItemEditor({
	items,
	busy,
	onUpdateNotes,
	onSetEnabled,
	onDelete,
}: {
	items: CatalogItem[];
	busy: boolean;
	onUpdateNotes?: LibraryBrowserViewProps["onUpdateNotes"];
	onSetEnabled?: LibraryBrowserViewProps["onSetItemsEnabled"];
	onDelete?: LibraryBrowserViewProps["onDeleteItems"];
}) {
	return (
		<div className="media-library-editor">
			<p className="media-library-eyebrow">Selection</p>
			<h2>{items.length} media files</h2>
			<div className="media-operator-toolbar">
				<Button
					type="button"
					disabled={busy}
					onClick={() => onSetEnabled?.(items, true)}
				>
					Enable selected
				</Button>
				<Button
					type="button"
					disabled={busy}
					onClick={() => onSetEnabled?.(items, false)}
				>
					Disable selected
				</Button>
				<Button
					type="button"
					variant="danger"
					disabled={busy}
					onClick={() => {
						if (
							globalThis.confirm(
								`Delete ${items.length} selected media files permanently? This cannot be undone.`,
							)
						)
							onDelete?.(items);
					}}
				>
					Delete selected
				</Button>
			</div>
			<LibraryNotesEditor
				label={`${items.length} media files`}
				notes={items.map((item) => item.note ?? "")}
				targets={items.map((item) => ({ kind: "item", id: item.id }))}
				busy={busy}
				onSave={onUpdateNotes}
			/>
		</div>
	);
}

function ItemEditor({
	folder,
	item,
	busy,
	onUpdate,
	onReplace,
	thumbnailUrl,
	thumbnailRevision,
	previewAspectRatio,
	onUpdateNotes,
	onDelete,
	onRetryThumbnail,
	onUploadCustomThumbnail,
}: {
	folder: number;
	item: CatalogItem;
	busy: boolean;
	onUpdate?: LibraryBrowserViewProps["onUpdateItem"];
	onReplace?: LibraryBrowserViewProps["onUploadAt"];
	thumbnailUrl: (folder: number, file: number) => string;
	thumbnailRevision: number;
	previewAspectRatio: number;
	onUpdateNotes?: LibraryBrowserViewProps["onUpdateNotes"];
	onDelete?: LibraryBrowserViewProps["onDeleteItem"];
	onRetryThumbnail?: LibraryBrowserViewProps["onRetryThumbnail"];
	onUploadCustomThumbnail?: LibraryBrowserViewProps["onUploadCustomThumbnail"];
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
			<div
				className="media-library-item-preview"
				style={{ aspectRatio: previewAspectRatio }}
			>
				<img
					src={versionedThumbnailUrl(
						thumbnailUrl(folder, item.file),
						thumbnailRevision,
					)}
					alt={`${item.name} preview`}
				/>
			</div>
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
			<SwitchField
				label="Enabled"
				checked={item.enabled !== false}
				disabled={busy}
				onChange={(event) =>
					onUpdate?.(item, { enabled: event.target.checked })
				}
			/>
			<LibraryNotesEditor
				key={item.id}
				label="this media file"
				notes={[item.note ?? ""]}
				targets={[{ kind: "item", id: item.id }]}
				busy={busy}
				onSave={onUpdateNotes}
			/>
			<button type="submit" className="ui-button primary" disabled={busy}>
				Save media
			</button>
			<ItemMediaActions {...{ folder, item, busy, name, onReplace,
				onDelete, onRetryThumbnail, onUploadCustomThumbnail }} />
		</form>
	);
}

function ItemMediaActions({ folder, item, busy, name, onReplace, onDelete,
	onRetryThumbnail, onUploadCustomThumbnail }: {
	folder: number;
	item: CatalogItem;
	busy: boolean;
	name: string;
	onReplace?: LibraryBrowserViewProps["onUploadAt"];
	onDelete?: LibraryBrowserViewProps["onDeleteItem"];
	onRetryThumbnail?: LibraryBrowserViewProps["onRetryThumbnail"];
	onUploadCustomThumbnail?: LibraryBrowserViewProps["onUploadCustomThumbnail"];
}) {
	const replacementPicker = useRef<HTMLInputElement>(null);
	const thumbnailPicker = useRef<HTMLInputElement>(null);
	const replace = (replacement?: File) => {
		if (replacement) void onReplace?.(replacement, { folder, file: item.file },
			name.trim() || item.name, true);
	};
	return <>
			<FileDropField
				label="Replacement media"
				constraints={{ mimeTypes: ["video/*", "image/*"] }}
				disabled={busy}
				onFiles={(files) => replace(files[0])}
				onOpenPicker={() => replacementPicker.current?.click()}
			/>
			<input
				ref={replacementPicker}
				hidden
				type="file"
				accept="video/*,image/*"
				onChange={(event) => {
					const replacement = event.currentTarget.files?.[0];
					event.currentTarget.value = "";
					replace(replacement);
				}}
			/>
			<p>Replacing keeps this exact folder and media number.</p>
			<div className="media-operator-toolbar">
				<Button
					type="button"
					disabled={busy}
					onClick={() => onRetryThumbnail?.(item)}
				>
					Retry thumbnail
				</Button>
				<Button
					type="button"
					disabled={busy}
					onClick={() => thumbnailPicker.current?.click()}
				>
					Upload custom thumbnail
				</Button>
			</div>
			<input
				ref={thumbnailPicker}
				hidden
				type="file"
				accept="image/png,image/jpeg,image/gif,image/webp"
				onChange={(event) => {
					const image = event.currentTarget.files?.[0];
					event.currentTarget.value = "";
					if (image) onUploadCustomThumbnail?.(item, image);
				}}
			/>
			<Button
				type="button"
				variant="danger"
				disabled={busy}
				onClick={() => {
					if (
						globalThis.confirm(
							`Delete ${item.name} permanently? This cannot be undone.`,
						)
					)
						onDelete?.(item);
				}}
			>
				Delete media
			</Button>
		</>;
}

function versionedThumbnailUrl(url: string, revision: number): string {
	return `${url}${url.includes("?") ? "&" : "?"}revision=${revision}`;
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
