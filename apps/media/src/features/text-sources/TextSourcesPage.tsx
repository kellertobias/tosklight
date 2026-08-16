import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { addressLabel } from "../../entities/catalog";
import { MediaPreview } from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { TextSlotView } from "../../shared/api/generated/media-wire";
import { useFolderPresentations, useText } from "../../shared/api/queries";
import { useMainOutputAspectRatio } from "../../shared/output/useMainOutputAspectRatio";
import { FolderPresentationEditor } from "../media-library/FolderPresentationEditor";
import {
	GeneratedLibraryBrowserView,
	type LibrarySourceType,
} from "../media-library/GeneratedLibraryBrowserView";
import {
	draftOf,
	emptyDraft,
	KINDS,
	payloadOf,
	type TextDraft,
	TextSourceEditor,
} from "./TextSourceEditor";

export function key(slot: TextSlotView): string {
	return `${slot.address.folder}/${slot.address.file}`;
}
const NEW = "new";

export function TextSourcesPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const text = useText();
	const folderPresentations = useFolderPresentations();
	const editing = useEditing(text.reload);
	const folderEditing = useEditing(folderPresentations.reload);
	const [draft, setDraft] = useState<TextDraft>(emptyDraft());
	const [address, setAddress] = useState({ folder: 200, file: 3 });
	const [selectedKey, setSelectedKey] = useState<string | null>(null);
	const [inspectedFolder, setInspectedFolder] = useState<number | null>(null);
	const aspectRatio = useMainOutputAspectRatio();

	useEffect(() => {
		if (selectedKey === null && text.data?.[0])
			setSelectedKey(key(text.data[0]));
	}, [selectedKey, text.data]);

	useEffect(() => {
		const selected = text.data?.find((slot) => key(slot) === selectedKey);
		if (!selected) return;
		setDraft(draftOf(selected));
		editing.begin(key(selected));
	}, [selectedKey, text.data, editing.begin]);

	const beginNew = () => {
		setDraft(emptyDraft());
		setAddress(nextFreeAddress(text.data ?? []));
		editing.begin(NEW);
	};

	return (
		<section className="media-page media-library-page">
			{(editing.failure ?? folderEditing.failure) && (
				<MediaErrorToast
					message={
						(editing.failure ?? folderEditing.failure)?.message ??
						"Folder operation failed"
					}
					onDismiss={() => {
						editing.dismiss();
						folderEditing.dismiss();
					}}
				/>
			)}
			<ResourceState resource={text} subject="the text sources">
				{(data) => {
					const selected = data.find((slot) => key(slot) === selectedKey);
					return (
						<GeneratedLibraryBrowserView
							type="text"
							inspectedFolder={inspectedFolder}
							onInspectFolder={setInspectedFolder}
							folderPresentations={folderPresentations.data?.folders ?? []}
							renderFolderDetail={(folder) => {
								const presentation = folderPresentations.data?.folders.find(
									(candidate) => candidate.folder === folder,
								) ?? { folder, name: null, icon: null, pictureUrl: null };
								return (
									<FolderPresentationEditor
										presentation={presentation}
										busy={folderEditing.busy}
										onName={(name) =>
											folderEditing.save(() =>
												api.updateFolderPresentation(folder, {
													requestId: requestId(),
													name,
												}),
											)
										}
										onIcon={(icon) =>
											folderEditing.save(() =>
												api.updateFolderPresentation(folder, {
													requestId: requestId(),
													icon,
												}),
											)
										}
										onPicture={(picture) =>
											folderEditing.save(() =>
												api.uploadFolderPicture(folder, requestId(), picture),
											)
										}
										onRemovePicture={() =>
											folderEditing.save(() =>
												api.removeFolderPicture(folder, requestId()),
											)
										}
									/>
								);
							}}
							items={data.map((slot) => ({
								id: key(slot),
								folder: slot.address.folder,
								file: slot.address.file,
								name: slot.name,
								detail: describe(slot),
								image: {
									src: textPreviewUrl(slot, aspectRatio),
									alt: `${slot.name} preview`,
								},
							}))}
							selectedId={editing.editing === NEW ? "" : (selectedKey ?? "")}
							onSelect={(next) => {
								setInspectedFolder(null);
								editing.cancel();
								setSelectedKey(next);
								const nextSlot = data.find((slot) => key(slot) === next);
								if (nextSlot) {
									setDraft(draftOf(nextSlot));
									editing.begin(next);
								}
							}}
							onTypeChange={onModeChange}
							headerActions={[
								{
									id: "new-text-source",
									label: "New text source",
									onPress: beginNew,
								},
							]}
							showDetail={editing.editing === NEW}
							detail={
								editing.editing === NEW ? (
									<NewTextDetail
										draft={draft}
										address={address}
										busy={editing.busy}
										onDraft={setDraft}
										onAddress={setAddress}
										onCancel={editing.cancel}
										onSave={() =>
											void editing.save(() =>
												api.createText({
													requestId: requestId(),
													folder: address.folder,
													file: address.file,
													name: draft.name,
													kind: draft.kind,
													style: draft.style,
													...payloadOf(draft),
												}),
											)
										}
									/>
								) : selected ? (
									<TextDetail
										slot={selected}
										aspectRatio={aspectRatio}
										busy={editing.busy}
										draft={draft}
										onDraft={(next) => {
											setDraft(next);
											editing.saveLive(() =>
												api.updateText(
													selected.address.folder,
													selected.address.file,
													{
														requestId: requestId(),
														name: next.name,
														enabled: next.enabled,
														kind: next.kind,
														style: next.style,
														...payloadOf(next),
													},
												),
											);
										}}
										onDelete={() =>
											void editing.save(() =>
												api.deleteText(
													selected.address.folder,
													selected.address.file,
													{ requestId: requestId() },
												),
											)
										}
									/>
								) : (
									<p>No text source is selected.</p>
								)
							}
							emptyDetail={
								<div className="media-library-reserved-copy">
									<h2>Empty text folder</h2>
									<p>Create a text source in this address range.</p>
									<Button onClick={beginNew}>New text source</Button>
								</div>
							}
						/>
					);
				}}
			</ResourceState>
		</section>
	);
}

