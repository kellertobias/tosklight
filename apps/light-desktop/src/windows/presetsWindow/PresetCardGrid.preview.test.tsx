import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StoredPreset } from "../../api/types";
import { srgbToXyz } from "../../features/presetPreview/colorDisplay";
import { defaultPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import type { PresetCard } from "../../features/presetRecording/presetCards";
import { resolvePresetCards } from "../../features/presetRecording/presetCards";
import type { PresetFamily } from "../../presetFamilies";
import {
	PresetCardGrid,
	type PresetCustomization,
	PresetCustomizationDialog,
} from "./PresetsWindowView";

afterEach(cleanup);

function semantic(rgb: [number, number, number]) {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: srgbToXyz(...rgb),
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: 0,
				white_target: { kelvin: 6504, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	};
}

function angles(pan: number, tilt: number) {
	return {
		kind: "position",
		value: {
			kind: "angles",
			pan_degrees: { kind: "value", value: pan },
			tilt_degrees: { kind: "value", value: tilt },
		},
	};
}

function card(family: PresetFamily, number: number, body: Partial<StoredPreset>): PresetCard {
	const prefix = family === "Color" ? 2 : 3;
	return {
		id: `${prefix}.${number}`,
		body: { name: `${family} ${number}`, number, family, values: {}, ...body },
	};
}

function Grid({
	family,
	presets,
	customizations = {},
}: {
	family: PresetFamily;
	presets: PresetCard[];
	customizations?: Record<string, PresetCustomization>;
}) {
	return (
		<PresetCardGrid
			cards={resolvePresetCards(presets, family, 8)}
			family={family}
			cardSizing={{ defaultWidth: 120, minimumWidth: 88 }}
			customizations={customizations}
			poolPresentation={defaultPoolPresentation()}
			showId="show"
			surfaceKey="show:show:builtin:preset"
			fallbackMode="type"
			selectionCount={0}
			recallReady
			storeArmed={false}
			updateArmed={false}
			setArmed={false}
			onActivate={() => undefined}
		/>
	);
}

function tile(number: number) {
	const tiles = document.querySelectorAll(".preset-card");
	return tiles[number - 1] as HTMLElement;
}

describe("automatic preset pool previews", () => {
	const rainbow = card("Color", 1, {
		values: {
			a: { color: semantic([0, 0, 1]) },
			b: { color: semantic([1, 0, 0]) },
			c: { color: semantic([1, 0, 0]) },
		},
	});

	it("draws a Color preset's distinct stored colours as a segmented swatch", () => {
		render(<Grid family="Color" presets={[rainbow]} />);
		const swatch = tile(1).querySelector('[data-preset-preview="color"]');
		expect(swatch).toHaveAttribute("data-preview-colors", "#ff0000 #0000ff");
		expect(swatch).toHaveAccessibleName("Color preview, 2 colours");
		expect(swatch?.querySelectorAll(".preset-preview-segment")).toHaveLength(2);
	});

	it("draws a Position preset as dots in a square", () => {
		const fan = card("Position", 1, {
			values: { a: { position: angles(-30, 40) }, b: { position: angles(30, 40) } },
		});
		render(<Grid family="Position" presets={[fan]} />);
		const square = tile(1).querySelector('[data-preset-preview="position"]');
		expect(square).toHaveAttribute("data-preview-dots", "2");
		expect(square?.querySelectorAll("circle")).toHaveLength(2);
	});

	it("follows a stored update without recalling anything", () => {
		function Live() {
			const [presets, setPresets] = useState([rainbow]);
			return (
				<>
					<button
						type="button"
						onClick={() =>
							setPresets([card("Color", 1, { universal_values: { color: semantic([0, 1, 0]) } })])
						}
					>
						update
					</button>
					<Grid family="Color" presets={presets} />
				</>
			);
		}
		render(<Live />);
		fireEvent.click(screen.getByRole("button", { name: "update" }));
		expect(tile(1).querySelector('[data-preset-preview="color"]')).toHaveAttribute(
			"data-preview-colors",
			"#00ff00",
		);
	});

	it("lets an explicit icon or colour override the preview and keeps legacy presets plain", () => {
		const legacy = card("Color", 2, {
			values: { a: { "color.red": { kind: "normalized", value: 1 } } },
			icon: "●",
		});
		const showColored = card("Color", 3, {
			universal_values: { color: semantic([0, 1, 0]) },
			color: "#123456",
		});
		render(
			<Grid
				family="Color"
				presets={[rainbow, legacy, showColored]}
				customizations={{ "2.1": { icon: "★" } }}
			/>,
		);
		expect(tile(1).querySelector(".preset-preview")).toBeNull();
		expect(tile(1).querySelector(".pool-card-media")).toHaveTextContent("★");
		expect(tile(2).querySelector(".preset-preview")).toBeNull();
		expect(tile(2).querySelector(".pool-card-media")).toHaveTextContent("●");
		expect(tile(3).querySelector(".preset-preview")).toBeNull();
		expect(tile(3)).toHaveStyle({ "--pool-card-icon-color": "#123456" });
	});

	it("offers a way back from a chosen icon and colour to the automatic preview", () => {
		const onDraft = vi.fn();
		render(
			<PresetCustomizationDialog
				index={0}
				draft={{ title: "Red", icon: "★", color: "#123456" }}
				onDraft={onDraft}
				onSave={() => undefined}
				onClose={() => undefined}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Automatic icon" }));
		expect(onDraft).toHaveBeenCalledWith({ title: "Red", icon: "", color: undefined });
	});
});
