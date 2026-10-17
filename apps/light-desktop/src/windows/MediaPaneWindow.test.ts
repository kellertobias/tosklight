import {
	act,
	cleanup,
	fireEvent,
	render,
	waitFor,
	within,
} from "@testing-library/react";
import { createElement, useCallback } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionFromProfileMode,
} from "../components/setup/fixtureProfileModel";
import type { ProgrammerFixtureValue } from "../features/programmerValues/contracts";
import {
	MediaPaneWindow,
	mediaCapabilitiesForLayer,
	mediaDraftForLayer,
	mediaFileMutations,
	mediaLibraryMutations,
	mediaRightPaneIsVisible,
	reconcileMediaPaneSelection,
} from "./MediaPaneWindow";
import {
	type BuildMediaPaneModelInput,
	buildMediaPaneModel,
} from "./media/buildMediaPaneModel";
import { mediaColourMutation } from "./media/mediaColour";
import { EMPTY_MEDIA_INSPECTION } from "./media/useMediaPaneData";
import { useMediaProgrammerValues } from "./media/useMediaProgrammerValues";

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
					layer: 0,
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

	it("starts an empty advertised layer in folder one so file touches stay valid", () => {
		const inspection = {
			library_revision: "citp-test",
			server: { name: "Server", layer_count: 1 },
			folders: [],
			files: [],
			preview_sources: [],
			layers: [
				{
					layer: 0,
					physical_output: 1,
					folder: 0,
					file: 0,
					name: "Layer 1",
					position_frames: 0,
					length_frames: 0,
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
				[{ fixture_id: "layer-1", head_index: 1 }],
				"layer-1",
			),
		).toEqual({ folderId: "1", fileId: "0" });
	});
});

describe("Media Cue control values", () => {
	it("falls back to accepted Cue values while preserving explicit zero overrides", async () => {
		const { mediaProgrammerValues } = await import(
			"./media/useMediaProgrammerValues"
		);
		const cue = {
			fixtureId: "layer-8",
			attribute: "intensity",
			value: { kind: "normalized" as const, value: 0.32 },
			programmerOrder: 0,
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		};
		const zero = { ...cue, value: { kind: "normalized" as const, value: 0 } };
		expect(mediaProgrammerValues([], undefined, [cue])).toEqual([cue]);
		expect(mediaProgrammerValues([zero], undefined, [cue])).toEqual([zero]);
		expect(mediaProgrammerValues([cue], [zero], [cue])).toEqual([zero]);
	});
});

