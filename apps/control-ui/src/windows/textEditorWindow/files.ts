import type { FileEntry, TextDocument } from "../../api/types";

export const TEXT_FILE_EXTENSIONS = new Set(["txt", "md", "csv", "log"]);
export const EXTERNAL_CHECK_INTERVAL_MILLIS = 1_500;
export const MAX_TEXT_FILE_BYTES = 4 * 1024 * 1024;
const FILE_CHOOSER_DIRECTORY_LIMIT = 256;
const FILE_CHOOSER_FILE_LIMIT = 2_000;

export type Availability = "none" | "loading" | "ready" | "missing";
export type Notice = {
	kind: "info" | "error" | "conflict";
	text: string;
} | null;

export interface LegacyTextEditorViewState {
	selectionStart: number;
	selectionEnd: number;
	scrollTop: number;
}

function extension(path: string) {
	return path.split(".").pop()?.toLowerCase() ?? "";
}

export function parentDirectory(path: string) {
	return path.split("/").slice(0, -1).join("/");
}

export function isSupportedTextFile(path: string) {
	return TEXT_FILE_EXTENSIONS.has(extension(path));
}

export function isSameDocumentVersion(
	left: TextDocument | null,
	right: TextDocument,
) {
	return Boolean(
		left &&
			left.root_id === right.root_id &&
			left.path === right.path &&
			left.revision === right.revision &&
			left.text === right.text &&
			left.read_only === right.read_only,
	);
}

export function friendlyError(error: unknown) {
	const raw = error instanceof Error ? error.message : String(error);
	try {
		const parsed = JSON.parse(raw) as { error?: unknown };
		if (typeof parsed.error === "string") return parsed.error;
	} catch {
		// Non-JSON errors already carry the most useful message available.
	}
	return raw.replace(/^Error:\s*/, "");
}

export function isMissingError(error: unknown) {
	const message = friendlyError(error).toLowerCase();
	return (
		message.includes("not found") ||
		message.includes("was removed") ||
		message.includes("unavailable")
	);
}

export function viewStateKey(
	paneId: string | undefined,
	root: string,
	path: string,
) {
	return `light.text-editor-view.${paneId ?? "window"}.${root}.${path}`;
}

/** Resolve supported files below a root without allowing an unbounded crawl. */
export async function listTextEditorFiles(
	fileEntries: (
		root: string,
		path?: string,
		hidden?: boolean,
	) => Promise<{ entries: FileEntry[] }>,
	root: string,
) {
	const directories = [""];
	const files: FileEntry[] = [];
	let visitedDirectories = 0;
	while (
		directories.length &&
		visitedDirectories < FILE_CHOOSER_DIRECTORY_LIMIT &&
		files.length < FILE_CHOOSER_FILE_LIMIT
	) {
		const path = directories.shift() as string;
		const listing = await fileEntries(root, path, false);
		visitedDirectories += 1;
		for (const entry of listing.entries) {
			if (entry.kind === "folder") directories.push(entry.path);
			else if (isSupportedTextFile(entry.path)) files.push(entry);
			if (files.length >= FILE_CHOOSER_FILE_LIMIT) break;
		}
	}
	return {
		files: files.sort((left, right) =>
			left.path.localeCompare(right.path, undefined, { sensitivity: "base" }),
		),
		truncated:
			directories.length > 0 || files.length >= FILE_CHOOSER_FILE_LIMIT,
	};
}
