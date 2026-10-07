import type { PresetPreview } from "./presetPreview";

function plural(count: number, one: string, many: string) {
	return `${count} ${count === 1 ? one : many}`;
}

export function presetPreviewLabel(preview: PresetPreview) {
	if (preview.kind === "color") {
		const shown = preview.colors.length;
		const colors = plural(preview.distinct, "colour", "colours");
		return shown < preview.distinct
			? `Color preview, ${shown} of ${colors}`
			: `Color preview, ${colors}`;
	}
	const space = preview.space === "target" ? "Target" : "Pan/Tilt";
	const aims = plural(preview.distinct, "aim", "aims");
	return preview.dots.length < preview.distinct
		? `${space} position preview, ${preview.dots.length} of ${aims}`
		: `${space} position preview, ${aims}`;
}

/** Pool-tile artwork generated from a preset's stored Color or Position intention. */
export function PresetPreviewGlyph({ preview }: { preview: PresetPreview }) {
	const label = presetPreviewLabel(preview);
	if (preview.kind === "color")
		return (
			<span
				className="preset-preview preset-preview-color"
				role="img"
				aria-label={label}
				data-preset-preview="color"
				data-preview-colors={preview.colors.map((color) => color.hex ?? "unknown").join(" ")}
			>
				{preview.colors.map((color, index) => (
					<span
						// Colours are distinct, but an unknown appearance has no hex to key on.
						key={`${color.hex ?? "unknown"}-${index}`}
						className={`preset-preview-segment${color.hex === null ? " unknown" : ""}${color.uv ? " uv" : ""}`}
						style={color.hex ? { background: color.hex } : undefined}
					/>
				))}
			</span>
		);
	return (
		<svg
			className="preset-preview preset-preview-position"
			viewBox="0 0 100 100"
			role="img"
			aria-label={label}
			data-preset-preview="position"
			data-preview-space={preview.space}
			data-preview-dots={preview.dots.length}
		>
			<rect className="preset-preview-frame" x="1" y="1" width="98" height="98" rx="8" />
			<path className="preset-preview-axis" d="M50 8V92M8 50H92" />
			{preview.dots.map((dot) => (
				<circle
					key={`${dot.x}:${dot.y}`}
					className="preset-preview-dot"
					cx={(dot.x * 100).toFixed(2)}
					cy={((1 - dot.y) * 100).toFixed(2)}
					r="7"
				/>
			))}
		</svg>
	);
}
