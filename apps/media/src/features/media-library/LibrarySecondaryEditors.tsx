import { Button, FileDropField, TextAreaField, TextField } from "@tosklight/ui/controls";
import type { ReactNode, RefObject } from "react";
import { useRef, useState } from "react";
import type { LibraryNoteTargetView } from "../../shared/api/generated/media-wire";
import { FolderPresentationEditor } from "./FolderPresentationEditor";
import type { LibraryBrowserViewProps } from "./LibraryPage";

const MEDIA_FOLDER_COUNT = 199;

export function FolderEditor({
	folder, name, icon, pictureUrl, busy, note, onSave, onSetIcon,
	onSetPicture, onRemovePicture, onUpload, onUpdateNotes, onCompact,
}: {
	folder: number;
	name: string;
	icon: string;
	pictureUrl: string | null;
	busy: boolean;
	note: string;
	onSave?: LibraryBrowserViewProps["onRenameFolder"];
	onSetIcon?: LibraryBrowserViewProps["onSetFolderIcon"];
	onSetPicture?: LibraryBrowserViewProps["onSetFolderPicture"];
	onRemovePicture?: LibraryBrowserViewProps["onRemoveFolderPicture"];
	onUpload?: LibraryBrowserViewProps["onUpload"];
	onUpdateNotes?: LibraryBrowserViewProps["onUpdateNotes"];
	onCompact?: LibraryBrowserViewProps["onCompactFolder"];
}) {
	const folderPicker = useRef<HTMLInputElement>(null);
	return (
		<FolderPresentationEditor
			presentation={{ folder, name: name || null, icon: icon || null, pictureUrl }}
			busy={busy}
			onName={(next) => onSave?.(folder, next)}
			onIcon={(next) => onSetIcon?.(folder, next)}
			onPicture={(picture) => onSetPicture?.(folder, picture)}
			onRemovePicture={() => onRemovePicture?.(folder)}
		>
			<LibraryNotesEditor
				label="this folder"
				notes={[note]}
				targets={[{ kind: "folder", folder }]}
				busy={busy}
				onSave={onUpdateNotes}
			/>
			<Button type="button" disabled={busy} onClick={() => onCompact?.(folder)}>
				Compact files
			</Button>
			<p>
				Moves this folder's files to consecutive slots starting at 1. Numeric
				media addresses will change.
			</p>
			<FileDropField
				label="Upload media to this folder"
				constraints={{ mimeTypes: ["video/*", "image/*"], multiple: true }}
				disabled={busy || !isPlayableFolder(folder)}
				onFiles={(files) => void onUpload?.(files, folder)}
				onOpenPicker={() => folderPicker.current?.click()}
			/>
			<input
				ref={folderPicker}
				hidden
				type="file"
				multiple
				accept="video/*,image/*"
				onChange={(event) => {
					const files = [...(event.currentTarget.files ?? [])];
					event.currentTarget.value = "";
					if (files.length) void onUpload?.(files, folder);
				}}
			/>
		</FolderPresentationEditor>
	);
}

export function LibraryNotesEditor({ label, notes, targets, busy, onSave }: {
	label: string;
	notes: string[];
	targets: LibraryNoteTargetView[];
	busy: boolean;
	onSave?: LibraryBrowserViewProps["onUpdateNotes"];
}) {
	const mixed = notes.some((note) => note !== notes[0]);
	const [note, setNote] = useState(mixed ? "" : (notes[0] ?? ""));
	return (
		<section className="media-library-note-editor">
			<h2>Note</h2>
			<p>Store licence, attribution, source, or other operator text with {label}.</p>
			{mixed && <p className="media-library-mixed-note">Multiple values</p>}
			<TextAreaField
				label={`Note for ${label}`}
				value={note}
				placeholder={mixed ? "Multiple values" : "Add a note"}
				onChange={(event) => setNote(event.target.value)}
			/>
			<div className="media-operator-toolbar">
				<button type="button" className="ui-button primary" disabled={busy}
					onClick={() => onSave?.(targets, note)}>
					{targets.length === 1 ? "Save note" : `Save note to ${targets.length}`}
				</button>
				<button type="button" className="ui-button" disabled={busy}
					onClick={() => { setNote(""); onSave?.(targets, ""); }}>
					Clear note{targets.length === 1 ? "" : "s"}
				</button>
			</div>
		</section>
	);
}

export function EmptySlotEditor({ folder, file, busy, onUpload }: {
	folder: number;
	file: number;
	busy: boolean;
	onUpload?: LibraryBrowserViewProps["onUploadAt"];
}) {
	const [name, setName] = useState("");
	const mediaPicker = useRef<HTMLInputElement>(null);
	const upload = (media?: File) => {
		if (media) void onUpload?.(media, { folder, file }, name.trim(), false);
	};
	return (
		<div className="media-library-editor">
			<p className="media-library-eyebrow">Empty media slot</p>
			<h2>Empty</h2>
			<p className="media-library-address">
				{String(folder).padStart(3, "0")} / {String(file).padStart(3, "0")}
			</p>
			<TextField label="Media name" value={name}
				onChange={(event) => setName(event.target.value)}
				placeholder="Name this media" required />
			<FileDropField label="Media file"
				constraints={{ mimeTypes: ["video/*", "image/*"] }}
				disabled={busy || !name.trim()}
				onFiles={(files) => upload(files[0])}
				onOpenPicker={() => mediaPicker.current?.click()} />
			<input ref={mediaPicker} hidden type="file" accept="video/*,image/*"
				onChange={(event) => {
					const media = event.currentTarget.files?.[0];
					event.currentTarget.value = "";
					upload(media);
				}} />
			<p>The upload is assigned directly to this slot.</p>
		</div>
	);
}

export function UploadEditor({ folder, busy, picker, onUpload, importPanel }: {
	folder: number;
	busy: boolean;
	picker: RefObject<HTMLInputElement | null>;
	onUpload?: LibraryBrowserViewProps["onUpload"];
	importPanel?: ReactNode;
}) {
	return (
		<div className="media-library-editor">
			<p className="media-library-eyebrow">Folder {String(folder).padStart(3, "0")}</p>
			<h2>Add media</h2>
			<p>Files take the first free slots. If this folder fills up, allocation continues in the next media folder.</p>
			<input ref={picker} hidden type="file" multiple accept="video/*,image/*"
				onChange={(event) => {
					const files = [...(event.target.files ?? [])];
					event.currentTarget.value = "";
					if (files.length) void onUpload?.(files, folder);
				}} />
			<FileDropField label="Media files"
				constraints={{ mimeTypes: ["video/*", "image/*"], multiple: true }}
				disabled={busy}
				onFiles={(files) => void onUpload?.(files, folder)}
				onOpenPicker={() => picker.current?.click()} />
			{importPanel}
		</div>
	);
}

export function isPlayableFolder(folder: number) {
	return folder >= 1 && folder <= MEDIA_FOLDER_COUNT;
}