function NewTextDetail({
	draft,
	address,
	busy,
	onDraft,
	onAddress,
	onCancel,
	onSave,
}: {
	draft: TextDraft;
	address: { folder: number; file: number };
	busy: boolean;
	onDraft: (draft: TextDraft) => void;
	onAddress: (address: { folder: number; file: number }) => void;
	onCancel: () => void;
	onSave: () => void;
}) {
	return (
		<div className="media-library-editor">
			<h2>New text source</h2>
			<TextSourceEditor
				draft={draft}
				address={address}
				onAddress={onAddress}
				busy={busy}
				submitLabel="Create"
				onChange={onDraft}
				onCancel={onCancel}
				onSave={onSave}
			/>
		</div>
	);
}

function TextDetail({
	slot,
	aspectRatio,
	busy,
	draft,
	onDraft,
	onDelete,
}: {
	slot: TextSlotView;
	aspectRatio: number;
	busy: boolean;
	draft: TextDraft;
	onDraft: (draft: TextDraft) => void;
	onDelete: () => void;
}) {
	const [now, setNow] = useState(() => Date.now());
	useEffect(() => {
		const interval = window.setInterval(() => setNow(Date.now()), 250);
		return () => window.clearInterval(interval);
	}, []);
	const previewText = formatDraftPreview(draft, now);
	const colour = `rgb(${Math.round(draft.style.red * 255)} ${Math.round(
		draft.style.green * 255,
	)} ${Math.round(draft.style.blue * 255)})`;
	return (
		<div className="media-library-editor media-generated-library-detail">
			<div className="media-generated-sticky-region">
				<MediaPreview
					title={draft.name || slot.name}
					meta={addressLabel(slot.address.folder, slot.address.file)}
					variant="text"
					aspectRatio={aspectRatio}
				>
					<span
						className="media-text-preview-words"
						style={{
							fontFamily: draft.style.family,
							fontSize: `${Math.max(1, draft.style.size * 100)}%`,
							fontWeight: draft.style.bold ? 700 : 400,
							fontStyle: draft.style.italic ? "italic" : "normal",
							textAlign: draft.style.alignment as "left" | "center" | "right",
							color: colour,
						}}
					>
						{previewText}
					</span>
				</MediaPreview>
				<TextSourceEditor
					draft={draft}
					busy={busy}
					onChange={onDraft}
					part="identity"
				/>
			</div>
			<TextSourceEditor
				draft={draft}
				busy={busy}
				onChange={onDraft}
				part="sections"
			/>
			<div className="media-settings-actions">
				<Button onClick={onDelete}>Remove</Button>
			</div>
		</div>
	);
}

