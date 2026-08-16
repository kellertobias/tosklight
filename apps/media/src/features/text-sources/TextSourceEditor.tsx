// Writing one text source.
//
// The kind decides which payload field is shown, so an operator is never asked for a duration on a
// clock or for words on a countdown. Appearance travels with the content because it is the same
// edit: a slot is what it says and how it looks.

import {
	Button,
	CheckboxField,
	ColorPickerField,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui/controls";
import { useState } from "react";
import type {
	TextFormatView,
	TextSlotView,
	TextStyleView,
} from "../../shared/api/generated/media-wire";

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

const CLOCK_FORMATS = ["HH:MM", "HH:MM:SS", "hh:mm", "hh:mm:ss"].map(
	(value) => ({ value, label: value }),
);
const COUNTDOWN_FORMATS = ["ss", "mm", "mm:ss", "hh:mm:ss", "h:mm:ss"].map(
	(value) => ({ value, label: value }),
);
const AFTER_ZERO = [
	{ value: "hold", label: "Hold at zero" },
	{ value: "negative", label: "Continue with minus sign" },
	{ value: "count-up", label: "Count up after zero" },
];

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
	format: TextFormatView;
}

export function draftOf(slot: TextSlotView): TextDraft {
	return {
		name: slot.name,
		enabled: slot.enabled,
		kind: slot.kind,
		text: slot.text ?? "",
		durationSeconds: slot.durationSeconds ?? 600,
		target:
			slot.targetUnixMillis === null ? "" : localMoment(slot.targetUnixMillis),
		style: slot.style,
		format: slot.format,
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
		format: {
			clockPattern: "HH:MM:SS",
			countdownPattern: "hh:mm:ss",
			separator: ":",
			utcOffsetMinutes: 0,
			afterZero: "hold",
			rollover: false,
		},
	};
}

/// The payload fields the server wants for the kind this draft is on, and nothing else.
export function payloadOf(draft: TextDraft): {
	text?: string;
	durationSeconds?: number;
	targetUnixMillis?: number;
	format: TextFormatView;
} {
	const format = draft.format;
	switch (draft.kind) {
		case "static":
			return { text: draft.text, format };
		case "countdown-duration":
			return { durationSeconds: draft.durationSeconds, format };
		case "countdown-target":
			return {
				targetUnixMillis: draft.target ? Date.parse(draft.target) : Date.now(),
				format,
			};
		default:
			return { format };
	}
}

