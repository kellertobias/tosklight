import { Button, SelectField } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import {
	MediaListDetail,
	MediaPreview,
} from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	OutputView,
	TextSlotView,
} from "../../shared/api/generated/media-wire";
import {
	useLayerControl,
	useOutputsForControl,
} from "../../shared/api/layerControl";
import { useText } from "../../shared/api/queries";
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

export function TextSourcesPage() {
	const text = useText();
	const outputs = useOutputsForControl();
	const control = useLayerControl(outputs.data);
	const [layer, setLayer] = useState(0);
	const editing = useEditing(text.reload);
	const [draft, setDraft] = useState<TextDraft>(emptyDraft());
	const [address, setAddress] = useState({ folder: 200, file: 3 });
	const [selectedKey, setSelectedKey] = useState("");

	useEffect(() => {
		if (!selectedKey && text.data?.[0]) setSelectedKey(key(text.data[0]));
	}, [selectedKey, text.data]);

	const beginNew = () => {
		setDraft(emptyDraft());
		setAddress(nextFreeAddress(text.data ?? []));
		editing.begin(NEW);
	};

	return (
		<section className="media-page">
			{control.refusal && (
				<p className="media-state is-error" role="alert">
					{control.refusal.deskOwnsIt
						? "That selection was not applied: a lighting desk is driving this output."
						: control.refusal.message}{" "}
					<Button size="compact" onClick={control.dismissRefusal}>
						Dismiss
					</Button>
				</p>
			)}
			<LayerChoice
				outputs={outputs.data ?? []}
				layer={layer}
				onChange={setLayer}
			/>
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}
			<div className="media-settings-actions">
				<Button onClick={beginNew}>Write a new text source</Button>
			</div>
			<ResourceState
				resource={text}
				subject="the text sources"
				isEmpty={(data) => data.length === 0}
				empty="No text is assigned to an address yet."
			>
				{(data) => {
					const selected =
						data.find((slot) => key(slot) === selectedKey) ?? data[0];
					return (
						<MediaListDetail
							label="Text sources"
							items={data.map((slot) => ({
								id: key(slot),
								title: slot.name,
								detail: describe(slot),
								meta: addressLabel(slot.address.folder, slot.address.file),
							}))}
							selectedId={editing.editing === NEW ? "" : key(selected)}
							onSelect={(next) => {
								editing.cancel();
								setSelectedKey(next);
							}}
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
								) : (
									<TextDetail
										slot={selected}
										outputs={outputs.data ?? []}
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
										onSelect={(output) =>
											void control.update(output, layer, {
												folder: selected.address.folder,
												file: selected.address.file,
											})
										}
									/>
								)
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
		<>
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
		</>
	);
}

function TextDetail({
	slot,
	outputs,
	editing,
	busy,
	draft,
	onDraft,
	onEdit,
	onCancel,
	onSave,
	onDelete,
	onSelect,
}: {
	slot: TextSlotView;
	outputs: OutputView[];
	editing: boolean;
	busy: boolean;
	draft: TextDraft;
	onDraft: (draft: TextDraft) => void;
	onEdit: () => void;
	onCancel: () => void;
	onSave: () => void;
	onDelete: () => void;
	onSelect: (output: OutputView) => void;
}) {
	return (
		<>
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
					{outputs.map((output) => (
						<Button
							key={output.id}
							disabled={output.dmxActive}
							onClick={() => onSelect(output)}
						>
							{outputs.length === 1
								? "Select on output"
								: `Select on ${output.name}`}
						</Button>
					))}
				</div>
			)}
		</>
	);
}

function LayerChoice({
	outputs,
	layer,
	onChange,
}: {
	outputs: OutputView[];
	layer: number;
	onChange: (layer: number) => void;
}) {
	const layers = Math.max(1, ...outputs.map((output) => output.layerCount));
	return (
		<SelectField
			label="Target layer for manual selection"
			value={String(layer)}
			options={Array.from({ length: layers }, (_, index) => ({
				value: String(index),
				label: `Layer ${index + 1}`,
			}))}
			onChange={(next) => onChange(Number(next))}
		/>
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
	for (let folder = 200; folder <= 219; folder += 1)
		for (let file = 1; file <= 254; file += 1)
			if (!taken.has(`${folder}/${file}`)) return { folder, file };
	return { folder: 200, file: 1 };
}
