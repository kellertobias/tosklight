// The library, as the desk addresses it.
//
// The address is the point of this page: an operator reading a cue sheet needs to see which
// folder and file number reaches which clip, so the DMX address is the first column and the file
// name is a detail beside it.

import {
	Button,
	CheckboxField,
	NumberField,
	SearchBar,
	TextField,
} from "@tosklight/ui/controls";
import { Fragment, useMemo, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel, folderLabel, itemDetail } from "../../entities/catalog";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog } from "../../shared/api/queries";
import { ImportPanel } from "./ImportPanel";

const CATALOG_POLL_MS = 15_000;

export function LibraryPage() {
	const catalog = useCatalog(CATALOG_POLL_MS);
	const [search, setSearch] = useState("");
	const editing = useEditing(catalog.reload);

	return (
		<section className="media-page">
			<ImportPanel onImported={catalog.reload} />
			<UploadForm
				busy={editing.busy}
				onUpload={(folder, file, name, media) =>
					void editing.save(() =>
						api.uploadLibraryItem(folder, file, requestId(), name, media),
					)
				}
			/>
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}

			<SearchBar
				value={search}
				onChange={setSearch}
				ariaLabel="Search the library"
				placeholder="Search the library"
			/>
			<ResourceState
				resource={catalog}
				subject="the library"
				isEmpty={(data) => data.itemCount === 0}
				empty="Nothing in the library can be played yet."
			>
				{(data) => (
					<Folders
						catalog={data}
						search={search}
						editing={editing.editing}
						busy={editing.busy}
						onBegin={editing.begin}
						onCancel={editing.cancel}
						onSave={editing.save}
					/>
				)}
			</ResourceState>
		</section>
	);
}

function Folders({
	catalog,
	search,
	editing,
	busy,
	onBegin,
	onCancel,
	onSave,
}: {
	catalog: CatalogView;
	search: string;
	editing: string | undefined;
	busy: boolean;
	onBegin: (key: string) => void;
	onCancel: () => void;
	onSave: (save: () => Promise<unknown>) => Promise<void>;
}) {
	const folders = useMemo(() => matching(catalog, search), [catalog, search]);

	if (folders.length === 0) {
		return (
			<p className="media-state is-empty">
				Nothing in the library matches “{search}”.
			</p>
		);
	}

	return (
		<>
			{folders.map((folder) => (
				<FolderSection
					key={folder.folder}
					folder={folder}
					editing={editing}
					busy={busy}
					onBegin={onBegin}
					onCancel={onCancel}
					onSave={onSave}
				/>
			))}
		</>
	);
}

function FolderSection({
	folder,
	editing,
	busy,
	onBegin,
	onCancel,
	onSave,
}: {
	folder: CatalogView["folders"][number];
	editing: string | undefined;
	busy: boolean;
	onBegin: (key: string) => void;
	onCancel: () => void;
	onSave: (save: () => Promise<unknown>) => Promise<void>;
}) {
	return (
		<section className="media-folder" aria-label={folderLabel(folder)}>
			<div className="media-folder-heading">
				<h2>{folderLabel(folder)}</h2>
				<Button
					size="compact"
					onClick={() => onBegin(`folder-${folder.folder}`)}
				>
					Change folder name
				</Button>
			</div>
			{editing === `folder-${folder.folder}` && (
				<FolderNameForm
					folder={folder.folder}
					name={folder.name ?? ""}
					busy={busy}
					onCancel={onCancel}
					onSave={(name) =>
						void onSave(() =>
							api.updateLibraryFolder(folder.folder, {
								requestId: requestId(),
								name,
							}),
						)
					}
				/>
			)}
			<FolderItemsTable
				{...{ folder, editing, busy, onBegin, onCancel, onSave }}
			/>
		</section>
	);
}

interface FolderEditingProps {
	editing: string | undefined;
	busy: boolean;
	onBegin: (key: string) => void;
	onCancel: () => void;
	onSave: (save: () => Promise<unknown>) => Promise<void>;
}

