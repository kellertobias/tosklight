import { useEffect, useState } from "react";
import type {
	FileEntry,
	FileNativeNote,
	FileOperationResult,
	FileRoot,
} from "../../api/types";
import { Button, TextArea } from "../../components/common";
import type {
	FileManagerOperationKind,
	FileManagerPickerOptions,
	FileManagerSelection,
} from "./types";

export const textExtensions = new Set(["txt", "md", "csv", "log"]);
export const imageExtensions = new Set(["png", "jpg", "jpeg", "gif", "webp"]);
export const audioExtensions = new Set(["mp3", "wav"]);

export function extension(name: string) {
	const value = name.split(".").pop()?.toLowerCase() ?? "";
	return value === name.toLowerCase() ? "" : value;
}

export function parentPath(path: string) {
	return path.split("/").slice(0, -1).join("/");
}

export function joinPath(parent: string, name: string) {
	return parent ? `${parent}/${name}` : name;
}

export function sortFileEntries(entries: FileEntry[]) {
	return [...entries].sort((left, right) => {
		if (left.kind !== right.kind) return left.kind === "folder" ? -1 : 1;
		return left.name.localeCompare(right.name, undefined, {
			numeric: true,
			sensitivity: "base",
		});
	});
}

export function validItemName(name: string) {
	const value = name.trim();
	const upper = value
		.replace(/[. ]+$/, "")
		.split(".")[0]
		?.toUpperCase();
	const containsControlCharacter = [...value].some((character) => {
		const code = character.charCodeAt(0);
		return code < 32 || code === 127;
	});
	const reserved = new Set([
		"CON",
		"PRN",
		"AUX",
		"NUL",
		...Array.from({ length: 9 }, (_, index) => `COM${index + 1}`),
		...Array.from({ length: 9 }, (_, index) => `LPT${index + 1}`),
	]);
	return Boolean(
		value &&
			value !== "." &&
			value !== ".." &&
			new TextEncoder().encode(value).length <= 255 &&
			!/[\\/]/.test(value) &&
			!containsControlCharacter &&
			!/[. ]$/.test(value) &&
			!reserved.has(upper),
	);
}

export function nextKeepBothName(
	name: string,
	existingNames: Iterable<string>,
) {
	const existing = new Set(
		[...existingNames].map((value) => value.toLocaleLowerCase()),
	);
	const dot = name.lastIndexOf(".");
	const hasExtension = dot > 0;
	const stem = hasExtension ? name.slice(0, dot) : name;
	const suffix = hasExtension ? name.slice(dot) : "";
	let sequence = 1;
	let candidate = `${stem} copy${suffix}`;
	while (existing.has(candidate.toLocaleLowerCase())) {
		sequence += 1;
		candidate = `${stem} copy ${sequence}${suffix}`;
	}
	return candidate;
}

export function assertFileOperationComplete(result: FileOperationResult) {
	if (result.complete) return;
	const failures = result.items
		.filter((item) => item.status === "failed")
		.map((item) => `${item.source}: ${item.error ?? "operation failed"}`);
	throw new Error(failures.join("; ") || "one or more file operations failed");
}

export function operationFromCommandLine(
	commandLine: string,
): FileManagerOperationKind | null {
	const command = commandLine.trim().toUpperCase();
	if (command === "SET") return "rename";
	if (command === "CPY" || command === "COPY") return "copy";
	if (command === "MOV" || command === "MOVE") return "move";
	if (command === "DEL" || command === "DELETE") return "delete";
	return null;
}

export function pickerSelectionIsValid(
	selection: FileManagerSelection[],
	picker: FileManagerPickerOptions,
) {
	if (!selection.length || (!picker.multiple && selection.length !== 1))
		return false;
	const target = picker.target ?? "files";
	const allowed = new Set(
		(picker.allowedExtensions ?? []).map((value) =>
			value.replace(/^\./, "").toLowerCase(),
		),
	);
	return selection.every(({ entry }) => {
		if (target === "files" && entry.kind !== "file") return false;
		if (target === "folders" && entry.kind !== "folder") return false;
		return (
			entry.kind !== "file" ||
			!allowed.size ||
			allowed.has(extension(entry.name))
		);
	});
}

export function selectionKey(selection: FileManagerSelection) {
	return `${selection.rootId}:${selection.entry.path}`;
}

export function rootIcon(root: FileRoot) {
	if (root.removable || root.icon === "drive") return "⏏";
	if (root.id === "shows" || root.icon === "shows") return "🎭";
	if (root.icon && root.icon !== "folder") return root.icon;
	return "▣";
}

export function itemIcon(item: FileEntry) {
	if (item.kind === "folder") return "📁";
	if (imageExtensions.has(extension(item.name))) return "▧";
	if (audioExtensions.has(extension(item.name))) return "♪";
	return "▤";
}

