import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCueRecording } from "../features/cueRecording/CueRecordingProvider";
import type { CueRecordingRequest } from "../features/cueRecording/contracts";
import type {
	ShowObject,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
} from "../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../features/showObjects/ShowObjectsView";
import type { ShowObjectsEventObserver } from "../features/showObjects/transport";
import { PlaybackApiClient } from "./client/playback";
import { LightApiClient } from "./LightApiClient";
import { ServerProvider, useServer } from "./ServerContext";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const CUE_LIST_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const CUE_ID = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const CUE_KINDS = ["cue_list", "playback", "playback_page"] as const;

const boundaries = vi.hoisted(() => ({
	loadCollection: vi.fn(),
	loadObjectSnapshot: vi.fn(),
	loadObject: vi.fn(),
	recordCue: vi.fn(),
	selectedPlayback: vi.fn(() => null),
	subscribeShowObjects: vi.fn(),
}));

vi.mock("../features/server/useServerPolling", () => ({
	useServerPolling: vi.fn(),
}));
vi.mock("../features/server/useShowData", () => ({
	useShowObjects: () => vi.fn().mockResolvedValue(undefined),
	useServerRefresh: () => vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../features/server/useServerConnection", async () => {
	const { useEffect } = await import("react");
	return {
		useServerConnection: (state: {
			setBootstrap(value: unknown): void;
			setSession(value: unknown): void;
			setConnectionGeneration(value: number): void;
		}) => {
			useEffect(() => {
				const user = { id: USER_ID, name: "Operator", enabled: true };
				const desk = {
					id: DESK_ID,
					name: "Main",
					osc_alias: "main",
					columns: 1,
					rows: 1,
					buttons: 1,
				};
				state.setBootstrap({
					api_version: "2",
					attribute_registry: [],
					users: [user],
					desks: [desk],
					clients: [],
					active_show: {
						id: SHOW_ID,
						name: "Show",
						path: "show",
						revision: 1,
						updated_at: "",
					},
					active_programmers: [],
					frame_rate_hz: 44,
					output_health: {
						frames_sent: 0,
						packets_sent: 0,
						send_errors: 0,
						deadline_misses: 0,
						maximum_lateness_micros: 0,
						frame_hz: 44,
						last_tick_micros: 0,
						maximum_tick_micros: 0,
						scheduler_utilization: 0,
					},
					active_timecode_source: null,
					active_timecode: null,
					active_show_error: null,
					hardware_connected: false,
				});
				state.setSession({
					session_id: "ffffffff-ffff-4fff-8fff-ffffffffffff",
					client_id: "99999999-9999-4999-8999-999999999999",
					token: "session-token",
					user,
					desk,
				});
				state.setConnectionGeneration(1);
			}, []);
		},
	};
});
vi.mock("./useServerFeatureBoundaries", () => {
	const idle = vi.fn();
	const boundary = {
		showObjectsTransport: { subscribe: boundaries.subscribeShowObjects },
		showObjectsAuthorityKey: "server-session-a",
		loadShowObjectCollection: boundaries.loadCollection,
		loadShowObjectSnapshot: boundaries.loadObjectSnapshot,
		loadShowObject: boundaries.loadObject,
		cueRecordingTransport: { record: boundaries.recordCue },
		selectedCueRecordingPlayback: boundaries.selectedPlayback,
		reportCueRecordingError: idle,
		groupRecordingTransport: null,
		loadGroupForRepair: idle,
		reportGroupRecordingError: idle,
		presetRecordingTransport: null,
		loadPresetForRepair: idle,
		reportPresetRecordingError: idle,
		playbackTransport: null,
		programmingTransport: null,
		programmerLifecycleTransport: null,
		programmerValuesTransport: null,
		programmerPreloadValuesTransport: null,
		programmerPreloadPlaybackQueueTransport: null,
		programmerCaptureModeTransport: null,
		programmerLifecycleAuthorityKey: "server-session-a",
		programmerValuesAuthorityKey: "server-session-a",
		programmerPreloadValuesAuthorityKey: "server-session-a",
		programmerPreloadPlaybackQueueAuthorityKey: "server-session-a",
		programmerCaptureModeAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: idle,
		loadProgrammingInteractionSnapshot: idle,
		loadProgrammerLifecycleSnapshot: idle,
		loadProgrammerValuesSnapshot: idle,
		applyProgrammerValuesAction: idle,
		loadProgrammerPreloadValuesSnapshot: idle,
		applyProgrammerPreloadValuesAction: idle,
		loadProgrammerPreloadPlaybackQueueSnapshot: idle,
		loadProgrammerCaptureModeSnapshot: idle,
		reportShowObjectError: idle,
		reportPlaybackError: idle,
		reportProgrammingSessionError: idle,
		reportProgrammingMutationError: idle,
		reportProgrammerLifecycleSessionError: idle,
		reportProgrammerValuesSessionError: idle,
		reportProgrammerValuesMutationError: idle,
		reportProgrammerPreloadValuesSessionError: idle,
		reportProgrammerPreloadValuesMutationError: idle,
		reportProgrammerPreloadPlaybackQueueSessionError: idle,
		reportProgrammerCaptureModeSessionError: idle,
	};
	return { useServerFeatureBoundaries: () => boundary };
});

let unrelatedRenders = 0;

function UnrelatedServerConsumer() {
	useServer();
	unrelatedRenders += 1;
	return null;
}

function RecordCueButton() {
	const actions = useCueRecording();
	return (
		<button
			type="button"
			onClick={() =>
				void actions?.record({
					target: { kind: "page_slot", page: 4, slot: 2 },
					operation: "overwrite",
					cueNumber: 1,
					timing: {},
					cueOnly: false,
					capturePolicy: "current_capture",
					activationPolicy: "hold",
				})
			}
		>
			Record Cue
		</button>
	);
}

function CueValuesProbe() {
	useShowObjectKindsView(CUE_KINDS);
	const cueLists = useCueLists();
	const playbacks = usePlaybackDefinitions();
	const pages = usePlaybackPages();
	return (
		<span data-testid="cue-values">
			{cueLists[0]?.body.cues[0]?.name ?? "No Cue"}/
			{playbacks[0]?.body.name ?? "No Playback"}/
			{pages[0]?.body.name ?? "No Page"}
		</span>
	);
}

function Harness({ showCueValues }: { showCueValues: boolean }) {
	return (
		<ServerProvider>
			<UnrelatedServerConsumer />
			<RecordCueButton />
			{showCueValues ? <CueValuesProbe /> : null}
		</ServerProvider>
	);
}

function cueList(revision: number, cueName: string): ShowObject<"cue_list"> {
	return {
		kind: "cue_list",
		id: CUE_LIST_ID,
		revision,
		updated_at: "",
		body: {
			id: CUE_LIST_ID,
			name: "Main",
			priority: 0,
			mode: "sequence",
			looped: false,
			cues: [
				{
					id: CUE_ID,
					number: 1,
					name: cueName,
					fade_millis: 1000,
					delay_millis: 0,
					trigger: { type: "manual" },
					cue_only: false,
					changes: [],
					group_changes: [],
					phasers: [],
				},
			],
		},
	};
}

function playback(revision: number, name: string): ShowObject<"playback"> {
	return {
		kind: "playback",
		id: "7",
		revision,
		updated_at: "",
		body: {
			number: 7,
			name,
			target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
			buttons: ["go_minus", "go", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
		},
	};
}

function page(revision: number, name: string): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id: "4",
		revision,
		updated_at: "",
		body: { number: 4, name, slots: { 2: 7 } },
	};
}