function FolderItemsTable({
	folder,
	...editingProps
}: { folder: CatalogView["folders"][number] } & FolderEditingProps) {
	return (
		<table className="media-table">
			<caption className="media-visually-hidden">
				Items in {folderLabel(folder)}
			</caption>
			<thead>
				<tr>
					<th scope="col">Thumbnail</th>
					<th scope="col">Address</th>
					<th scope="col">Name</th>
					<th scope="col">Detail</th>
					<th scope="col">Tempo</th>
					<th scope="col">Actions</th>
				</tr>
			</thead>
			<tbody>
				{folder.items.map((item) => (
					<LibraryItemRows
						key={item.id}
						folder={folder.folder}
						item={item}
						{...editingProps}
					/>
				))}
			</tbody>
		</table>
	);
}

function LibraryItemRows({
	folder,
	item,
	editing,
	busy,
	onBegin,
	onCancel,
	onSave,
}: {
	folder: number;
	item: CatalogView["folders"][number]["items"][number];
} & FolderEditingProps) {
	return (
		<Fragment>
			<tr>
				<td>
					<Thumbnail folder={folder} file={item.file} name={item.name} />
				</td>
				<td>{addressLabel(folder, item.file)}</td>
				<td>{item.name}</td>
				<td>{itemDetail(item)}</td>
				<td>{item.intrinsicBpm === null ? "—" : `${item.intrinsicBpm} BPM`}</td>
				<td className="media-library-actions">
					<Button size="compact" onClick={() => onBegin(`rename-${item.id}`)}>
						Rename
					</Button>
					<Button size="compact" onClick={() => onBegin(`move-${item.id}`)}>
						Move
					</Button>
				</td>
			</tr>
			{editing === `rename-${item.id}` && (
				<tr>
					<td colSpan={6}>
						<RenameItemForm
							name={item.name}
							busy={busy}
							onCancel={onCancel}
							onSave={(name) =>
								void onSave(() =>
									api.updateLibraryItem(item.id, {
										requestId: requestId(),
										name,
										swap: false,
									}),
								)
							}
						/>
					</td>
				</tr>
			)}
			{editing === `move-${item.id}` && (
				<tr>
					<td colSpan={6}>
						<MoveItemForm
							folder={folder}
							file={item.file}
							busy={busy}
							onCancel={onCancel}
							onSave={(destinationFolder, destinationFile, swap) =>
								void onSave(() =>
									api.updateLibraryItem(item.id, {
										requestId: requestId(),
										folder: destinationFolder,
										file: destinationFile,
										swap,
									}),
								)
							}
						/>
					</td>
				</tr>
			)}
		</Fragment>
	);
}

function Thumbnail({
	folder,
	file,
	name,
}: {
	folder: number;
	file: number;
	name: string;
}) {
	const [missing, setMissing] = useState(false);
	if (missing)
		return <span className="media-thumbnail-missing">No thumbnail</span>;
	return (
		<img
			className="media-thumbnail"
			src={api.thumbnailUrl(folder, file)}
			alt={`${name} thumbnail`}
			onError={() => setMissing(true)}
		/>
	);
}

function FolderNameForm({
	folder,
	name,
	busy,
	onSave,
	onCancel,
}: {
	folder: number;
	name: string;
	busy: boolean;
	onSave: (name: string) => void;
	onCancel: () => void;
}) {
	const [next, setNext] = useState(name);
	return (
		<form
			className="media-inline-editor"
			onSubmit={(event) => {
				event.preventDefault();
				onSave(next);
			}}
		>
			<TextField
				label={`Folder ${folder} name`}
				value={next}
				onChange={(event) => setNext(event.target.value)}
			/>
			<span className="media-inline-note">
				Leave empty to remove the folder name.
			</span>
			<Button type="submit" disabled={busy}>
				Save
			</Button>
			<Button type="button" onClick={onCancel}>
				Cancel
			</Button>
		</form>
	);
}

