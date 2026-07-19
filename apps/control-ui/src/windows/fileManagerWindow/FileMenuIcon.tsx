import type { ReactNode } from "react";

type FileMenuIconName =
	| "chevron"
	| "rename"
	| "copy"
	| "move"
	| "delete"
	| "file-new"
	| "folder-new"
	| "list"
	| "grid"
	| "folder";

export function FileMenuIcon({ name }: { name: FileMenuIconName }) {
	const paths: Record<Exclude<FileMenuIconName, "grid">, ReactNode> = {
		chevron: <path d="m6 9 6 6 6-6" />,
		rename: (
			<>
				<path d="M12 20h9" />
				<path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
			</>
		),
		copy: (
			<>
				<rect width="13" height="13" x="9" y="9" rx="2" />
				<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
			</>
		),
		move: (
			<>
				<path d="M5 9h11" />
				<path d="m13 6 3 3-3 3" />
				<path d="M19 15H8" />
				<path d="m11 12-3 3 3 3" />
			</>
		),
		delete: (
			<>
				<path d="M3 6h18" />
				<path d="M8 6V4h8v2" />
				<path d="m19 6-1 14H6L5 6" />
				<path d="M10 11v5M14 11v5" />
			</>
		),
		"file-new": (
			<>
				<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
				<path d="M14 2v6h6M12 18v-6M9 15h6" />
			</>
		),
		"folder-new": (
			<>
				<path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
				<path d="M12 11v6M9 14h6" />
			</>
		),
		list: (
			<>
				<path d="M8 6h13M8 12h13M8 18h13" />
				<path d="M3 6h.01M3 12h.01M3 18h.01" />
			</>
		),
		folder: (
			<path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
		),
	};
	return (
		<svg
			className="file-menu-icon"
			aria-hidden="true"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.8"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			{name === "grid" ? (
				<>
					<rect width="7" height="7" x="3" y="3" />
					<rect width="7" height="7" x="14" y="3" />
					<rect width="7" height="7" x="3" y="14" />
					<rect width="7" height="7" x="14" y="14" />
				</>
			) : (
				paths[name]
			)}
		</svg>
	);
}
