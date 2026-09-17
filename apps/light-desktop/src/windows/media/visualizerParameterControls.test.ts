import { describe, expect, it } from "vitest";
import type { NativeMediaVisualizerChannel } from "../../api/client/mediaOutput";
import {
	type BuildMediaPaneModelInput,
	buildMediaPaneModel,
} from "./buildMediaPaneModel";
import { visualizerChannelValue } from "./specializedMediaControls";
import { EMPTY_MEDIA_INSPECTION } from "./useMediaPaneData";

const VISUALIZER_ATTRIBUTES = [1, 2, 3, 4].map(
	(parameter) => `media.visualizer.parameter.${parameter}`,
);

const server = {
	fixture_id: "server-1",
	name: "ToskLight Pixel",
	endpoint: null,
	layers: [
		{
			fixture_id: "layer-1",
			head_index: 1,
			attributes: [...VISUALIZER_ATTRIBUTES, "media.effect.bank.1.select"],
		},
	],
	master_attributes: ["intensity"],
	status: { online: false, last_success: null, last_error: null },
};

/** Waveform Oscilloscope's first three channels, as the Media Server reports them. */
const WAVEFORM: NativeMediaVisualizerChannel[] = [
	{
		index: 0,
		parameter: "size",
		label: "Size",
		minimum: 0.005,
		maximum: 0.1,
		step: 0.001,
		defaultValue: 0.05,
	},
	{
		index: 1,
		parameter: "thickness",
		label: "Thickness",
		minimum: 0.0005,
		maximum: 0.5,
		step: 0.001,
		defaultValue: 0.01,
	},
	{
		index: 2,
		parameter: "filled",
		label: "Filled",
		minimum: 0,
		maximum: 1,
		step: 1,
		defaultValue: 0,
	},
];

function visualizerControls(
	patch: Partial<BuildMediaPaneModelInput>,
	raw: Record<string, number> = {},
) {
	const model = buildMediaPaneModel({
		inspection: EMPTY_MEDIA_INSPECTION,
		inspectionError: null,
		servers: [server],
		selectedServer: server,
		selectedServerId: server.fixture_id,
		selectedLayerId: "layer-1",
		browserMode: "media",
		selectedControlSectionId: "visualizer",
		mainSectionId: "content",
		rightPaneVisible: false,
		draftFolderId: "250",
		draftFileId: null,
		thumbnailUrls: {},
		previewUrls: {},
		liveProgrammer: Object.entries(raw).map(
			([attribute, value], programmerOrder) => ({
				fixtureId: "layer-1",
				attribute,
				value: { kind: "normalized" as const, value: value / 255 },
				programmerOrder,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			}),
		),
		...patch,
	});
	return (
		model.controlSections.find((section) => section.id === "visualizer")
			?.controls ?? []
	);
}

describe("desk Visualizer Parameter channels", () => {
	it("names, ranges, and defaults each channel as the shown visualizer defines it", () => {
		const controls = visualizerControls(
			{ visualizerChannels: WAVEFORM },
			{
				"media.visualizer.parameter.1": 255,
				"media.visualizer.parameter.3": 200,
			},
		);
		expect(controls).toEqual([
			expect.objectContaining({
				id: "media.visualizer.parameter.1",
				label: "Size",
				value: 255,
				display: "0.1",
			}),
			expect.objectContaining({
				id: "media.visualizer.parameter.2",
				label: "Thickness",
				value: 0,
				display: "Default · 0.01",
			}),
			expect.objectContaining({
				id: "media.visualizer.parameter.3",
				label: "Filled",
				display: "On",
			}),
			expect.objectContaining({
				id: "media.visualizer.parameter.4",
				label: "Parameter 4",
				display: "Unused",
				description: "The shown visualizer does not use this channel.",
			}),
		]);
		// The channels stay programmable: a cue may be written before the visualizer is shown.
		expect(controls.every((control) => !control.disabled)).toBe(true);
	});

	it("marks every channel unused for ordinary media and stays generic when unknown", () => {
		expect(
			visualizerControls({ visualizerChannels: [] }).map(
				(control) => control.description,
			),
		).toEqual(Array(4).fill("Only a shown visualizer uses this channel."));
		expect(
			visualizerControls({}, { "media.visualizer.parameter.2": 9 }).map(
				(control) => ({
					label: control.label,
					display: control.kind === "value" ? control.display : undefined,
				}),
			),
		).toEqual([
			{ label: "Parameter 1", display: "Default" },
			{ label: "Parameter 2", display: "9" },
			{ label: "Parameter 3", display: "Default" },
			{ label: "Parameter 4", display: "Default" },
		]);
	});

	it("decodes a byte exactly as the Media Server does", () => {
		const count = {
			parameter: "count",
			minimum: 1,
			maximum: 512,
		};
		expect(visualizerChannelValue(count, 1)).toBe(1);
		expect(visualizerChannelValue(count, 255)).toBe(512);
		expect(visualizerChannelValue(count, 128)).toBe(257);
		expect(visualizerChannelValue({ ...count, parameter: "mode" }, 4)).toBe(3);
		expect(visualizerChannelValue(WAVEFORM[0], 128)).toBeCloseTo(
			0.005 + (127 / 254) * 0.095,
		);
	});
});