function RenameItemForm({
	name,
	busy,
	onSave,
	onCancel,
}: {
	name: string;
	busy: boolean;
	onSave: (name: string) => void;
	onCancel: () => void;
}) {
	const [next, setNext] = useState(name);
	return (
		<form
			className="media-inline-editor"
			onSubmit={(event) => {
				event.preventDefault();
				onSave(next);
			}}
		>
			<TextField
				label="Media name"
				value={next}
				onChange={(event) => setNext(event.target.value)}
				required
			/>
			<Button type="submit" disabled={busy}>
				Save name
			</Button>
			<Button type="button" onClick={onCancel}>
				Cancel
			</Button>
		</form>
	);
}

function MoveItemForm({
	folder,
	file,
	busy,
	onSave,
	onCancel,
}: {
	folder: number;
	file: number;
	busy: boolean;
	onSave: (folder: number, file: number, swap: boolean) => void;
	onCancel: () => void;
}) {
	const [nextFolder, setNextFolder] = useState(folder);
	const [nextFile, setNextFile] = useState(file);
	const [swap, setSwap] = useState(false);
	return (
		<form
			className="media-inline-editor"
			onSubmit={(event) => {
				event.preventDefault();
				onSave(nextFolder, nextFile, swap);
			}}
		>
			<NumberField
				label="Folder"
				value={nextFolder}
				min={1}
				max={199}
				onChange={(event) => setNextFolder(Number(event.target.value))}
			/>
			<NumberField
				label="File"
				value={nextFile}
				min={1}
				max={254}
				onChange={(event) => setNextFile(Number(event.target.value))}
			/>
			<CheckboxField
				label="Exchange with the item already there"
				stateLabel="Move by safely swapping the two media items"
				checked={swap}
				onChange={(event) => setSwap(event.target.checked)}
			/>
			<Button type="submit" disabled={busy}>
				Move media
			</Button>
			<Button type="button" onClick={onCancel}>
				Cancel
			</Button>
		</form>
	);
}

function UploadForm({
	busy,
	onUpload,
}: {
	busy: boolean;
	onUpload: (folder: number, file: number, name: string, media: File) => void;
}) {
	const [folder, setFolder] = useState(1);
	const [file, setFile] = useState(1);
	const [name, setName] = useState("");
	const [media, setMedia] = useState<File>();
	return (
		<section className="media-import-panel" aria-label="Upload media">
			<h2>Upload media</h2>
			<p>
				Choose an unused desk address. The original is kept and converted to HAP
				Alpha as a visible import job.
			</p>
			<form
				className="media-inline-editor"
				onSubmit={(event) => {
					event.preventDefault();
					if (media)
						onUpload(
							folder,
							file,
							name || media.name.replace(/\.[^.]+$/u, ""),
							media,
						);
				}}
			>
				<NumberField
					label="Folder"
					value={folder}
					min={1}
					max={199}
					onChange={(event) => setFolder(Number(event.target.value))}
				/>
				<NumberField
					label="File"
					value={file}
					min={1}
					max={254}
					onChange={(event) => setFile(Number(event.target.value))}
				/>
				<TextField
					label="Media name"
					value={name}
					onChange={(event) => setName(event.target.value)}
				/>
				<label>
					Source file
					<input
						type="file"
						accept="video/*,image/*"
						required
						onChange={(event) => setMedia(event.target.files?.[0])}
					/>
				</label>
				<Button type="submit" disabled={busy || !media}>
					Upload and import
				</Button>
			</form>
		</section>
	);
}

/** Folders whose name, or one of whose items, matches. An empty search matches everything. */
function matching(
	catalog: CatalogView,
	search: string,
): CatalogView["folders"] {
	const needle = search.trim().toLowerCase();
	if (!needle) return catalog.folders;
	return catalog.folders
		.map((folder) => ({
			...folder,
			items: folder.items.filter((item) =>
				item.name.toLowerCase().includes(needle),
			),
		}))
		.filter(
			(folder) =>
				folder.items.length > 0 ||
				(folder.name ?? "").toLowerCase().includes(needle),
		);
}