export interface TextSourceEditorProps {
	draft: TextDraft;
	busy: boolean;
	/** Shown on a new slot only; an existing one is addressed by where it already is. */
	address?: { folder: number; file: number };
	onAddress?: (address: { folder: number; file: number }) => void;
	onChange: (draft: TextDraft) => void;
	onSave?: () => void;
	onCancel?: () => void;
	submitLabel?: string;
	part?: "all" | "identity" | "sections";
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
	part = "all",
}: TextSourceEditorProps) {
	const [section, setSection] = useState<"content" | "appearance">("content");
	const set = <K extends keyof TextDraft>(field: K, value: TextDraft[K]) =>
		onChange({ ...draft, [field]: value });
	const showIdentity = part !== "sections";
	const showSections = part !== "identity";
	return (
		<form
			className="media-text-editor"
			onSubmit={(event) => {
				event.preventDefault();
				onSave?.();
			}}
		>
			{address && onAddress && (
				<div className="media-text-address">
					<NumberField
						label="Bank"
						description="200 to 249."
						min={200}
						max={249}
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

			{showIdentity && (
				<div className="media-source-identity-grid">
					<SelectField
						label="Text type"
						ariaLabel="Text type"
						value={draft.kind}
						options={KINDS.map((kind) => ({
							value: kind.value,
							label: kind.label,
						}))}
						onChange={(next) => set("kind", next)}
					/>
					<TextField
						label="Name"
						value={draft.name}
						onChange={(event) => set("name", event.target.value)}
					/>
				</div>
			)}

			{showSections && (
				<div className="media-text-section-heading">
					<div
						className="media-text-section-tabs"
						role="tablist"
						aria-label="Text settings"
					>
						<Button
							role="tab"
							aria-selected={section === "content"}
							variant={section === "content" ? "primary" : "secondary"}
							onClick={() => setSection("content")}
						>
							Content
						</Button>
						<Button
							role="tab"
							aria-selected={section === "appearance"}
							variant={section === "appearance" ? "primary" : "secondary"}
							onClick={() => setSection("appearance")}
						>
							Appearance
						</Button>
					</div>
				</div>
			)}

			{showSections && (
				<div
					className="media-text-section-content"
					role="tabpanel"
					aria-label={section === "content" ? "Content" : "Appearance"}
				>
					{section === "content" && draft.kind === "static" && (
						<TextAreaField
							label="Content"
							description="Line breaks are kept in the rendered source."
							value={draft.text}
							onChange={(event) => set("text", event.target.value)}
						/>
					)}
					{section === "content" && draft.kind === "countdown-duration" && (
						<NumberField
							label="Length in seconds"
							description="Starts when the layer showing it becomes visible."
							min={0}
							step={1}
							value={String(draft.durationSeconds)}
							onChange={(event) =>
								set("durationSeconds", Number(event.target.value))
							}
						/>
					)}
					{section === "content" && draft.kind === "countdown-target" && (
						<label className="media-text-moment">
							Counts down to
							<input
								type="datetime-local"
								value={draft.target}
								onChange={(event) => set("target", event.target.value)}
							/>
						</label>
					)}
					{section === "content" && draft.kind === "clock" && (
						<FormatSection draft={draft} onChange={onChange} clock />
					)}
					{section === "content" && draft.kind.startsWith("countdown") && (
						<FormatSection draft={draft} onChange={onChange} />
					)}
					{section === "appearance" && (
						<Appearance draft={draft} onChange={onChange} />
					)}
				</div>
			)}

			{onSave && submitLabel && (
				<div className="media-settings-actions">
					<Button type="submit" variant="primary" loading={busy}>
						{submitLabel}
					</Button>
					{onCancel && <Button onClick={onCancel}>Cancel</Button>}
				</div>
			)}
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
	const setStyle = <K extends keyof TextStyleView>(
		field: K,
		value: TextStyleView[K],
	) => onChange({ ...draft, style: { ...draft.style, [field]: value } });

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
			<ColorPickerField
				label="Colour"
				value={toHex(draft.style.red, draft.style.green, draft.style.blue)}
				onChange={(value) => {
					const [red, green, blue] = fromHex(value);
					onChange({
						...draft,
						style: { ...draft.style, red, green, blue },
					});
				}}
			/>
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

function FormatSection({
	draft,
	onChange,
	clock = false,
}: {
	draft: TextDraft;
	onChange: (draft: TextDraft) => void;
	clock?: boolean;
}) {
	const set = <K extends keyof TextFormatView>(
		field: K,
		value: TextFormatView[K],
	) => onChange({ ...draft, format: { ...draft.format, [field]: value } });
	return (
		<fieldset>
			<legend>Format</legend>
			<SelectField
				label="Display format"
				ariaLabel="Display format"
				value={
					clock ? draft.format.clockPattern : draft.format.countdownPattern
				}
				options={clock ? CLOCK_FORMATS : COUNTDOWN_FORMATS}
				onChange={(value) =>
					set(clock ? "clockPattern" : "countdownPattern", value)
				}
			/>
			<TextField
				label="Separator"
				description="One to three visible characters."
				value={draft.format.separator}
				onChange={(event) => set("separator", event.target.value.slice(0, 3))}
			/>
			{clock ? (
				<NumberField
					label="UTC offset in minutes"
					min={-840}
					max={840}
					step={15}
					value={String(draft.format.utcOffsetMinutes)}
					onChange={(event) =>
						set("utcOffsetMinutes", Number(event.target.value))
					}
				/>
			) : (
				<>
					<SelectField
						label="After zero"
						ariaLabel="After zero"
						value={draft.format.afterZero}
						options={AFTER_ZERO}
						onChange={(value) => set("afterZero", value)}
					/>
					{draft.format.countdownPattern === "mm:ss" && (
						<CheckboxField
							label="Rollover minutes at 60"
							stateLabel="Use clock-style minute rollover"
							checked={draft.format.rollover}
							onChange={(event) => set("rollover", event.target.checked)}
						/>
					)}
				</>
			)}
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
	return [
		((value >> 16) & 0xff) / 255,
		((value >> 8) & 0xff) / 255,
		(value & 0xff) / 255,
	];
}
