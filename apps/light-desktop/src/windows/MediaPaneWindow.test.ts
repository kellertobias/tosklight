import { describe, expect, it } from "vitest";
import {
	mediaCapabilitiesForLayer,
	mediaDraftForLayer,
	mediaFileMutations,
	mediaLibraryMutations,
	mediaRightPaneIsVisible,
	reconcileMediaPaneSelection,
} from "./MediaPaneWindow";

describe("Media pane programmer transaction", () => {
	it("reconciles zero, one, and stale multi-server pane selections", () => {
		expect(reconcileMediaPaneSelection([], "missing")).toEqual({
			serverId: "",
			layerId: "master",
		});
		expect(
			reconcileMediaPaneSelection(
				[{ fixture_id: "internal", layers: [{ fixture_id: "audio" }] }],
				"",
			),
		).toEqual({ serverId: "internal", layerId: "audio" });
		expect(
			reconcileMediaPaneSelection(
				[
					{ fixture_id: "video", layers: [{ fixture_id: "video-1" }] },
					{ fixture_id: "backup", layers: [{ fixture_id: "backup-1" }] },
				],
				"removed",
			),
		).toEqual({ serverId: "video", layerId: "video-1" });
	});

	it("opens a new Media pane with its right pane visible", () => {
		expect(mediaRightPaneIsVisible(undefined)).toBe(true);
		expect(mediaRightPaneIsVisible({})).toBe(true);
		expect(mediaRightPaneIsVisible({ rightPaneVisible: false })).toBe(false);
	});

	it("commits folder and file together for the exact logical layer", () => {
		expect(mediaFileMutations("layer-7", 2, 19)).toEqual([
			{
				action: "set_fixture",
				fixtureId: "layer-7",
				attribute: "media.folder",
				value: { kind: "normalized", value: 2 / 255 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
			{
				action: "set_fixture",
				fixtureId: "layer-7",
				attribute: "media.file",
				value: { kind: "normalized", value: 19 / 255 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
		]);
	});

	it("commits offline mask slots through the same ordered pair", () => {
		expect(mediaLibraryMutations("layer-7", "mask", 3, 21)).toEqual([
			expect.objectContaining({ attribute: "media.mask.folder" }),
			expect.objectContaining({ attribute: "media.mask.file" }),
		]);
	});

	it("exposes only the exact layer capabilities advertised by the provider", () => {
		const inspection = {
			library_revision: "citp-revision",
			server: { name: "Server", layer_count: 1 },
			folders: [],
			files: [],
			preview_sources: [],
			layers: [],
			capabilities: {
				provider: "citp_msex",
				native_action: null,
				layers: [
					{
						layer: 7,
						content_library: true,
						mask_library: true,
						secondary_controls: [{ attribute: "media.opacity" }],
					},
				],
			},
		};
		expect(mediaCapabilitiesForLayer(inspection, 7)).toEqual(
			inspection.capabilities.layers[0],
		);
		expect(mediaCapabilitiesForLayer(inspection, 8)).toBeUndefined();
	});

	it("resets a switched logical layer draft to that layer's advertised live pair", () => {
		const inspection = {
			library_revision: "citp-test",
			server: { name: "Server", layer_count: 2 },
			folders: [],
			files: [],
			preview_sources: [],
			layers: [
				{
					layer: 7,
					physical_output: 1,
					folder: 2,
					file: 19,
					name: "Layer 7",
					position_frames: 0,
					length_frames: 100,
					fps: 25,
					flags: 0,
				},
			],
			capabilities: {
				provider: "citp_msex",
				native_action: null,
				layers: [],
			},
		};
		expect(
			mediaDraftForLayer(
				inspection,
				[{ fixture_id: "layer-7", head_index: 7 }],
				"layer-7",
			),
		).toEqual({ folderId: "2", fileId: "19" });
		expect(
			mediaDraftForLayer(
				inspection,
				[{ fixture_id: "layer-7", head_index: 7 }],
				"master",
			),
		).toBeNull();
	});
});
