import { Button, ColorPickerField } from "@tosklight/ui/controls";
import { LargeTextField, TextField } from "@tosklight/ui/forms";
import type { SliceGridTypographyView } from "../../shared/api/generated/media-wire";

export function SliceGridTypographyEditor({
	value,
	onChange,
	fontStatus,
}: {
	value: SliceGridTypographyView;
	onChange(value: SliceGridTypographyView): void;
	fontStatus?: string;
}) {
	return (
		<section aria-label="Grid typography">
			<TextField
				label="Font family"
				value={value.font}
				maxLength={128}
				onChange={(event) => onChange({ ...value, font: event.target.value })}
			/>
			<p role="status">
				{fontStatus ??
					"The Media server resolves this font family and uses its system fallback if unavailable."}
			</p>
			<ColorPickerField
				label="Text color"
				value={toHex(value.textColor)}
				onChange={(hex) => onChange({ ...value, textColor: fromHex(hex) })}
			/>
			<ColorPickerField
				label="Background color"
				value={toHex(value.backgroundColor)}
				onChange={(hex) =>
					onChange({ ...value, backgroundColor: fromHex(hex) })
				}
			/>
			<p>
				Up to 8 text entries; each accepts 512 characters and 16 lines. Text
				always fits its cell. Entries cycle across text cells on each beat.
			</p>
			{value.texts.map((text, index) => (
				<div key={index}>
					<LargeTextField
						label={`Grid text ${index + 1}`}
						value={text}
						maxLength={512}
						rows={3}
						onChange={(event) =>
							onChange({
								...value,
								texts: value.texts.map((entry, at) =>
									at === index ? event.target.value : entry,
								),
							})
						}
					/>
					<Button
						disabled={value.texts.length <= 1}
						onClick={() =>
							onChange({
								...value,
								texts: value.texts.filter((_, at) => at !== index),
							})
						}
					>
						Remove text {index + 1}
					</Button>
				</div>
			))}
			<Button
				disabled={value.texts.length >= 8}
				onClick={() => onChange({ ...value, texts: [...value.texts, ""] })}
			>
				Add text
			</Button>
		</section>
	);
}
function toHex(rgb: number[]): string {
	return `#${rgb.map((value) => Math.round(value).toString(16).padStart(2, "0")).join("")}`;
}
function fromHex(hex: string): [number, number, number] {
	return [
		Number.parseInt(hex.slice(1, 3), 16),
		Number.parseInt(hex.slice(3, 5), 16),
		Number.parseInt(hex.slice(5, 7), 16),
	];
}
