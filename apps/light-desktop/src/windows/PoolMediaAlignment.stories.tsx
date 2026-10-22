import type { Meta, StoryObj } from "@storybook/react-vite";
import { PoolCard, type PoolCardViewModel } from "@tosklight/ui/pools";
import { PresetPreviewGlyph } from "../features/presetPreview/PresetPreviewGlyph";

const meta: Meta = {
	title: "ToskLight/Windows/Pool media alignment",
	parameters: { layout: "fullscreen" },
};
export default meta;
type Story = StoryObj;

const artwork: Array<
	Pick<PoolCardViewModel, "primary" | "icon" | "image" | "preview">
> = [
	{ primary: "Fixture", icon: "icon:fixture-type/wash-moving-light" },
	{ primary: "Beam", icon: "icon:gobo/ring" },
	{
		primary: "Image",
		image: {
			src: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 80 80'%3E%3Crect x='10' y='10' width='60' height='60' fill='cyan'/%3E%3C/svg%3E",
			alt: "Stored cue preview",
		},
	},
	{
		primary: "Position",
		preview: (
			<PresetPreviewGlyph
				preview={{
					kind: "position",
					space: "angles",
					distinct: 1,
					dots: [{ x: 0.5, y: 0.5 }],
				}}
			/>
		),
	},
	{ primary: "Symbol", icon: "◇" },
];

/** The same artwork must fit automatic square and configured rectangular pools. */
export const SquareAndConfiguredTiles: Story = {
	render: () => (
		<div style={{ display: "grid", gap: 16 }}>
			{[
				{ label: "square", width: 96, height: 96 },
				{ label: "rectangular", width: 160, height: 68 },
			].map(({ label, width, height }) =>
				[false, true].map((selected) => (
					<div
						key={`${label}-${selected}`}
						data-tile-shape={label}
						data-selected={String(selected)}
						style={{ display: "flex", gap: 8 }}
					>
						{artwork.map((model, index) => (
							<PoolCard
								key={String(model.primary)}
								style={{ width, height, aspectRatio: "auto" }}
								model={{
									...model,
									number: index + 1,
									secondary: "8",
									kind: "preset",
									states: selected ? ["selected"] : [],
								}}
							/>
						))}
					</div>
				)),
			)}
		</div>
	),
};
