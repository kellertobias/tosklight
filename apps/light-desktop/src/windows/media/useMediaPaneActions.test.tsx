import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MediaServerFixture } from "../../api/types";
import { useMediaPaneActions } from "./useMediaPaneActions";
import { EMPTY_MEDIA_INSPECTION } from "./useMediaPaneData";

const server: MediaServerFixture = {
	fixture_id: "server-1",
	name: "Server",
	endpoint: { protocol: "citp", ip_address: "127.0.0.1", port: 4809 },
	layers: [{ fixture_id: "layer-1", head_index: 0 }],
	status: { online: false, last_success: null, last_error: "offline" },
};

function setup() {
	const submitLatest = vi.fn().mockResolvedValue(null);
	const submitBarrier = vi.fn().mockResolvedValue(null);
	const applySelection = vi.fn().mockResolvedValue(null);
	const input = {
		servers: [server],
		selectedServer: server,
		selectedLayer: server.layers[0] as
			| MediaServerFixture["layers"][number]
			| undefined,
		selectedFixtureId: "layer-1" as string | undefined,
		selectedLayerId: "layer-1" as string,
		browserMode: "media" as "media" | "mask",
		browserModeByLayer: {} as Record<string, "media" | "mask">,
		sourceFilter: "media" as const,
		selectedControlSectionId: "playback",
		mainSectionId: "content",
		rightPaneVisible: false,
		inspection: EMPTY_MEDIA_INSPECTION,
		draftFolder: 2,
		applySelection,
		selectionActions: null,
		valuesQueue: {
			canWrite: true,
			unavailableReason: null,
			route: "normal",
			submitLatest,
			submitBarrier,
		},
		setSelectedServerId: vi.fn(),
		setSelectedLayerId: vi.fn(),
		setBrowserMode: vi.fn(),
		setBrowserModeByLayer: vi.fn(),
		setSourceFilter: vi.fn(),
		setSelectedControlSectionId: vi.fn(),
		setMainSectionId: vi.fn(),
		setRightPaneVisible: vi.fn(),
		setDraftFolderId: vi.fn(),
		setDraftFileId: vi.fn(),
		setInspectionError: vi.fn(),
		initializeLayer: vi.fn(),
		resetMediaData: vi.fn(),
		onPersist: vi.fn(),
	};
	const hook = renderHook(() => useMediaPaneActions(input as never));
	return {
		...hook,
		submitLatest,
		submitBarrier,
		applySelection,
		input,
	};
}