export function formatSize(value: number) {
	if (value < 1024) return `${value} B`;
	if (value < 1024 * 1024)
		return `${(value / 1024).toFixed(value < 10 * 1024 ? 1 : 0)} KB`;
	if (value < 1024 * 1024 * 1024) {
		return `${(value / (1024 * 1024)).toFixed(value < 10 * 1024 * 1024 ? 1 : 0)} MB`;
	}
	return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatTime(value: number | null) {
	return value == null ? "Unavailable" : new Date(value).toLocaleString();
}

export function safeObjectUrl(blob: Blob) {
	return typeof URL.createObjectURL === "function"
		? URL.createObjectURL(blob)
		: "";
}

export function releaseObjectUrl(url: string) {
	if (url.startsWith("blob:") && typeof URL.revokeObjectURL === "function")
		URL.revokeObjectURL(url);
}

export function isMissingFileError(error: unknown) {
	const message = String(error).toLowerCase();
	return (
		message.includes("not found") ||
		message.includes("was removed") ||
		message.includes("unavailable")
	);
}

export function RasterThumbnail({
	rootId,
	entry,
	load,
}: {
	rootId: string;
	entry: FileEntry;
	load: (rootId: string, path: string) => Promise<Blob>;
}) {
	const [url, setUrl] = useState("");
	useEffect(() => {
		let cancelled = false;
		let allocated = "";
		void load(rootId, entry.path)
			.then((blob) => {
				allocated = safeObjectUrl(blob);
				if (cancelled) releaseObjectUrl(allocated);
				else setUrl(allocated);
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
			releaseObjectUrl(allocated);
		};
	}, [entry.path, load, rootId]);
	return url ? (
		<img className="file-thumbnail" src={url} alt="" />
	) : (
		<span className="file-item-icon" aria-hidden="true">
			▧
		</span>
	);
}

export function FileProperties({
	selection,
	previewUrl,
	nativeNote,
	noteDraft,
	busy,
	onNoteDraft,
	onSaveNote,
	onOpenText,
}: {
	selection: FileManagerSelection;
	previewUrl: string;
	nativeNote: FileNativeNote | null;
	noteDraft: string;
	busy: boolean;
	onNoteDraft: (value: string) => void;
	onSaveNote: () => void;
	onOpenText?: (selection: FileManagerSelection) => void;
}) {
	const { entry } = selection;
	const image =
		entry.kind === "file" && imageExtensions.has(extension(entry.name));
	const audio =
		entry.kind === "file" && audioExtensions.has(extension(entry.name));
	const text =
		entry.kind === "file" && textExtensions.has(extension(entry.name));
	return (
		<>
			<b>{entry.name}</b>
			<dl>
				<dt>Type</dt>
				<dd>
					{entry.kind === "folder"
						? "Folder"
						: extension(entry.name).toUpperCase() || "File"}
				</dd>
				<dt>Size</dt>
				<dd>{entry.kind === "file" ? formatSize(entry.size) : "—"}</dd>
				<dt>Created</dt>
				<dd>{formatTime(entry.created_millis)}</dd>
				<dt>Modified</dt>
				<dd>{formatTime(entry.modified_millis)}</dd>
				<dt>Access</dt>
				<dd>{entry.writable ? "Read and write" : "Read only"}</dd>
			</dl>
			{entry.note_supported && nativeNote?.supported ? (
				<div className="file-notes">
					<span>Notes</span>
					<TextArea
						aria-label="Notes"
						value={noteDraft}
						onChange={(event) => onNoteDraft(event.target.value)}
					/>
					<Button
						disabled={busy || noteDraft === (nativeNote.note ?? "")}
						onClick={onSaveNote}
					>
						Save Note
					</Button>
				</div>
			) : (
				<div className="file-notes">
					<span>Notes</span>
					<TextArea
						aria-label="Notes"
						value="Notes unavailable on this filesystem"
						disabled
						readOnly
					/>
				</div>
			)}
			{text && onOpenText && (
				<Button onClick={() => onOpenText(selection)}>Edit Text</Button>
			)}
			{previewUrl && image && (
				<img
					className="file-preview"
					src={previewUrl}
					alt={`Preview of ${entry.name}`}
				/>
			)}
			{previewUrl && audio && (
				// biome-ignore lint/a11y/useMediaCaption: Arbitrary operator audio files do not have companion caption tracks.
				<audio
					aria-label={`Audio preview of ${entry.name}`}
					src={previewUrl}
					controls
					preload="metadata"
				/>
			)}
			{!image && !audio && !text && entry.kind === "file" && (
				<p>Preview unavailable for this file type.</p>
			)}
		</>
	);
}
