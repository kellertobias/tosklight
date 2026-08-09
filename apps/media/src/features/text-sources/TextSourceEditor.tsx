// Writing one text source.
//
// The kind decides which payload field is shown, so an operator is never asked for a duration on a
// clock or for words on a countdown. Appearance travels with the content because it is the same
// edit: a slot is what it says and how it looks.

import { Button, CheckboxField, NumberField, SelectField, TextField } from "@tosklight/ui/controls";
import { useState } from "react";
import type { TextSlotView, TextStyleView } from "../../shared/api/generated/media-wire";

/// The kinds, in the order an operator meets them.
export const KINDS = [
	{ value: "static", label: "Fixed words" },
	{ value: "clock", label: "Time of day" },
	{ value: "countdown-duration", label: "Countdown of a length" },
	{ value: "countdown-target", label: "Countdown to a moment" },
] as const;

const ALIGNMENTS = [
	{ value: "left", label: "Left" },
	{ value: "center", label: "Centre" },
	{ value: "right", label: "Right" },
] as const;

/// Everything an edit or a creation carries. One shape, because both write the same slot.
export interface TextDraft {
	name: string;
	enabled: boolean;
	kind: string;
	text: string;
	durationSeconds: number;
	/** A local datetime as an `<input type="datetime-local">` gives it. */
	target: string;
	style: TextStyleView;
}

export function draftOf(slot: TextSlotView): TextDraft {
	return {
		name: slot.name,
		enabled: slot.enabled,
		kind: slot.kind,
		text: slot.text ?? "",
		durationSeconds: slot.durationSeconds ?? 600,
		target: slot.targetUnixMillis === null ? "" : localMoment(slot.targetUnixMillis),
		style: slot.style,
	};
}

export function emptyDraft(): TextDraft {
	return {
		name: "",
		enabled: true,
		kind: "static",
		text: "",
		durationSeconds: 600,
		target: "",
		style: {
			family: "sans-serif",
			size: 0.2,
			bold: false,
			italic: false,
			alignment: "center",
			red: 1,
			green: 1,
			blue: 1,
		},
	};
}

/// The payload fields the server wants for the kind this draft is on, and nothing else.
export function payloadOf(draft: TextDraft): {
	text?: string;
	durationSeconds?: number;
	targetUnixMillis?: number;
} {
	switch (draft.kind) {
		case "static":
			return { text: draft.text };
		case "countdown-duration":
			return { durationSeconds: draft.durationSeconds };
		case "countdown-target":
			return { targetUnixMillis: draft.target ? Date.parse(draft.target) : Date.now() };
		default:
			return {};
	}
}

export interface TextSourceEditorProps {
	draft: TextDraft;
	busy: boolean;
	/** Shown on a new slot only; an existing one is addressed by where it already is. */
	address?: { folder: number; file: number };
	onAddress?: (address: { folder: number; file: number }) => void;
	onChange: (draft: TextDraft) => void;
	onSave: () => void;
	onCancel: () => void;
	submitLabel: string;
}