const mediaReadout = vi.hoisted(() => ({
	output: null as VisualizationSnapshot | null,
	normal: [] as ProgrammerFixtureValue[],
	pending: [] as ProgrammerFixtureValue[],
	route: "normal" as "normal" | "preload",
	ready: true,
	error: null as Error | null,
	authority: "server-a",
	sessionId: "client-a",
	patchFixtures: [] as PatchedFixture[],
	patchStatus: "ready",
	patchError: null as string | null,
	patchShowId: "show-a",
	patchPending: new Set<string>(),
	patchListeners: new Set<() => void>(),
	read: vi.fn(),
	dispatch: vi.fn(),
	replace: vi.fn(),
	submit: vi.fn(),
}));
vi.mock("../features/patch/PatchState", () => ({
	useSelectedPatchedFixtures: (ids: readonly string[], active: boolean) =>
		active
			? mediaReadout.patchFixtures.filter((f) => ids.includes(f.fixture_id))
			: [],
	usePatchStatus: () => ({
		status: mediaReadout.patchStatus,
		error: mediaReadout.patchError,
	}),
}));
vi.mock("../features/patch/PatchContext", () => ({
	usePatchStoreOrNull: () => ({
		subscribe: (listener: () => void) => {
			mediaReadout.patchListeners.add(listener);
			return () => mediaReadout.patchListeners.delete(listener);
		},
		getSnapshot: () => ({
			showId: mediaReadout.patchShowId,
			pendingFixtureIds: mediaReadout.patchPending,
		}),
	}),
}));
vi.mock("../state/AppContext", () => ({
	useApp: () => ({ dispatch: mediaReadout.dispatch }),
}));
vi.mock("../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => ({
		mediaServers: [
			{
				fixture_id: "server",
				name: "Pixel",
				endpoint: null,
				status: { online: false, last_success: null, last_error: null },
				layers: [
					{ fixture_id: "layer-1", head_index: 1 },
					{ fixture_id: "layer-8", head_index: 8 },
				],
			},
		],
	}),
}));
vi.mock("../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesView: () => ({ fixtureValues: mediaReadout.normal }),
}));
vi.mock(
	"../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({
		useProgrammerPreloadValuesView: () => ({
			fixtureValues: mediaReadout.pending,
		}),
	}),
);
vi.mock(
	"../features/programmerValues/useProgrammerValuesMutationQueue",
	async (importOriginal) => ({
		...(await importOriginal<
			typeof import("../features/programmerValues/useProgrammerValuesMutationQueue")
		>()),
		useProgrammerValuesMutationQueue: () => ({
			route: mediaReadout.route,
			submitLatest: mediaReadout.submit,
		}),
	}),
);
vi.mock(
	"../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionActions: () => ({ replace: mediaReadout.replace }),
	}),
);
vi.mock("../features/visualizationRuntime/VisualizationRuntimeView", () => ({
	useVisualizationRuntimeView: () => ({
		snapshot: mediaReadout.output,
		ready: mediaReadout.ready,
		error: mediaReadout.error,
	}),
	useVisualizationRuntimeRead: (
		lane: string,
		options: { fixtureIds: string[] },
	) => {
		const ids = options.fixtureIds.join(",");
		const authority = mediaReadout.authority;
		const sessionId = mediaReadout.sessionId;
		return useCallback(
			() => mediaReadout.read(lane, ids, authority),
			[lane, ids, authority, sessionId],
		);
	},
}));
function cueSnapshot(level = 0.32): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-10-17T00:00:00Z",
		grand_master: 1,
		blackout: false,
		values: [
			{
				fixture_id: "layer-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 1 },
			},
			{
				fixture_id: "layer-8",
				attribute: "intensity",
				value: { kind: "normalized", value: level },
			},
			{
				fixture_id: "layer-8",
				attribute: "media.scale.x",
				value: { kind: "normalized", value: 0.56 },
			},
		],
	};
}
function cueColour(): ProgrammerFixtureValue {
	const mutation = mediaColourMutation("layer-8", "#000000");
	if (!mutation || mutation.action !== "set_fixture")
		throw new Error("Missing semantic colour");
	return {
		fixtureId: "layer-8",
		attribute: "color",
		value: mutation.value,
		programmerOrder: 0,
		fade: false,
		fadeMillis: null,
		delayMillis: null,
	};
}
function controlModel(values: readonly ProgrammerFixtureValue[]) {
	const server = {
		fixture_id: "server",
		name: "Pixel",
		endpoint: null,
		status: { online: false, last_success: null, last_error: null },
		layers: [{ fixture_id: "layer-8", head_index: 8 }],
	};
	const input: BuildMediaPaneModelInput = {
		inspection: EMPTY_MEDIA_INSPECTION,
		inspectionError: null,
		servers: [server],
		selectedServer: server,
		selectedServerId: "server",
		selectedLayerId: "layer-8",
		browserMode: "media",
		selectedControlSectionId: "playback",
		mainSectionId: "content",
		rightPaneVisible: true,
		draftFolderId: "0",
		draftFileId: null,
		thumbnailUrls: {},
		previewUrls: {},
		liveProgrammer: [],
		controlValues: values,
	};
	return buildMediaPaneModel(input).controlSections.flatMap(
		(section) => section.controls,
	);
}
function selectedValues(fixtureId = "layer-8", sourceKey = "server-a") {
	return createElement(MediaValuesHarness, { fixtureId, sourceKey });
}
function MediaValuesHarness({
	fixtureId,
	sourceKey,
}: {
	fixtureId: string;
	sourceKey: string;
}) {
	const values = useMediaProgrammerValues(true, mediaReadout.route, {
		fixtureId,
		parentFixtureId: "server",
		sourceKey,
		output:
			mediaReadout.ready && !mediaReadout.error ? mediaReadout.output : null,
	});
	return createElement("output", null, JSON.stringify(values ?? []));
}
function readValues(view: ReturnType<typeof render>) {
	return JSON.parse(
		view.container.querySelector("output")?.textContent ?? "[]",
	) as ProgrammerFixtureValue[];
}
function deferredSnapshot() {
	let resolve!: (value: VisualizationSnapshot) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<VisualizationSnapshot>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

describe("Media selected Cue readouts", () => {
	beforeEach(() => {
		vi.stubGlobal(
			"ResizeObserver",
			class {
				observe() {}
				disconnect() {}
			},
		);
		mediaReadout.patchFixtures = [];
		mediaReadout.patchStatus = "ready";
		mediaReadout.patchError = null;
		mediaReadout.patchShowId = "show-a";
		mediaReadout.patchPending = new Set();
		mediaReadout.output = cueSnapshot();
		mediaReadout.normal = [];
		mediaReadout.pending = [];
		mediaReadout.ready = true;
		mediaReadout.error = null;
		mediaReadout.route = "normal";
		mediaReadout.authority = "server-a";
		mediaReadout.sessionId = "client-a";
		mediaReadout.read.mockReset().mockResolvedValue(cueSnapshot());
		mediaReadout.dispatch.mockClear();
		mediaReadout.replace.mockClear();
		mediaReadout.submit.mockClear();
	});
	afterEach(cleanup);
	it("reads Cue-only selected Dimmer through the bounded logical-head transport", async () => {
		const view = render(
			createElement(MediaPaneWindow, {
				mediaPaneState: { serverId: "server", layerId: "layer-8" },
			}),
		);
		await waitFor(() =>
			expect(within(view.container).getByText("32%")).toBeInTheDocument(),
		);
		expect(mediaReadout.read).toHaveBeenCalledWith(
			"normal",
			"layer-8",
			"server-a",
		);
		expect(
			mediaReadout.read.mock.calls.every((call) => call[1] === "layer-8"),
		).toBe(true);
		expect(mediaReadout.submit).not.toHaveBeenCalled();
		expect(mediaReadout.replace).not.toHaveBeenCalled();
		expect(mediaReadout.dispatch).not.toHaveBeenCalled();
		fireEvent.click(
			within(view.container).getByRole("button", { name: /^Layer 1 / }),
		);
		await waitFor(() =>
			expect(within(view.container).getByText("100%")).toBeInTheDocument(),
		);
		expect(mediaReadout.read).toHaveBeenLastCalledWith(
			"normal",
			"layer-1",
			"server-a",
		);
		expect(mediaReadout.replace).toHaveBeenCalledWith({
			resolvedFixtures: ["layer-1"],
		});
	});
	it("keeps selected Frame controls and semantic Colour alongside Cue Dimmer", async () => {
		const colour = cueColour();
		mediaReadout.read.mockResolvedValue({
			...cueSnapshot(),
			values: [
				...cueSnapshot().values,
				{ fixture_id: "layer-8", attribute: "color", value: colour.value },
			],
		});
		const view = render(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(3));
		expect(readValues(view)).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					attribute: "media.scale.x",
					value: { kind: "normalized", value: 0.56 },
				}),
			]),
		);
		const controls = controlModel(readValues(view));
		expect(controls.find((v) => v.id === "intensity")).toMatchObject({
			kind: "value",
			value: 32,
			display: "32%",
			disabled: false,
		});
		expect(controls.find((v) => v.id === "media.scale.x")).toMatchObject({
			kind: "value",
			value: expect.closeTo(2.08),
			disabled: false,
		});
		expect(controls.find((v) => v.id === "color.tint")).toMatchObject({
			kind: "color",
			value: "#000000",
			disabled: false,
		});
		expect(
			mediaColourMutation("layer-8", "#0000ff", readValues(view)),
		).toMatchObject({
			value: {
				kind: "color_program",
				value: {
					intent: {
						recipe: { rgb: [0, 0, 1] },
						white_target: { kelvin: 6500, duv: 0 },
					},
				},
			},
		});
	});
	it("preserves explicit normal and pending zero without changing accepted live output", async () => {
		const value = {
			fixtureId: "layer-8",
			attribute: "intensity",
			value: { kind: "normalized" as const, value: 0 },
			programmerOrder: 1,
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		};
		mediaReadout.normal = [value];
		const view = render(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(2));
		expect(
			readValues(view).find((v) => v.attribute === "intensity")?.value,
		).toEqual(value.value);
		mediaReadout.normal = [];
		mediaReadout.pending = [value];
		mediaReadout.route = "preload";
		view.rerender(selectedValues());
		expect(
			readValues(view).find((v) => v.attribute === "intensity")?.value,
		).toEqual(value.value);
		expect(mediaReadout.output?.values[1].value).toEqual({
			kind: "normalized",
			value: 0.32,
		});
		mediaReadout.route = "normal";
		view.rerender(selectedValues());
		expect(
			readValues(view).find((v) => v.attribute === "intensity")?.value,
		).toEqual({ kind: "normalized", value: 0.32 });
	});
	it("coalesces pushed updates and retires old fixture and source results", async () => {
		const old = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(old.promise);
		const view = render(selectedValues());
		mediaReadout.output = cueSnapshot();
		view.rerender(selectedValues());
		mediaReadout.output = cueSnapshot();
		view.rerender(selectedValues());
		expect(mediaReadout.read).toHaveBeenCalledTimes(1);
		view.rerender(selectedValues("layer-1", "server-b"));
		await waitFor(() => expect(readValues(view)[0]?.fixtureId).toBe("layer-1"));
		await act(async () => old.resolve(cueSnapshot(0.99)));
		expect(readValues(view)).toHaveLength(1);
		expect(readValues(view)[0]?.value).toEqual({
			kind: "normalized",
			value: 1,
		});
	});
	it("clears stale values on read failure, readiness loss and inactive view", async () => {
		const view = render(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(2));
		mediaReadout.read.mockRejectedValueOnce(new Error("Disconnected"));
		mediaReadout.output = cueSnapshot();
		view.rerender(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(0));
		expect(
			controlModel(readValues(view)).find((v) => v.id === "intensity"),
		).toMatchObject({ display: "Unknown", disabled: true });
		mediaReadout.ready = false;
		view.rerender(selectedValues());
		expect(readValues(view)).toHaveLength(0);
		view.unmount();
		const count = mediaReadout.read.mock.calls.length;
		const inactive = render(createElement(MediaPaneWindow, { active: false }));
		expect(mediaReadout.read).toHaveBeenCalledTimes(count);
		expect(mediaReadout.submit).not.toHaveBeenCalled();
		inactive.unmount();
	});
	it("does not adopt an old endpoint result for the same selected head", async () => {
		const old = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(old.promise);
		const view = render(selectedValues("layer-8", "old-endpoint"));
		view.rerender(selectedValues("layer-8", "new-endpoint"));
		await waitFor(() => expect(readValues(view)).toHaveLength(2));
		await act(async () => old.resolve(cueSnapshot(0.99)));
		expect(
			readValues(view).find((v) => v.attribute === "intensity")?.value,
		).toEqual({ kind: "normalized", value: 0.32 });
	});
	it("retires a late result when the same show changes authenticated client session", async () => {
		const old = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(old.promise);
		const view = render(selectedValues());
		mediaReadout.sessionId = "client-b";
		view.rerender(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(2));
		await act(async () => old.resolve(cueSnapshot(0.99)));
		expect(
			readValues(view).find((v) => v.attribute === "intensity")?.value,
		).toEqual({ kind: "normalized", value: 0.32 });
	});
	it("retires old authority errors and refuses wrong show or pending-lane replies", async () => {
		const old = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(old.promise);
		const view = render(selectedValues());
		mediaReadout.authority = "server-b";
		view.rerender(selectedValues());
		await waitFor(() => expect(readValues(view)).toHaveLength(2));
		await act(async () => old.reject(new Error("Old server retired")));
		expect(readValues(view)).toHaveLength(2);
		mediaReadout.output = { ...cueSnapshot(), scope: { show_id: "show-b" } };
		mediaReadout.read.mockResolvedValueOnce({
			...cueSnapshot(),
			scope: { show_id: "show-a" },
		});
		view.rerender(selectedValues());
		await act(async () => {});
		expect(readValues(view)).toHaveLength(0);
		mediaReadout.read.mockResolvedValueOnce({
			...cueSnapshot(),
			scope: { show_id: "show-b" },
			preload: true,
		});
		mediaReadout.output = { ...mediaReadout.output };
		view.rerender(selectedValues());
		await act(async () => {});
		expect(readValues(view)).toHaveLength(0);
	});
	it("keeps invalid normalized controls unknown and semantic Cue envelopes intact", async () => {
		for (const value of [NaN, Infinity, -1, 1.1]) {
			const controls = controlModel([
				{
					...cueColour(),
					attribute: "intensity",
					value: { kind: "normalized", value },
				},
			]);
			expect(
				controls.find((control) => control.id === "intensity"),
			).toMatchObject({ display: "Unknown", disabled: true });
		}
		const cue = cueColour();
		if (
			cue.value.kind !== "color_program" ||
			cue.value.value.kind !== "semantic"
		)
			throw new Error("Expected semantic Cue");
		cue.value.value.intent.white_blend = 0.25;
		cue.value.value.intent.recipe.amber = 0.2;
		cue.value.value.intent.uv.amount = 0.4;
		cue.value.value.intent.relative_output = 0.8;
		const { mediaProgrammerValues } = await import(
			"./media/useMediaProgrammerValues"
		);
		expect(
			mediaColourMutation(
				"layer-8",
				"#0000ff",
				mediaProgrammerValues([], undefined, [cue]),
			),
		).toMatchObject({
			value: {
				value: {
					intent: {
						white_blend: 0.25,
						uv: { amount: 0.4 },
						relative_output: 0.8,
						recipe: { rgb: [0, 0, 1], amber: 0.2 },
					},
				},
			},
		});
	});
});

