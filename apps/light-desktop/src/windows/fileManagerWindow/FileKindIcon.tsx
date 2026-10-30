import "./FileKindIcon.css";
import type { ReactNode } from "react";
import type { FileEntry, FileRoot } from "../../api/types";
import {
	audioExtensions,
	extension,
	imageExtensions,
	textExtensions,
} from "./fileUtilities";

/**
 * Visual kinds of File Manager entries. Each kind owns one outline glyph and one
 * accent colour so folders, shows, media and text stay distinguishable at a glance.
 */
export type FileKind =
	| "folder"
	| "show"
	| "image"
	| "audio"
	| "text"
	| "file"
	| "location-shows"
	| "location-drive"
	| "location";

export function fileKind(entry: FileEntry): FileKind {
	if (entry.kind === "folder") return "folder";
	const suffix = extension(entry.name);
	if (suffix === "show") return "show";
	if (imageExtensions.has(suffix)) return "image";
	if (audioExtensions.has(suffix)) return "audio";
	if (textExtensions.has(suffix)) return "text";
	return "file";
}

export function rootKind(root: FileRoot): FileKind {
	if (root.removable || root.icon === "drive") return "location-drive";
	if (root.id === "shows" || root.icon === "shows") return "location-shows";
	return "location";
}

const page = (
	<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z M14 2v6h6" />
);

const glyphs: Record<FileKind, ReactNode> = {
	folder: (
		<path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
	),
	show: (
		<>
			{page}
			<circle cx="12" cy="11.5" r="1.6" />
			<path d="m9 18.5 3-4.5 3 4.5" />
		</>
	),
	image: (
		<>
			<rect width="18" height="18" x="3" y="3" rx="2" />
			<circle cx="9" cy="9" r="2" />
			<path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21" />
		</>
	),
	audio: (
		<>
			<path d="M9 18V5l12-2v13" />
			<circle cx="6" cy="18" r="3" />
			<circle cx="18" cy="16" r="3" />
		</>
	),
	text: (
		<>
			{page}
			<path d="M8 13h8M8 17h6" />
		</>
	),
	file: page,
	"location-shows": (
		<>
			<path d="M4 20h16" />
			<path d="M6 20V9l6-5 6 5v11" />
			<path d="M10 20v-5h4v5" />
		</>
	),
	"location-drive": (
		<>
			<rect width="20" height="8" x="2" y="13" rx="2" />
			<path d="M5.5 5h13l3 8h-19Z" />
			<path d="M6 17h.01M10 17h.01" />
		</>
	),
	location: (
		<>
			<rect width="20" height="14" x="2" y="5" rx="2" />
			<path d="M2 10h20" />
		</>
	),
};

export function FileKindIcon({ kind }: { kind: FileKind }) {
	return (
		<svg
			className={`file-kind-icon file-kind-${kind}`}
			aria-hidden="true"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.7"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			{glyphs[kind]}
		</svg>
	);
}

export function TreeChevron({ expanded }: { expanded: boolean }) {
	return (
		<svg
			className={`file-tree-chevron ${expanded ? "is-expanded" : ""}`}
			aria-hidden="true"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2.2"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			<path d="m9 6 6 6-6 6" />
		</svg>
	);
}