function textPreviewUrl(slot: TextSlotView, aspectRatio: number): string {
	const width = 640;
	const height = Math.max(180, Math.round(width / aspectRatio));
	const words = escapeSvg(formatDraftPreview(draftOf(slot), Date.now()));
	const red = Math.round(slot.style.red * 255);
	const green = Math.round(slot.style.green * 255);
	const blue = Math.round(slot.style.blue * 255);
	const [x, anchor] =
		slot.style.alignment === "left"
			? ["4%", "start"]
			: slot.style.alignment === "right"
				? ["96%", "end"]
				: ["50%", "middle"];
	const family = escapeSvg(slot.style.family);
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}"><rect width="100%" height="100%" fill="#081018"/><path d="M0 ${height * 0.72} L${width} ${height * 0.35}" stroke="#172b3d" stroke-width="2"/><text x="${x}" y="52%" dominant-baseline="middle" text-anchor="${anchor}" font-family="${family}" font-size="${Math.max(28, height * slot.style.size)}" font-weight="${slot.style.bold ? 700 : 400}" font-style="${slot.style.italic ? "italic" : "normal"}" fill="rgb(${red} ${green} ${blue})">${words}</text></svg>`;
	return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export function formatDraftPreview(
	draft: TextDraft,
	nowUnixMillis: number,
): string {
	if (draft.kind === "static") return draft.text;
	const separator = draft.format.separator || ":";
	if (draft.kind === "clock") {
		const at = new Date(nowUnixMillis + draft.format.utcOffsetMinutes * 60_000);
		const hour24 = at.getUTCHours();
		const hour12 = hour24 % 12 || 12;
		const pad = (value: number) => String(value).padStart(2, "0");
		const twelve = draft.format.clockPattern.startsWith("hh");
		const seconds =
			draft.format.clockPattern.endsWith("SS") ||
			draft.format.clockPattern.endsWith("ss");
		return [
			pad(twelve ? hour12 : hour24),
			pad(at.getUTCMinutes()),
			...(seconds ? [pad(at.getUTCSeconds())] : []),
		].join(separator);
	}
	let remaining =
		draft.kind === "countdown-target"
			? (draft.target ? Date.parse(draft.target) : nowUnixMillis) -
				nowUnixMillis
			: draft.durationSeconds * 1_000;
	let sign = "";
	if (remaining < 0) {
		if (draft.format.afterZero === "hold") remaining = 0;
		else {
			if (draft.format.afterZero === "negative") sign = "-";
			remaining = Math.abs(remaining);
		}
	}
	const total = Math.ceil(remaining / 1_000);
	const pattern = draft.format.countdownPattern;
	if (pattern === "ss") return `${sign}${String(total).padStart(2, "0")}`;
	if (pattern === "mm")
		return `${sign}${String(Math.ceil(total / 60)).padStart(2, "0")}`;
	if (pattern === "mm:ss") {
		const minutes = draft.format.rollover
			? Math.floor(total / 60) % 60
			: Math.floor(total / 60);
		return `${sign}${String(minutes).padStart(2, "0")}${separator}${String(total % 60).padStart(2, "0")}`;
	}
	const hours = Math.floor(total / 3_600);
	return `${sign}${pattern === "h:mm:ss" ? hours : String(hours).padStart(2, "0")}${separator}${String(Math.floor((total % 3_600) / 60)).padStart(2, "0")}${separator}${String(total % 60).padStart(2, "0")}`;
}

function escapeSvg(value: string): string {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&apos;");
}

function describe(slot: TextSlotView): string {
	const kind =
		KINDS.find((candidate) => candidate.value === slot.kind)?.label ??
		slot.kind;
	if (slot.text !== null) return `${kind} · ${slot.text.replace(/\s+/gu, " ")}`;
	if (slot.durationSeconds !== null)
		return `${kind} · ${slot.durationSeconds} s`;
	if (slot.targetUnixMillis !== null)
		return `${kind} · ${new Date(slot.targetUnixMillis).toLocaleString()}`;
	return kind;
}

export function nextFreeAddress(slots: TextSlotView[]): {
	folder: number;
	file: number;
} {
	const taken = new Set(slots.map(key));
	for (let folder = 200; folder <= 249; folder += 1)
		for (let file = 1; file <= 254; file += 1)
			if (!taken.has(`${folder}/${file}`)) return { folder, file };
	return { folder: 200, file: 1 };
}