function objectsFor(kind: ShowObjectKind, revision: number, label: string) {
	if (kind === "cue_list") return [cueList(revision, label)];
	if (kind === "playback") return [playback(revision, label)];
	if (kind === "playback_page") return [page(revision, label)];
	return [];
}

afterEach(() => vi.restoreAllMocks());

describe("ServerProvider Cue recording boundary", () => {
	it("stays dormant until a Cue view mounts and isolates three-object outcomes and events", async () => {
		boundaries.loadCollection.mockReset();
		boundaries.loadObjectSnapshot.mockReset();
		boundaries.loadObject.mockReset();
		boundaries.recordCue.mockReset();
		boundaries.subscribeShowObjects.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		const broadPlaybacks = vi.spyOn(PlaybackApiClient.prototype, "playbacks");
		let observer: ShowObjectsEventObserver | null = null;
		boundaries.loadCollection.mockImplementation(
			async (_showId: string, kind: ShowObjectKind) => ({
				objects: objectsFor(kind, 1, "Initial"),
				showRevision: 7,
			}),
		);
		boundaries.subscribeShowObjects.mockImplementation(
			(_showId, _scope, _cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		boundaries.recordCue.mockImplementation(
			async (
				_showId: string,
				_expectedShowRevision: number,
				request: CueRecordingRequest,
			) => ({
				requestId: request.requestId,
				correlationId: "12121212-1212-4212-8212-121212121212",
				replayed: false,
				capturedSource: "normal" as const,
				status: "changed" as const,
				showRevision: 8,
				recordedCue: { id: CUE_ID, number: 1, deleted: false },
				projections: {
					cueList: cueList(2, "Outcome"),
					playback: playback(2, "Outcome"),
					page: page(2, "Outcome"),
				},
				showEventSequence: 12,
				runtime: null,
			}),
		);
		unrelatedRenders = 0;
		const rendered = render(<Harness showCueValues={false} />);
		await waitFor(() => expect(unrelatedRenders).toBeGreaterThan(0));

		expect(boundaries.loadCollection).not.toHaveBeenCalled();
		expect(boundaries.loadObjectSnapshot).not.toHaveBeenCalled();
		expect(boundaries.loadObject).not.toHaveBeenCalled();
		expect(boundaries.subscribeShowObjects).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();
		expect(broadPlaybacks).not.toHaveBeenCalled();

		rendered.rerender(<Harness showCueValues />);
		await waitFor(() =>
			expect(screen.getByTestId("cue-values")).toHaveTextContent(
				"Initial/Initial/Initial",
			),
		);
		expect(boundaries.loadCollection).toHaveBeenCalledTimes(3);
		expect(boundaries.subscribeShowObjects).toHaveBeenCalledOnce();
		const rendersBeforeOutcome = unrelatedRenders;

		fireEvent.click(screen.getByRole("button", { name: "Record Cue" }));
		await waitFor(() =>
			expect(screen.getByTestId("cue-values")).toHaveTextContent(
				"Outcome/Outcome/Outcome",
			),
		);
		expect(boundaries.recordCue).toHaveBeenCalledWith(
			SHOW_ID,
			7,
			expect.objectContaining({
				target: { kind: "page_slot", page: 4, slot: 2 },
			}),
		);
		expect(unrelatedRenders).toBe(rendersBeforeOutcome);

		act(() =>
			observer?.message({
				type: "event",
				change: {
					showId: SHOW_ID,
					showRevision: 9,
					eventSequence: 13,
					changes: [
						{
							kind: "cue_list",
							objectId: CUE_LIST_ID,
							objectRevision: 3,
							body: cueList(3, "Event").body,
							deleted: false,
						},
						{
							kind: "playback",
							objectId: "7",
							objectRevision: 3,
							body: playback(3, "Event").body,
							deleted: false,
						},
						{
							kind: "playback_page",
							objectId: "4",
							objectRevision: 3,
							body: page(3, "Event").body,
							deleted: false,
						},
					],
				},
			}),
		);

		expect(screen.getByTestId("cue-values")).toHaveTextContent(
			"Event/Event/Event",
		);
		expect(unrelatedRenders).toBe(rendersBeforeOutcome);
		expect(boundaries.loadObjectSnapshot).not.toHaveBeenCalled();
		expect(boundaries.loadObject).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();
		expect(broadPlaybacks).not.toHaveBeenCalled();
	});
});
