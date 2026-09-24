import { describe, expect, it } from "vitest";
import { aVisualizer } from "../../testing/server";
import { visualizerChange, visualizerControls } from "./visualizerPaneControls";

describe("a shown visualizer's pane controls", () => {
	it("offers the audio-or-beat choice by the name the kind gives it", () => {
		const shape = aVisualizer({ typeId: 50, uses: ["smoothing", "on-beat"] });
		const choice = visualizerControls(shape, false).find(
			(control) => control.id === "visualizer-on-beat",
		);
		expect(choice).toMatchObject({
			kind: "choice",
			label: "React to",
			value: "false",
			options: [
				{ value: "false", label: "Audio" },
				{ value: "true", label: "Beat" },
			],
		});

		const stars = aVisualizer({ typeId: 22, uses: ["on-beat"] });
		expect(
			visualizerControls(stars, false).find(
				(control) => control.id === "visualizer-on-beat",
			)?.label,
		).toBe("Spawn stars");
	});

	it("switches the layer's tuning to the beat", () => {
		const shape = aVisualizer({ typeId: 50, uses: ["on-beat"] });
		const edit = visualizerChange(
			"visualizer-on-beat",
			"true",
			shape.parameters,
		);
		expect(edit.visualizerParameters?.onBeat).toBe(true);
	});
});