describe("Media pane offline actions", () => {
	it("programs an unadvertised folder and file while CITP is unavailable", () => {
		const { result, submitBarrier, applySelection } = setup();
		act(() =>
			result.current.onBrowseItem("media", {
				id: "19",
				kind: "file",
				name: "File 19",
			}),
		);
		expect(applySelection).not.toHaveBeenCalled();
		expect(submitBarrier).toHaveBeenCalledWith([
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "media.folder",
			}),
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "media.file",
			}),
		]);
	});

	it("addresses a legacy Audio Player through its own folder and file", () => {
		const { result, submitBarrier, applySelection, input } = setup();
		input.selectedLayer = {
			fixture_id: "layer-1",
			head_index: 0,
			attributes: ["audio.file", "audio.folder", "audio.volume"],
		} as never;
		act(() =>
			result.current.onBrowseItem("media", {
				id: "12",
				kind: "file",
				name: "File 12",
			}),
		);
		expect(applySelection).not.toHaveBeenCalled();
		expect(submitBarrier).toHaveBeenCalledWith([
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "audio.folder",
				value: expect.objectContaining({ value: 2 / 255 }),
			}),
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "audio.file",
				value: expect.objectContaining({ value: 12 / 255 }),
			}),
		]);
	});

	it("programs file zero to clear content while retaining the draft folder", () => {
		const { result, submitBarrier, input } = setup();
		act(() =>
			result.current.onBrowseItem("media", {
				id: "0",
				kind: "file",
				name: "No file selected",
			}),
		);
		expect(input.setDraftFileId).toHaveBeenCalledWith("0");
		expect(submitBarrier).toHaveBeenCalledWith([
			expect.objectContaining({
				attribute: "media.folder",
				value: expect.objectContaining({ value: 2 / 255 }),
			}),
			expect.objectContaining({
				attribute: "media.file",
				value: expect.objectContaining({ value: 0 }),
			}),
		]);
	});

	it("changes source filter locally without programming a fixture", () => {
		const { result, submitBarrier, applySelection, input } = setup();
		act(() => result.current.onSelectSourceFilter("text"));
		expect(input.setSourceFilter).toHaveBeenCalledWith("text");
		expect(input.setDraftFolderId).toHaveBeenCalledWith("200");
		expect(input.setDraftFileId).toHaveBeenCalledWith(null);
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({ sourceFilter: "text" }),
		);
		expect(submitBarrier).not.toHaveBeenCalled();
		expect(applySelection).not.toHaveBeenCalled();
	});

	it("switches the complete context to the selected server's first logical head", () => {
		const { result, input } = setup();
		const nextServer = {
			...server,
			fixture_id: "server-2",
			layers: [{ fixture_id: "server-2-layer-1", head_index: 0 }],
		};
		input.servers.push(nextServer);
		act(() => result.current.onSelectServer("server-2"));
		expect(input.setSelectedServerId).toHaveBeenCalledWith("server-2");
		expect(input.setSelectedLayerId).toHaveBeenCalledWith("server-2-layer-1");
		expect(input.resetMediaData).toHaveBeenCalledOnce();
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				serverId: "server-2",
				layerId: "server-2-layer-1",
			}),
		);
	});

	it("selects Mask automatically and targets the parent fixture for Master", () => {
		const { result, input } = setup();
		act(() => result.current.onSelectLayer("master"));

		expect(input.setSelectedLayerId).toHaveBeenCalledWith("master");
		expect(input.setBrowserMode).toHaveBeenCalledWith("mask");
		expect(input.setMainSectionId).toHaveBeenCalledWith("mask");
		expect(input.initializeLayer).toHaveBeenCalledWith("master");
		expect(input.setBrowserModeByLayer).toHaveBeenCalledWith({
			"layer-1": "media",
		});
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				layerId: "master",
				browserMode: "mask",
				mainSectionId: "mask",
			}),
		);
	});

	it("restores each layer's remembered Content or Mask browser", () => {
		const { result, input } = setup();
		input.selectedLayerId = "master";
		input.browserMode = "mask";
		input.browserModeByLayer = { "layer-1": "media" };

		act(() => result.current.onSelectLayer("layer-1"));

		expect(input.setBrowserMode).toHaveBeenCalledWith("media");
		expect(input.setMainSectionId).toHaveBeenCalledWith("content");
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				layerId: "layer-1",
				browserMode: "media",
				mainSectionId: "content",
			}),
		);
	});

	it("updates only the selected layer's remembered browser mode", () => {
		const { result, input } = setup();
		input.browserModeByLayer = { "layer-2": "media" };

		act(() => result.current.onSelectBrowserMode("mask"));

		expect(input.setBrowserModeByLayer).toHaveBeenCalledWith({
			"layer-1": "mask",
			"layer-2": "media",
		});
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				browserModeByLayer: {
					"layer-1": "mask",
					"layer-2": "media",
				},
			}),
		);
	});

	it("programs Master controls on the parent media-server fixture", () => {
		const { result, submitLatest, input } = setup();
		input.selectedLayer = undefined;
		input.selectedFixtureId = "server-1";
		input.selectedLayerId = "master";
		act(() => result.current.onChangeControl("intensity", 75));

		expect(submitLatest).toHaveBeenCalledWith("fixture:server-1:intensity", [
			expect.objectContaining({
				fixtureId: "server-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.75 },
			}),
		]);
	});

	it("programs Master geometry and shapers through their centred physical ranges", () => {
		const { result, submitLatest, input } = setup();
		input.selectedLayer = undefined;
		input.selectedFixtureId = "server-1";
		input.selectedLayerId = "master";

		act(() => result.current.onChangeControl("media.scale.x", 4));
		expect(submitLatest).toHaveBeenLastCalledWith(
			"fixture:server-1:media.scale.x",
			[expect.objectContaining({ value: { kind: "normalized", value: 1 } })],
		);

		act(() => result.current.onChangeControl("shaper.blade.1.angle", 0));
		expect(submitLatest).toHaveBeenLastCalledWith(
			"fixture:server-1:shaper.blade.1.angle",
			[expect.objectContaining({ value: { kind: "normalized", value: 0.5 } })],
		);
	});

	it("programs a Master mask file without exposing content selection", () => {
		const { result, submitBarrier, input } = setup();
		input.selectedLayer = undefined;
		input.selectedFixtureId = "server-1";
		input.selectedLayerId = "master";
		input.browserMode = "mask";
		input.draftFolder = 1;
		act(() =>
			result.current.onBrowseItem("mask", {
				id: "17",
				kind: "file",
				name: "Master mask",
			}),
		);

		expect(submitBarrier).toHaveBeenCalledWith([
			expect.objectContaining({
				fixtureId: "server-1",
				attribute: "media.mask.file",
				value: { kind: "normalized", value: 17 / 255 },
			}),
		]);
	});

	it("programs discrete control choices through the same normalized Programmer path", () => {
		const { result, submitLatest } = setup();
		act(() => result.current.onChangeControl("media.play_mode", "216"));
		expect(submitLatest).toHaveBeenCalledWith(
			"fixture:layer-1:media.play_mode",
			[
				expect.objectContaining({
					fixtureId: "layer-1",
					attribute: "media.play_mode",
					value: { kind: "normalized", value: 216 / 255 },
				}),
			],
		);
	});

	it("programs percentage controls without treating the percent as a DMX byte", () => {
		const { result, submitLatest } = setup();
		act(() => result.current.onChangeControl("intensity", 42));
		expect(submitLatest).toHaveBeenCalledWith("fixture:layer-1:intensity", [
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.42 },
			}),
		]);
	});

	it("programs centered physical frame values through their neutral DMX midpoint", () => {
		const { result, submitLatest } = setup();
		act(() => result.current.onChangeControl("media.scale.x", 1));
		act(() => result.current.onChangeControl("media.position.x", 0));
		act(() => result.current.onChangeControl("position.rotation", 0));
		expect(submitLatest).toHaveBeenNthCalledWith(
			1,
			"fixture:layer-1:media.scale.x",
			[expect.objectContaining({ value: { kind: "normalized", value: 0.5 } })],
		);
		expect(submitLatest).toHaveBeenNthCalledWith(
			2,
			"fixture:layer-1:media.position.x",
			[expect.objectContaining({ value: { kind: "normalized", value: 0.5 } })],
		);
		expect(submitLatest).toHaveBeenNthCalledWith(
			3,
			"fixture:layer-1:position.rotation",
			[expect.objectContaining({ value: { kind: "normalized", value: 0.5 } })],
		);
	});

	it("programs the RGB picker as one grouped subtractive fixture mutation", () => {
		const { result, submitLatest } = setup();
		act(() => result.current.onChangeControl("color.tint", "#ff8000"));
		expect(submitLatest).toHaveBeenCalledWith(expect.any(String), [
			expect.objectContaining({
				attribute: "color.red",
				value: { kind: "normalized", value: 1 },
			}),
			expect.objectContaining({
				attribute: "color.green",
				value: { kind: "normalized", value: 128 / 255 },
			}),
			expect.objectContaining({
				attribute: "color.blue",
				value: { kind: "normalized", value: 0 },
			}),
		]);
	});

	it("resets one control by releasing its Programmer override", () => {
		const { result, submitBarrier } = setup();
		act(() => result.current.onResetControl("media.scale.x"));
		expect(submitBarrier).toHaveBeenCalledWith([
			expect.objectContaining({
				fixtureId: "layer-1",
				attribute: "media.scale.x",
			}),
		]);
	});

	it("resets the RGB picker by releasing all three RGB overrides together", () => {
		const { result, submitBarrier } = setup();
		act(() => result.current.onResetControl("color.tint"));
		expect(submitBarrier).toHaveBeenCalledWith([
			{
				action: "release_fixture",
				fixtureId: "layer-1",
				attribute: "color.red",
			},
			{
				action: "release_fixture",
				fixtureId: "layer-1",
				attribute: "color.green",
			},
			{
				action: "release_fixture",
				fixtureId: "layer-1",
				attribute: "color.blue",
			},
		]);
	});

	it("keeps the chosen control column selected across the next render", () => {
		const { result, input } = setup();
		act(() => result.current.onSelectControlSection("frame"));
		expect(input.setSelectedControlSectionId).toHaveBeenCalledWith("frame");
		expect(input.setMainSectionId).toHaveBeenCalledWith("frame");
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				controlSectionId: "frame",
				mainSectionId: "frame",
			}),
		);
	});
});