function defaultMediaFixture(volume = 1, revision = 1): PatchedFixture {
	const parameter = (attribute: string, value: number) => ({
		attribute,
		default: value,
		components: [],
		virtual_dimmer: false,
		capabilities: [],
	});
	const profile = blankFixtureProfile();
	profile.id = "profile-a";
	profile.revision = revision;
	const mode = profile.modes[0];
	mode.id = "mode-a";
	mode.heads = Array.from({ length: 9 }, (_, i) => ({
		id: `head-${i}`,
		master_shared: i === 0,
		name: String(i),
	}));
	const definition = fixtureDefinitionFromProfileMode(profile, mode);
	return {
		fixture_id: "server",
		universe: null,
		address: null,
		logical_heads: [
			{ fixture_id: "layer-8", head_index: 8, profile_head_id: "head-8" },
		],
		definition: {
			...definition,
			heads: [
				{
					index: 0,
					name: "Master",
					shared: true,
					parameters: [parameter("intensity", 0.7)],
				},
				{
					index: 8,
					name: "Layer8",
					shared: false,
					parameters: [
						parameter("intensity", 0),
						parameter("volume", volume),
						parameter("media.play_mode", 0),
						parameter("media.playback_speed", 127 / 255),
						parameter("media.in_point", 0),
						parameter("media.out_point", 0),
						parameter("color.red", 1),
						parameter("color.green", 1),
						parameter("color.blue", 1),
					],
				},
			],
		},
	};
}
describe("Media authoritative profile homes", () => {
	beforeEach(() => {
		vi.stubGlobal(
			"ResizeObserver",
			class {
				observe() {}
				disconnect() {}
			},
		);
		mediaReadout.patchFixtures = [defaultMediaFixture()];
		mediaReadout.patchStatus = "ready";
		mediaReadout.patchError = null;
		mediaReadout.patchPending = new Set();
		mediaReadout.patchShowId = "show-a";
		mediaReadout.normal = [];
		mediaReadout.pending = [];
		mediaReadout.route = "normal";
		mediaReadout.ready = true;
		mediaReadout.error = null;
		mediaReadout.output = { ...cueSnapshot(), scope: { show_id: "show-a" } };
		mediaReadout.read
			.mockReset()
			.mockResolvedValue({ ...mediaReadout.output, values: [] });
		mediaReadout.submit.mockClear();
		mediaReadout.replace.mockClear();
		mediaReadout.dispatch.mockClear();
	});
	afterEach(cleanup);
	it("mounts real Window and Surface with empty Normal and permits first Play, volume and Dimmer edits", async () => {
		const view = render(
			createElement(MediaPaneWindow, {
				mediaPaneState: { serverId: "server", layerId: "layer-8" },
			}),
		);
		const ui = within(view.container);
		await waitFor(() =>
			expect(ui.getByRole("button", { name: "Play looped" })).toBeEnabled(),
		);
		expect(ui.getByRole("slider", { name: "Dimmer" })).toHaveValue("0");
		expect(ui.getByRole("slider", { name: "Volume" })).toHaveValue("100");
		expect(mediaReadout.submit).not.toHaveBeenCalled();
		expect(mediaReadout.replace).not.toHaveBeenCalled();
		fireEvent.click(ui.getByRole("button", { name: "Play looped" }));
		expect(mediaReadout.submit.mock.lastCall?.[1]).toEqual([
			expect.objectContaining({
				action: "set_fixture",
				fixtureId: "layer-8",
				attribute: "media.play_mode",
				value: { kind: "normalized", value: 0 },
			}),
		]);
		mediaReadout.submit.mockClear();
		fireEvent.input(ui.getByRole("slider", { name: "Volume" }), {
			target: { value: "50" },
		});
		expect(mediaReadout.submit.mock.lastCall?.[1]).toEqual([
			expect.objectContaining({
				fixtureId: "layer-8",
				attribute: "volume",
				value: { kind: "normalized", value: 0.5 },
			}),
		]);
		mediaReadout.submit.mockClear();
		fireEvent.input(ui.getByRole("slider", { name: "Dimmer" }), {
			target: { value: "50" },
		});
		expect(mediaReadout.submit.mock.lastCall?.[1]).toEqual([
			expect.objectContaining({
				fixtureId: "layer-8",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
			}),
		]);
	});
	it("uses exact nonstandard head home only after successful current read and retires replacement", async () => {
		mediaReadout.patchFixtures = [defaultMediaFixture(0.2)];
		const initial = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(initial.promise);
		const view = render(selectedValues());
		expect(readValues(view)).toEqual([]);
		await act(async () =>
			initial.resolve({
				...cueSnapshot(),
				scope: { show_id: "show-a" },
				values: [],
			}),
		);
		expect(
			readValues(view).find((v) => v.attribute === "volume")?.value,
		).toEqual({ kind: "normalized", value: 0.2 });
		const replacement = deferredSnapshot();
		mediaReadout.read.mockReturnValueOnce(replacement.promise);
		mediaReadout.patchFixtures = [defaultMediaFixture(0.6, 2)];
		view.rerender(selectedValues());
		expect(readValues(view)).toEqual([]);
		await act(async () =>
			replacement.resolve({
				...cueSnapshot(),
				scope: { show_id: "show-a" },
				values: [],
			}),
		);
		expect(
			readValues(view).find((v) => v.attribute === "volume")?.value,
		).toEqual({ kind: "normalized", value: 0.6 });
	});
	it("keeps explicit Normal and Pending zero above homes and semantic Cue colour intact", async () => {
		const colour = cueColour();
		mediaReadout.read.mockResolvedValue({
			...mediaReadout.output,
			values: [
				{
					fixture_id: "layer-8",
					attribute: "volume",
					value: { kind: "normalized", value: 0 },
				},
				{ fixture_id: "layer-8", attribute: "color", value: colour.value },
			],
		});
		const view = render(selectedValues());
		await waitFor(() =>
			expect(readValues(view).some((v) => v.attribute === "volume")).toBe(true),
		);
		expect(
			readValues(view).find((v) => v.attribute === "volume")?.value,
		).toEqual({ kind: "normalized", value: 0 });
		expect(
			controlModel(readValues(view)).find((c) => c.id === "color.tint"),
		).toMatchObject({ value: "#000000", disabled: false });
		mediaReadout.pending = [
			{
				...colour,
				attribute: "volume",
				value: { kind: "normalized", value: 0 },
			},
		];
		mediaReadout.route = "preload";
		view.rerender(selectedValues());
		expect(
			readValues(view).find((v) => v.attribute === "volume")?.value,
		).toEqual({ kind: "normalized", value: 0 });
		expect(mediaReadout.submit).not.toHaveBeenCalled();
	});
	it("exposes only shared Master homes or the exact selected logical head", async () => {
		const master = render(selectedValues("server"));
		await waitFor(() => expect(readValues(master)).toHaveLength(1));
		expect(readValues(master)[0]).toMatchObject({
			fixtureId: "server",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.7 },
		});
		master.unmount();
		const absent = render(selectedValues("layer-1"));
		await act(async () => {});
		expect(readValues(absent)).toEqual([]);
	});
	it("retires homes on pending-only Patch notifications and waits for a fresh read on recovery", async () => {
		const view = render(selectedValues());
		await waitFor(() =>
			expect(readValues(view).some((v) => v.attribute === "volume")).toBe(true),
		);
		act(() => {
			mediaReadout.patchPending = new Set(["server"]);
			for (const listener of mediaReadout.patchListeners) listener();
		});
		expect(readValues(view)).toEqual([]);
		let resolveRead!: (value: VisualizationSnapshot) => void;
		mediaReadout.read.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveRead = resolve;
				}),
		);
		act(() => {
			mediaReadout.patchPending = new Set();
			for (const listener of mediaReadout.patchListeners) listener();
		});
		expect(readValues(view)).toEqual([]);
		await act(async () => resolveRead({ ...mediaReadout.output!, values: [] }));
		await waitFor(() =>
			expect(readValues(view).some((v) => v.attribute === "volume")).toBe(true),
		);
		expect(mediaReadout.submit).not.toHaveBeenCalled();
	});
	it.each([
		"read-error",
		"patch-error",
		"pending-patch",
		"wrong-show",
		"wrong-profile-head",
	])(
		"retires homes for %s without masking unknown or writing",
		async (reason) => {
			const view = render(selectedValues());
			await waitFor(() =>
				expect(readValues(view).some((v) => v.attribute === "volume")).toBe(
					true,
				),
			);
			if (reason === "read-error")
				mediaReadout.read.mockRejectedValueOnce(new Error("unavailable"));
			if (reason === "patch-error") mediaReadout.patchError = "unavailable";
			if (reason === "pending-patch")
				mediaReadout.patchPending = new Set(["server"]);
			if (reason === "wrong-show") mediaReadout.patchShowId = "show-b";
			if (reason === "wrong-profile-head") {
				const fixture = defaultMediaFixture();
				fixture.logical_heads[0].profile_head_id = "another-head";
				mediaReadout.patchFixtures = [fixture];
			}
			mediaReadout.output = { ...mediaReadout.output! };
			view.rerender(selectedValues());
			await waitFor(() => expect(readValues(view)).toEqual([]));
			expect(
				controlModel(readValues(view)).find((c) => c.id === "volume"),
			).toMatchObject({ display: "Unknown", disabled: true });
			expect(mediaReadout.submit).not.toHaveBeenCalled();
		},
	);
});
