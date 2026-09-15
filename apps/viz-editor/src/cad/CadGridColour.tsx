/**
 * The grid colour in the CAD settings: typed as a hex value, picked from the system colour panel, or
 * taken from a few pale presets.
 *
 * It is a setting of this computer's Architect, not of the show: every show opened here draws its
 * grid in it, and a show carried to another machine takes that machine's grid colour.
 */
import { Input } from "@tosklight/ui";
import { type KeyboardEvent, useEffect, useState } from "react";

export const GRID_COLOUR_PRESETS = [
	{ label: "Pale grey", value: "#c9d1d9" },
	{ label: "Mid grey", value: "#8b949e" },
	{ label: "Blue", value: "#58a6ff" },
	{ label: "Green", value: "#56d364" },
	{ label: "Amber", value: "#e3b341" },
] as const;

/** `#RRGGBB` from what was typed, with or without its hash and in either case; null when not a colour. */
export function parseHexColour(text: string): string | null {
	const match = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/iu.exec(text.trim());
	if (!match) return null;
	const digits =
		match[1].length === 3
			? [...match[1]].map((digit) => digit + digit).join("")
			: match[1];
	return `#${digits.toLowerCase()}`;
}

export function CadGridColour({
	value,
	onChange,
}: {
	value: string;
	onChange(colour: string): void;
}) {
	const [draft, setDraft] = useState(value);
	useEffect(() => setDraft(value), [value]);
	const commit = () => {
		const colour = parseHexColour(draft);
		if (colour && colour !== value) onChange(colour);
		else setDraft(value);
	};
	return (
		<fieldset className="cad-grid-colour">
			<legend>Grid colour</legend>
			<div className="cad-grid-colour-row">
				<input
					type="color"
					aria-label="Pick grid colour"
					value={value}
					onChange={(event) => onChange(event.currentTarget.value)}
				/>
				<Input
					type="text"
					aria-label="Grid colour"
					spellCheck={false}
					value={draft}
					onChange={(event) => setDraft(event.currentTarget.value)}
					onBlur={commit}
					onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
						if (event.key === "Enter") commit();
						if (event.key === "Escape") setDraft(value);
					}}
				/>
			</div>
			<div className="cad-grid-colour-presets" role="group" aria-label="Grid colour presets">
				{GRID_COLOUR_PRESETS.map((preset) => (
					<button
						key={preset.value}
						type="button"
						className="cad-grid-colour-swatch"
						aria-label={preset.label}
						aria-pressed={preset.value === value}
						title={preset.label}
						style={{ background: preset.value }}
						onClick={() => onChange(preset.value)}
					/>
				))}
			</div>
			<small>Saved on this computer for every show, not in the show file.</small>
		</fieldset>
	);
}
