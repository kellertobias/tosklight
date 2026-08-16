import { FileDropField, IconPickerField, TextField } from "@tosklight/ui/controls";
import { type ReactNode, useEffect, useRef, useState } from "react";

export interface FolderPresentation {
	folder: number;
	name: string | null;
	icon: string | null;
	pictureUrl: string | null;
}

export function FolderPresentationEditor({
	presentation,
	busy = false,
	onName,
	onIcon,
	onPicture,
	onRemovePicture,
	children,
}: {
	presentation: FolderPresentation;
	busy?: boolean;
	onName?: (name: string) => void | Promise<void>;
	onIcon?: (icon: string) => void | Promise<void>;
	onPicture?: (picture: File) => void | Promise<void>;
	onRemovePicture?: () => void | Promise<void>;
	children?: ReactNode;
}) {
	const [name, setName] = useState(presentation.name ?? "");
	const [icon, setIcon] = useState(presentation.icon ?? "");
	const picker = useRef<HTMLInputElement>(null);
	const lastSavedName = useRef(name);

	useEffect(() => {
		const next = presentation.name ?? "";
		lastSavedName.current = next;
		setName(next);
	}, [presentation.name]);
	useEffect(() => setIcon(presentation.icon ?? ""), [presentation.icon]);
	useEffect(() => {
		if (name === lastSavedName.current) return;
		const timer = window.setTimeout(() => {
			lastSavedName.current = name;
			void onName?.(name);
		}, 350);
		return () => window.clearTimeout(timer);
	}, [name, onName]);

	return (
		<div className="media-library-editor media-folder-presentation-editor">
			<p className="media-library-eyebrow">
				Folder {String(presentation.folder).padStart(3, "0")}
			</p>
			<h2>Configure folder</h2>
			{presentation.pictureUrl ? (
				<figure className="media-folder-picture">
					<img src={presentation.pictureUrl} alt="Folder preview" />
				</figure>
			) : null}
			<TextField
				label="Folder name"
				value={name}
				onChange={(event) => setName(event.target.value)}
				placeholder="Empty folder"
				disabled={busy}
			/>
			<IconPickerField
				label="Folder icon"
				value={icon}
				disabled={busy}
				onChange={(next) => {
					setIcon(next);
					if (next !== (presentation.icon ?? "")) void onIcon?.(next);
				}}
			/>
			<FileDropField
				label={presentation.pictureUrl ? "Replace folder picture" : "Folder picture"}
				constraints={{ mimeTypes: ["image/*"] }}
				disabled={busy}
				onFiles={(files) => {
					const picture = files[0];
					if (picture) void onPicture?.(picture);
				}}
				onOpenPicker={() => picker.current?.click()}
			/>
			<input
				ref={picker}
				hidden
				type="file"
				accept="image/*"
				onChange={(event) => {
					const picture = event.currentTarget.files?.[0];
					event.currentTarget.value = "";
					if (picture) void onPicture?.(picture);
				}}
			/>
			{presentation.pictureUrl ? (
				<button
					type="button"
					className="ui-button secondary"
					disabled={busy}
					onClick={() => void onRemovePicture?.()}
				>
					Remove folder picture
				</button>
			) : null}
			<p className="media-settings-save-state" role="status" aria-live="polite">
				{busy ? "Saving…" : "Saved automatically"}
			</p>
			{children}
		</div>
	);
}
