// The text a desk can address.
//
// A text source is selected exactly like a clip — by address — so this page's job is the same as
// the visualizers': show which address reaches which words, let an operator write their own, and
// let them put one on a layer without looking the number up first.

import { Button, SelectField } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { OutputView, TextSlotView } from "../../shared/api/generated/media-wire";
import { useLayerControl, useOutputsForControl } from "../../shared/api/layerControl";
import { useText } from "../../shared/api/queries";
import {
	KINDS,
	type TextDraft,
	TextSourceEditor,
	draftOf,
	emptyDraft,
	payloadOf,
} from "./TextSourceEditor";

/// The identity of a slot in the editing state, which is its address.
export function key(slot: TextSlotView): string {
	return `${slot.address.folder}/${slot.address.file}`;
}

/// The key the creation form is open under. Not an address, so it cannot collide with a slot.
const NEW = "new";

export function TextSourcesPage() {
	const text = useText();
	const outputs = useOutputsForControl();
	const control = useLayerControl(outputs.data);
	const [layer, setLayer] = useState(0);
	const editing = useEditing(text.reload);
	const [draft, setDraft] = useState<TextDraft>(emptyDraft());
	const [address, setAddress] = useState({ folder: 200, file: 3 });

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

			<LayerChoice outputs={outputs.data ?? []} layer={layer} onChange={setLayer} />

			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}

			{editing.editing === NEW ? (
				<article className="media-output-card" aria-label="New text source">
					<header>
						<h2>New text source</h2>
					</header>
					<TextSourceEditor
						draft={draft}
						address={address}
						onAddress={setAddress}
						busy={editing.busy}
						submitLabel="Create"
						onChange={setDraft}
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
				</article>
			) : (
				<div className="media-settings-actions">
					<Button onClick={beginNew}>Write a new text source</Button>
				</div>
			)}

			<ResourceState
				resource={text}
				subject="the text sources"
				isEmpty={(data) => data.length === 0}
				empty="No text is assigned to an address yet."
			>
				{(data) => (
					<div className="media-output-grid">
						{data.map((slot) => (
							<TextCard
								key={key(slot)}
								slot={slot}
								outputs={outputs.data ?? []}
								editing={editing.editing === key(slot)}
								busy={editing.busy}
								draft={draft}
								onDraft={setDraft}
								onEdit={() => {
									setDraft(draftOf(slot));
									editing.begin(key(slot));
								}}
								onCancel={editing.cancel}
								onSave={() =>
									void editing.save(() =>
										api.updateText(slot.address.folder, slot.address.file, {
											requestId: requestId(),
											name: draft.name,
											enabled: draft.enabled,
											kind: draft.kind,
											style: draft.style,
											...payloadOf(draft),
										}),
									)
								}
								onDelete={() =>
									void editing.save(() =>
										api.deleteText(slot.address.folder, slot.address.file, {
											requestId: requestId(),
										}),
									)
								}
								onSelect={(output) =>
									void control.update(output, layer, {
										folder: slot.address.folder,
										file: slot.address.file,
									})
								}
							/>
						))}
					</div>
				)}
			</ResourceState>
		</section>
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
			label="Put the chosen text on"
			value={String(layer)}
			options={Array.from({ length: layers }, (_, index) => ({
				value: String(index),
				label: `Layer ${index + 1}`,
			}))}
			onChange={(next) => onChange(Number(next))}
		/>
	);
}

function TextCard({
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
	const address = addressLabel(slot.address.folder, slot.address.file);
	return (
		<article className="media-output-card" aria-label={slot.name}>
			<header>
				<h2>{slot.name}</h2>
				<span className="media-address">{address}</span>
			</header>
			<p>
				{describe(slot)}
				{!slot.enabled && <span className="media-badge is-busy">Parked</span>}
			</p>
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
							{outputs.length === 1 ? "Select" : `Select on ${output.name}`}
						</Button>
					))}
				</div>
			)}
		</article>
	);
}

/// What a slot shows, in one line an operator can read in a list.
function describe(slot: TextSlotView): string {
	const kind = KINDS.find((candidate) => candidate.value === slot.kind)?.label ?? slot.kind;
	if (slot.text !== null) return `${kind} · “${slot.text}”`;
	if (slot.durationSeconds !== null) return `${kind} · ${slot.durationSeconds} s`;
	if (slot.targetUnixMillis !== null) {
		return `${kind} · ${new Date(slot.targetUnixMillis).toLocaleString()}`;
	}
	return kind;
}

/// The first address in the text range nothing answers at, so a new slot starts somewhere usable.
export function nextFreeAddress(slots: TextSlotView[]): { folder: number; file: number } {
	const taken = new Set(slots.map(key));
	for (let folder = 200; folder <= 219; folder += 1) {
		for (let file = 1; file <= 254; file += 1) {
			if (!taken.has(`${folder}/${file}`)) return { folder, file };
		}
	}
	// Every address in the text range is in use, which is 5080 text sources. Land on the first and
	// let the server refuse it rather than silently overwriting one.
	return { folder: 200, file: 1 };
}
