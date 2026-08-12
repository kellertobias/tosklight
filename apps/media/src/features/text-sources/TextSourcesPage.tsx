import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { MediaPreview } from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { TextSlotView } from "../../shared/api/generated/media-wire";
import { useText } from "../../shared/api/queries";
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
	const editing = useEditing(text.reload);
	const [draft, setDraft] = useState<TextDraft>(emptyDraft());
	const [address, setAddress] = useState({ folder: 200, file: 3 });
	const [selectedKey, setSelectedKey] = useState<string | null>(null);

	useEffect(() => {
		if (selectedKey === null && text.data?.[0])
			setSelectedKey(key(text.data[0]));
	}, [selectedKey, text.data]);

	const beginNew = () => {
		setDraft(emptyDraft());
		setAddress(nextFreeAddress(text.data ?? []));
		editing.begin(NEW);
	};

	return (
		<section className="media-page media-library-page">
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}
			<ResourceState resource={text} subject="the text sources">
				{(data) => {
					const selected = data.find((slot) => key(slot) === selectedKey);
					return (
						<GeneratedLibraryBrowserView
							type="text"
							items={data.map((slot) => ({
								id: key(slot),
								folder: slot.address.folder,
								file: slot.address.file,
								name: slot.name,
								detail: describe(slot),
							}))}
							selectedId={editing.editing === NEW ? "" : (selectedKey ?? "")}
							onSelect={(next) => {
								editing.cancel();
								setSelectedKey(next);
							}}
							onTypeChange={onModeChange}
							headerAction={
								<Button size="compact" onClick={beginNew}>
									New text source
								</Button>
							}
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
										editing={editing.editing === key(selected)}
										busy={editing.busy}
										draft={draft}
										onDraft={setDraft}
										onEdit={() => {
											setDraft(draftOf(selected));
											editing.begin(key(selected));
										}}
										onCancel={editing.cancel}
										onSave={() =>
											void editing.save(() =>
												api.updateText(
													selected.address.folder,
													selected.address.file,
													{
														requestId: requestId(),
														name: draft.name,
														enabled: draft.enabled,
														kind: draft.kind,
														style: draft.style,
														...payloadOf(draft),
													},
												),
											)
										}
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
	editing,
	busy,
	draft,
	onDraft,
	onEdit,
	onCancel,
	onSave,
	onDelete,
}: {
	slot: TextSlotView;
	editing: boolean;
	busy: boolean;
	draft: TextDraft;
	onDraft: (draft: TextDraft) => void;
	onEdit: () => void;
	onCancel: () => void;
	onSave: () => void;
	onDelete: () => void;
}) {
	return (
		<div className="media-library-editor media-generated-library-detail">
			<MediaPreview title={slot.name} variant="text">
				<span className="media-text-preview-words">
					{slot.text ?? describe(slot)}
				</span>
			</MediaPreview>
			<header className="media-detail-heading">
				<div>
					<h2>{slot.name}</h2>
					<p>
						{addressLabel(slot.address.folder, slot.address.file)}{" "}
						{!slot.enabled && (
							<span className="media-badge is-busy">Parked</span>
						)}
					</p>
				</div>
			</header>
			{editing ? (
				<TextSourceEditor
					draft={draft}
					busy={busy}
					submitLabel="Save"
					onChange={onDraft}
					onCancel={onCancel}
					onSave={onSave}
				/>
			) : (
				<div className="media-settings-actions">
					<Button onClick={onEdit}>Change</Button>
					<Button onClick={onDelete}>Remove</Button>
				</div>
			)}
		</div>
	);
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