export function TextSourceEditor({
	draft,
	busy,
	address,
	onAddress,
	onChange,
	onSave,
	onCancel,
	submitLabel,
}: TextSourceEditorProps) {
	const set = <K extends keyof TextDraft>(field: K, value: TextDraft[K]) =>
		onChange({ ...draft, [field]: value });
	return (
		<form
			className="media-text-editor"
			onSubmit={(event) => {
				event.preventDefault();
				onSave();
			}}
		>
			{address && onAddress && (
				<div className="media-text-address">
					<NumberField
						label="Bank"
						description="200 to 219."
						min={200}
						max={219}
						value={String(address.folder)}
						onChange={(event) =>
							onAddress({ ...address, folder: Number(event.target.value) })
						}
					/>
					<NumberField
						label="File"
						description="1 to 254. File 0 and file 255 are blank in every bank."
						min={1}
						max={254}
						value={String(address.file)}
						onChange={(event) =>
							onAddress({ ...address, file: Number(event.target.value) })
						}
					/>
				</div>
			)}

			<TextField
				label="Name"
				value={draft.name}
				onChange={(event) => set("name", event.target.value)}
			/>
			<SelectField
				label="Shows"
				value={draft.kind}
				options={KINDS.map((kind) => ({ value: kind.value, label: kind.label }))}
				onChange={(next) => set("kind", next)}
			/>

			{draft.kind === "static" && (
				<TextField
					label="Words"
					value={draft.text}
					onChange={(event) => set("text", event.target.value)}
				/>
			)}
			{draft.kind === "countdown-duration" && (
				<NumberField
					label="Length in seconds"
					description="Starts when the layer showing it becomes visible."
					min={0}
					step={1}
					value={String(draft.durationSeconds)}
					onChange={(event) => set("durationSeconds", Number(event.target.value))}
				/>
			)}
			{draft.kind === "countdown-target" && (
				<label className="media-text-moment">
					Counts down to
					<input
						type="datetime-local"
						value={draft.target}
						onChange={(event) => set("target", event.target.value)}
					/>
				</label>
			)}

			<CheckboxField
				label="Available to a desk"
				stateLabel="Draw when a layer selects it"
				description="A parked source keeps its words and draws nothing."
				checked={draft.enabled}
				onChange={(event) => set("enabled", event.target.checked)}
			/>

			<Appearance draft={draft} onChange={onChange} />

			<div className="media-settings-actions">
				<Button type="submit" variant="primary" loading={busy}>
					{submitLabel}
				</Button>
				<Button onClick={onCancel}>Cancel</Button>
			</div>
		</form>
	);
}

/// How the words look. Its own component because content and appearance are edited together but
/// read separately, and because one form of twenty fields is not something anybody can follow.
function Appearance({
	draft,
	onChange,
}: {
	draft: TextDraft;
	onChange: (draft: TextDraft) => void;
}) {
	const setStyle = <K extends keyof TextStyleView>(field: K, value: TextStyleView[K]) =>
		onChange({ ...draft, style: { ...draft.style, [field]: value } });

	return (
		<fieldset>
			<legend>Appearance</legend>
			<TextField
				label="Font"
				description="A family this machine has. An absent one falls back, and the server says it substituted."
				value={draft.style.family}
				onChange={(event) => setStyle("family", event.target.value)}
			/>
			<NumberField
				label="Height"
				description="A fraction of the output's height, so a look survives a change of resolution."
				min={0.01}
				max={2}
				step={0.01}
				value={String(draft.style.size)}
				onChange={(event) => setStyle("size", Number(event.target.value))}
			/>
			<SelectField
				label="Alignment"
				value={draft.style.alignment}
				options={ALIGNMENTS.map((alignment) => ({
					value: alignment.value,
					label: alignment.label,
				}))}
				onChange={(next) => setStyle("alignment", next)}
			/>
			<label className="media-text-colour">
				Colour
				<input
					type="color"
					value={toHex(draft.style.red, draft.style.green, draft.style.blue)}
					onChange={(event) => {
						const [red, green, blue] = fromHex(event.target.value);
						onChange({
							...draft,
							style: { ...draft.style, red, green, blue },
						});
					}}
				/>
			</label>
			<CheckboxField
				label="Bold"
				stateLabel="Draw in a heavier weight"
				checked={draft.style.bold}
				onChange={(event) => setStyle("bold", event.target.checked)}
			/>
			<CheckboxField
				label="Italic"
				stateLabel="Draw slanted"
				checked={draft.style.italic}
				onChange={(event) => setStyle("italic", event.target.checked)}
			/>
		</fieldset>
	);
}

/** A Unix millisecond stamp as a local `datetime-local` value. */
function localMoment(unixMillis: number): string {
	const at = new Date(unixMillis);
	const pad = (value: number) => String(value).padStart(2, "0");
	return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}T${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

/** The server keeps colour as three 0–1 channels; a colour input speaks hex. */
function toHex(red: number, green: number, blue: number): string {
	const channel = (value: number) =>
		Math.round(Math.min(Math.max(value, 0), 1) * 255)
			.toString(16)
			.padStart(2, "0");
	return `#${channel(red)}${channel(green)}${channel(blue)}`;
}

function fromHex(hex: string): [number, number, number] {
	const value = Number.parseInt(hex.replace("#", ""), 16);
	return [((value >> 16) & 0xff) / 255, ((value >> 8) & 0xff) / 255, (value & 0xff) / 255];
}
