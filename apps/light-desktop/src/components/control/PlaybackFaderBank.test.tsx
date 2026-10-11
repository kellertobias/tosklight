import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackSoloProvider } from "../../features/playbackSolo/PlaybackSoloContext";
import type {
	PlaybackSoloRegion,
	PlaybackSoloTransport,
} from "../../features/playbackSolo/contracts";
import { registerControlSurfaceTarget } from "../../features/controlSurfaceInteraction/registry";
import {
	PlaybackFaderBank as PhysicalPlaybackFaderBank,
	playbackRowUnits,
} from "./PlaybackFaderBank";

describe("playback row height units", () => {
	const oneButton = {
		first_position: 1,
		count: 1,
		presentation: "buttons" as const,
		button_count: 1,
	};
	const multipleButtons = {
		first_position: 11,
		count: 1,
		presentation: "buttons" as const,
		button_count: 3,
	};
	const fader = {
		first_position: 21,
		count: 1,
		presentation: "fader" as const,
		button_count: 1,
	};

	it("uses 1/1/2 units with attached hardware", () => {
		expect(
			[oneButton, multipleButtons, fader].map((row) =>
				playbackRowUnits(row, true),
			),
		).toEqual([1, 1, 2]);
	});

	it("uses 1/2/4 units for touch controls", () => {
		expect(
			[oneButton, multipleButtons, fader].map((row) =>
				playbackRowUnits(row, false),
			),
		).toEqual([1, 2, 4]);
	});
});

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommandLine: vi.fn(),
	refresh: vi.fn(),
	poolPlaybackAction: vi.fn(),
	resetCommandLine: vi.fn(),
	savePlaybackSlot: vi.fn(),
	clearPlaybackSlot: vi.fn(),
	mapExistingPlayback: vi.fn(),
	recordCue: vi.fn(),
	recordDefault: null as string | null,
	commandLine: "FIXTURE",
	error: null as string | null,
	hardwareConnected: false,
	topologyReady: true,
	runtimeReady: true,
	groupReady: true,
	showObjectView: vi.fn(),
	portableGroups: vi.fn(),
	topologyView: vi.fn(),
	runtimeProjectionMismatch: false,
	runtimeCueListIdOverride: null as string | null,
	state: {
		midiProfile: null,
		playbackColumns: 1,
		playbackRows: 1,
		playbackPage: 0,
		cueListSetTarget: 12 as number | null,
		cueListSetArmed: true,
		playbackSetArmed: false,
		shiftArmed: false,
		updateArmed: false,
		storeArmed: false,
		blackout: false,
	},
	playbacks: {
		active_page: 1,
		pages: [{ number: 1, name: "Main", slots: {} as Record<string, number> }],
		pool: [] as Array<Record<string, any>>,
		active: [] as Array<Record<string, any>>,
		cue_lists: [
			{
				id: "front",
				name: "Front sequence",
				pool_number: 1,
				cues: [] as Array<Record<string, any>>,
				mode: "sequence",
				priority: 0,
				looped: false,
			},
		],
		desk: { buttons: 3 },
		selected_playback: null as number | null,
	},
	scopedCueLists: [] as Array<Record<string, any>>,
}));

function canonicalPage(page: {
	number: number;
	name: string;
	slots: Record<string, number>;
}) {
	return {
		number: page.number,
		name: page.name,
		assignments: Object.fromEntries(
			Object.entries(page.slots).flatMap(([position, fixtureId]) => {
				const fixture = mocks.playbacks.pool.find(
					(p) => p.number === fixtureId,
				);
				if (!fixture) return [];
				const { number, ...definition } = fixture;
				return [[position, definition]];
			}),
		),
	};
}

function PlaybackFaderBank(
	props: ComponentProps<typeof PhysicalPlaybackFaderBank>,
) {
	return (
		<PhysicalPlaybackFaderBank
			{...props}
			hardwareConnected={mocks.hardwareConnected}
		/>
	);
}

vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../../features/programmingUpdate/ProgrammingUpdateProvider", () => ({
	useProgrammingUpdate: () =>
		mocks.recordDefault === null
			? null
			: {
					loadSettings: async () => ({
						cue_mode: "existing_in_current_cue",
						preset_mode: "update_existing",
						group_mode: "update_existing",
						show_update_modal_on_touch: true,
						record_default: mocks.recordDefault,
						update_default: "smart",
					}),
				},
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingCommandLineView: () => ({
			text: mocks.commandLine,
			target: "FIXTURE",
			pristine: false,
			revision: 1,
			pendingChoice: null,
		}),
		useProgrammingCommandLineActions: () => ({
			reset: mocks.resetCommandLine,
		}),
		useProgrammingInteractionStatus: () => ({ status: "ready", error: null }),
	}),
);
vi.mock("../../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackTopologyView: () => {
		mocks.topologyView();
		return {
			ready: mocks.topologyReady,
			error: mocks.error ? new Error(mocks.error) : null,
			cueLists: mocks.scopedCueLists.map((body) => ({
				kind: "cue_list",
				id: body.storageId ?? `cue-list-object-${body.id}`,
				revision: 2,
				updated_at: "",
				body,
			})),
			playbacks: mocks.playbacks.pool.map((body) => ({
				kind: "playback",
				id: `playback-object-${body.number}`,
				revision: 5,
				updated_at: "",
				body,
			})),
			pages: mocks.playbacks.pages.map((body) => ({
				kind: "playback_page",
				id: `page-object-${body.number}`,
				revision: 3,
				updated_at: "",
				body: canonicalPage(body),
			})),
		};
	},
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		assignPlayback: mocks.savePlaybackSlot,
		updatePlayback: mocks.savePlaybackSlot,
		clearPlayback: mocks.clearPlaybackSlot,
		mapExistingPlayback: mocks.mapExistingPlayback,
		error: mocks.error ? new Error(mocks.error) : null,
	}),
}));
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => {
	const reference = (number: unknown) =>
		typeof number === "number" ? { id: `cue-${number}`, number } : null;
	const cueListProjection = (number: number, playback: Record<string, any>) => {
		const active = mocks.playbacks.active.find(
			(candidate) => candidate.playback_number === number,
		);
		return {
			scope: { show_id: "show-1", show_revision: 9 },
			identity: { kind: "playback", page: 1, position: number },
			target: "cue_list",
			cue_list_id:
				mocks.runtimeCueListIdOverride ?? playback.target.cue_list_id,
			runtime: active
				? {
						cue_index: active.cue_index ?? -1,
						previous_index: active.previous_index ?? null,
						current: reference(
							active.current_cue_number ??
								mocks.scopedCueLists[0]?.cues?.[active.cue_index]?.number,
						),
						loaded: reference(active.loaded_cue_number),
						normal_next: reference(active.normal_next_cue_number),
						effective_next: reference(active.effective_next_cue_number),
						effective_next_is_loaded: Boolean(active.effective_next_is_loaded),
						paused: Boolean(active.paused),
						activated_at: active.activated_at ?? "",
						master: active.master ?? 0,
						fader_position: active.fader_position ?? active.master ?? 0,
						fader_pickup_required: Boolean(active.fader_pickup_required),
						fader_pickup_target: active.fader_pickup_target ?? null,
						flash: Boolean(active.flash),
						temporary: Boolean(active.temporary),
						temporary_active: Boolean(active.temporary_active),
						temporary_master: active.temporary_master ?? 0,
						swap_active: Boolean(active.swap_active),
						enabled: active.enabled ?? true,
						transition_timing_bypassed: Boolean(
							active.transition_timing_bypassed,
						),
						manual_xfade_position: active.manual_xfade_position ?? 0,
						manual_xfade_direction:
							active.manual_xfade_direction ?? "towards_high",
						manual_xfade_progress: active.manual_xfade_progress ?? 0,
					}
				: null,
		};
	};
	const projection = (number: number) => {
		const playback = mocks.playbacks.pool.find(
			(candidate) => candidate.number === number,
		);
		if (!playback)
			return {
				scope: { show_id: "show-1", show_revision: 9 },
				identity: { kind: "playback", page: 1, position: number },
				target: "missing",
			};
		if (mocks.runtimeProjectionMismatch)
			return {
				scope: { show_id: "show-1", show_revision: 9 },
				identity: { kind: "playback", page: 1, position: number },
				target: "missing",
			};
		if (playback.target.type === "cue_list")
			return cueListProjection(number, playback);
		const active = mocks.playbacks.active.find(
			(candidate) => candidate.playback_number === number,
		);
		return {
			scope: { show_id: "show-1", show_revision: 9 },
			identity: { kind: "playback", page: 1, position: number },
			target: playback.target.type,
			...(playback.target.type === "group"
				? {
						group_id: playback.target.group_id,
						master: active?.master ?? 1,
						flash_level: 1,
						fader_position: active?.fader_position ?? 0,
						fader_pickup_required: Boolean(active?.fader_pickup_required),
						fader_pickup_target: active?.fader_pickup_target ?? null,
					}
				: {}),
		};
	};
	const runtimeActions = { playbackAction: mocks.poolPlaybackAction };
	return {
		usePlaybackRuntimeActions: () => runtimeActions,
		usePlaybackRuntimeAuthority: () => null,
		usePlaybackRuntimeStatus: () => ({
			status: mocks.runtimeReady ? "ready" : "loading",
			error: null,
		}),
		usePlaybackDeskView: () => ({
			scope: { show_id: "show-1", show_revision: 9 },
			desk_id: "desk-1",
			command_page: mocks.playbacks.active_page,
			selected_surface_id: "main",
			resolved_pages: { main: mocks.playbacks.active_page },
			selected_playback:
				mocks.playbacks.selected_playback == null
					? null
					: { page: 1, position: 1 },
		}),
		usePlaybackProjectionMap: (
			addresses: Array<{ page: number; position: number }>,
		) =>
			new Map(
				addresses.map((address) => {
					const number = mocks.playbacks.pages.find(
						(p) => p.number === address.page,
					)?.slots[String(address.position)];
					const value = projection(number ?? address.position);
					return [
						`playback:${address.page}.${address.position}`,
						{ ...value, identity: { kind: "playback", ...address } },
					];
				}),
			),
	};
});
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	useDynamics: () => [],
	usePortableGroups: (enabled: boolean) => {
		mocks.portableGroups(enabled);
		return [];
	},
	useShowObjectCollectionsReady: () => mocks.groupReady,
	useCueLists: () =>
		mocks.scopedCueLists.map((body) => ({
			kind: "cue_list",
			id: body.id,
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackDefinitions: () =>
		mocks.playbacks.pool.map((body) => ({
			kind: "playback",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackPages: () =>
		mocks.playbacks.pages.map((body) => ({
			kind: "playback_page",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body: canonicalPage(body),
		})),
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: mocks.showObjectView,
	useShowObjectKindsView: () => undefined,
}));
vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));

afterEach(cleanup);

function resetPlaybackFaderMocks() {
	mocks.dispatch.mockReset();
	mocks.executeCommandLine.mockReset().mockResolvedValue(true);
	mocks.refresh.mockReset().mockResolvedValue(undefined);
	mocks.poolPlaybackAction
		.mockReset()
		.mockResolvedValue({ status: "no_change" });
	mocks.resetCommandLine.mockReset();
	mocks.savePlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.mapExistingPlayback
		.mockReset()
		.mockResolvedValue({ status: "changed" });
	mocks.recordCue.mockReset().mockResolvedValue({ status: "changed" });
	mocks.playbacks.cue_lists = [
		{
			id: "front",
			name: "Front sequence",
			pool_number: 1,
			cues: [],
			mode: "sequence",
			priority: 0,
			looped: false,
		},
	];
	mocks.scopedCueLists = mocks.playbacks.cue_lists.map((cueList) => ({
		...cueList,
		cues: [...cueList.cues],
	}));
	mocks.commandLine = "FIXTURE";
	mocks.recordDefault = null;
	mocks.error = null;
	mocks.hardwareConnected = false;
	mocks.topologyReady = true;
	mocks.runtimeReady = true;
	mocks.groupReady = true;
	mocks.showObjectView.mockReset();
	mocks.portableGroups.mockReset();
	mocks.topologyView.mockReset();
	mocks.runtimeProjectionMismatch = false;
	mocks.runtimeCueListIdOverride = null;
	Object.assign(mocks.state, {
		cueListSetTarget: 12,
		cueListSetArmed: true,
		playbackSetArmed: false,
		shiftArmed: false,
		updateArmed: false,
		storeArmed: false,
		blackout: false,
	});
	mocks.playbacks.pages[0].slots = {};
	mocks.playbacks.pool = [];
	mocks.playbacks.active = [];
	mocks.playbacks.selected_playback = null;
}

function playbackDefinition(
	number: number,
	overrides: Record<string, unknown> = {},
) {
	return {
		number,
		name: "Front Wash",
		target: { type: "cue_list", cue_list_id: "front" },
		buttons: ["go", "go_minus", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
		...overrides,
	};
}

function assignPlayback(overrides: Record<string, unknown> = {}) {
	mocks.playbacks.pages[0].slots = { "1": 7 };
	mocks.playbacks.pool = [playbackDefinition(7, overrides)];
	Object.assign(mocks.state, {
		cueListSetTarget: null,
		cueListSetArmed: false,
	});
}

describe("PlaybackFaderBank layout and configuration surfaces", () => {
	beforeEach(resetPlaybackFaderMocks);
	it.each([
		false,
		true,
	])("keeps five configured push encoders above the matrix with hardware=%s", (hardware) => {
		mocks.hardwareConnected = hardware;
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		const positions = [
			...Array.from({ length: 5 }, (_, index) => 301 + index),
			...Array.from({ length: 50 }, (_, index) => 201 + index),
		];
		mocks.playbacks.pages[0].slots = Object.fromEntries(
			positions.map((position) => [String(position), position]),
		);
		mocks.playbacks.pool = positions.map((position) =>
			playbackDefinition(position, {
				name: `Position ${position}`,
				button_count: 1,
				buttons: ["go", "none", "none"],
			}),
		);
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					rows: [
						{
							first_position: 301,
							count: 5,
							presentation: "push_encoder",
							button_count: 1,
						},
						...Array.from({ length: 5 }, (_, index) => ({
							first_position: 201 + index * 10,
							count: 10,
							presentation: "buttons" as const,
							button_count: 1,
						})),
					],
				}}
			/>,
		);
		const cards = [
			...container.querySelectorAll<HTMLElement>("[data-playback-slot]"),
		];
		expect(cards.map((card) => Number(card.dataset.playbackSlot))).toEqual(
			positions,
		);
		expect(container.querySelectorAll(".playback-push-encoder")).toHaveLength(
			5,
		);
		const encoder = within(cards[0]).getByRole("group", { name: "Master" });
		fireEvent.pointerDown(encoder, { pointerId: 73, button: 0, clientY: 100 });
		fireEvent.pointerMove(encoder, { pointerId: 73, clientY: 160 });
		fireEvent.pointerUp(encoder, { pointerId: 73, clientY: 160 });
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
			{ page: 1, position: 301 },
			"master",
			{
				value: expect.any(Number),
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 3010,
				},
			},
		);
		expect(mocks.poolPlaybackAction.mock.calls.at(-1)?.[2].value).toBeLessThan(
			1,
		);
		expect(
			cards
				.slice(5)
				.every((card) => card.querySelector(".touch-encoder") === null),
		).toBe(true);
	});

	it("does not render stale topology while scoped authority is loading", () => {
		assignPlayback();
		mocks.topologyReady = false;
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("status")).toHaveTextContent("Loading Playbacks…");
		expect(
			screen.queryByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		).not.toBeInTheDocument();
	});

	it("rejects a loaded runtime projection that does not match topology", () => {
		assignPlayback();
		mocks.runtimeProjectionMismatch = true;
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Playback runtime authority does not match the visible topology",
		);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("rejects a runtime Cuelist projection with a foreign semantic ID", () => {
		assignPlayback();
		mocks.runtimeCueListIdOverride = "foreign-cuelist";
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Playback runtime authority does not match the visible topology",
		);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("does not rerender for an unrelated parent-context change", () => {
		assignPlayback();
		function Parent({ label }: { label: string }) {
			return (
				<div data-unrelated={label}>
					<PlaybackFaderBank count={1} />
				</div>
			);
		}
		const { rerender } = render(<Parent label="before" />);
		const renders = mocks.topologyView.mock.calls.length;

		rerender(<Parent label="after" />);

		expect(mocks.topologyView).toHaveBeenCalledTimes(renders);
	});

	it("activates the Group collection only for a visible Group playback", () => {
		assignPlayback();
		const { unmount } = render(<PlaybackFaderBank count={1} />);
		expect(mocks.showObjectView).toHaveBeenCalledWith("group", false);
		expect(mocks.portableGroups).toHaveBeenCalledWith(false);
		unmount();
		mocks.showObjectView.mockReset();
		mocks.portableGroups.mockReset();
		assignPlayback({ target: { type: "group", group_id: "group-front" } });
		render(<PlaybackFaderBank count={1} />);

		expect(mocks.showObjectView).toHaveBeenCalledWith("group", true);
		expect(mocks.portableGroups).toHaveBeenCalledWith(true);
	});

	it("projects arbitrary row starts and fills touch height with weighted tracks", () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 1,
							presentation: "buttons" as const,
							button_count: 1,
						},
						{
							first_position: 11,
							count: 1,
							presentation: "buttons" as const,
							button_count: 3,
						},
						{
							first_position: 21,
							count: 1,
							presentation: "fader" as const,
							button_count: 1,
						},
					],
				}}
			/>,
		);

		expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
			gridTemplateRows: "minmax(0, 1fr) minmax(0, 2fr) minmax(0, 4fr)",
		});
		expect(
			[...container.querySelectorAll("[data-playback-slot]")].map((element) =>
				element.getAttribute("data-playback-slot"),
			),
		).toEqual(["1", "11", "21"]);
		expect(
			container.querySelectorAll('[data-ui-component="touch-playback-card"]'),
		).toHaveLength(3);
	});

	it("renders one wider anchor, suppresses the claimed slot, and routes its extra controls", () => {
		assignPlayback({
			footprint: {
				type: "wider",
				right_buttons: ["go", "pause", "flash"],
				right_fader: "x_fade",
			},
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 2,
							presentation: "fader" as const,
							button_count: 3,
						},
					],
				}}
			/>,
		);

		const cards = container.querySelectorAll("[data-playback-slot]");
		expect(cards).toHaveLength(1);
		expect(cards[0]).toHaveAttribute("data-playback-slot", "1");
		expect(cards[0]).toHaveStyle({ gridColumn: "1 / span 2" });
		expect(
			screen.getByLabelText("Right controls for Playback 1"),
		).toBeInTheDocument();
		const extraButton = container.querySelector(
			'[data-playback-button-index="4"]',
		);
		expect(extraButton).not.toBeNull();
		if (extraButton) fireEvent.click(extraButton);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			"button",
			{
				button: 4,
				pressed: true,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 14,
				},
			},
		);
		expect(screen.getAllByRole("slider")).toHaveLength(2);
	});

	it.each([
		1, 2,
	] as const)("limits a wider Playback to its persisted %i-button topology", (buttonCount) => {
		assignPlayback({
			button_count: buttonCount,
			footprint: {
				type: "wider",
				right_buttons: ["go", "pause", "flash"],
				right_fader: "x_fade",
			},
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 2,
							presentation: "fader" as const,
							button_count: 3,
						},
					],
				}}
			/>,
		);

		expect(
			container.querySelectorAll(
				".expanded-playback-controls [data-playback-button-index]",
			),
		).toHaveLength(buttonCount);
		expect(
			container.querySelector('[data-playback-button-index="4"]'),
		).not.toBeNull();
		expect(
			container.querySelector('[data-playback-button-index="6"]'),
		).toBeNull();
	});

	it("fills hardware height with faderless and fader row weights", () => {
		mocks.hardwareConnected = true;
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 1,
							presentation: "buttons" as const,
							button_count: 1,
						},
						{
							first_position: 11,
							count: 1,
							presentation: "buttons" as const,
							button_count: 3,
						},
						{
							first_position: 21,
							count: 1,
							presentation: "fader" as const,
							button_count: 1,
						},
					],
				}}
			/>,
		);
		expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
			gridTemplateRows: "minmax(0, 1fr) minmax(0, 1fr) minmax(0, 2fr)",
		});
		expect(
			container.querySelectorAll(
				'[data-ui-component="hardware-playback-card"]',
			),
		).toHaveLength(3);
	});

	it("assigns the selected Cuelist source to the touched physical page slot", async () => {
		mocks.scopedCueLists[0].pool_number = 12;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Assign Cuelist 12 to page 1 playback 1",
			}),
		);
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				expect.objectContaining({
					target: { type: "cue_list", cue_list_id: "front" },
				}),
				{ expectedPageRevision: 3, expectedPageObjectId: "page-object-1" },
			),
		);
		expect(mocks.executeCommandLine).not.toHaveBeenCalled();
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_ARMED",
			value: false,
		});
	});

	it.each([
		[
			"software representation",
			() =>
				screen.getByRole("button", {
					name: "Playback representation page 1 playback 1",
				}),
		],
		["top button", () => screen.getByRole("button", { name: "GO +" })],
		["middle button", () => screen.getByRole("button", { name: "GO −" })],
		["bottom button", () => screen.getByRole("button", { name: "FLASH" })],
		[
			"fader track and handle",
			() => screen.getByRole("slider", { name: "Master" }),
		],
	])("SET intercepts the %s without executing it and Close is inert", (_surface, target) => {
		assignPlayback();
		mocks.state.playbackSetArmed = true;
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(container.querySelector("article")).toHaveClass(
			"playback-command-target",
			"command-target-set",
		);
		expect(screen.getByText("SET TARGET")).toBeInTheDocument();
		fireEvent.pointerDown(target(), { pointerId: 3 });
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-page", "1");
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-slot", "1");
		fireEvent.click(
			screen.getByRole("button", { name: "Close playback configuration" }),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
	});

	it("opens the same configuration for a command-line ASSIGN PBK request", () => {
		assignPlayback();
		render(<PlaybackFaderBank count={1} />);
		fireEvent(
			window,
			new CustomEvent("light:playback-configuration", {
				detail: { desk_id: "desk-1", address: { page: 1, position: 1 } },
			}),
		);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-slot", "1");
	});

	it("ignores configuration requests for another page or an invisible position", () => {
		assignPlayback();
		render(<PlaybackFaderBank count={1} />);
		for (const address of [
			{ page: 2, position: 1 },
			{ page: 1, position: 401 },
		]) {
			fireEvent(
				window,
				new CustomEvent("light:playback-configuration", {
					detail: { desk_id: "desk-1", address },
				}),
			);
		}
		expect(
			screen.queryByRole("dialog", { name: "Playback Configuration" }),
		).not.toBeInTheDocument();
		expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
	});

	it.each([
		["card label", () => screen.getByText("Front Wash")],
		["top button", () => screen.getByRole("button", { name: "GO +" })],
		["middle button", () => screen.getByRole("button", { name: "GO −" })],
		["bottom button", () => screen.getByRole("button", { name: "FLASH" })],
		[
			"fader track and handle",
			() => screen.getByRole("slider", { name: "Page 1 playback 1 fader" }),
		],
	])("SET intercepts the hardware-layout %s", (_surface, target) => {
		assignPlayback();
		mocks.hardwareConnected = true;
		mocks.state.playbackSetArmed = true;
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(container.querySelector("article")).toHaveClass(
			"playback-command-target",
			"command-target-set",
		);
		expect(screen.getByText("SET TARGET")).toBeInTheDocument();
		fireEvent.pointerDown(target(), { pointerId: 3 });
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-slot", "1");
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it.each([
		["touch", false, "virtual"],
		["hardware-connected", true, "physical"],
	] as const)("OFF makes the entire %s Playback an authoritative Off target", async (_surface, hardwareConnected, actionSurface) => {
		assignPlayback();
		mocks.hardwareConnected = hardwareConnected;
		mocks.commandLine = "OFF";
		const { container } = render(<PlaybackFaderBank count={1} />);

		expect(container.querySelector("article")).toHaveClass(
			"playback-command-target",
			"command-target-off",
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Turn off Front Wash" }),
		);

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"off",
				{
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 10,
					},
				},
			),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("OFF targets only the chosen Playback and is valid when it is already off", async () => {
		mocks.playbacks.pages[0].slots = { "1": 7, "2": 8 };
		mocks.playbacks.pool = [
			playbackDefinition(7),
			playbackDefinition(8, { name: "Neighbor" }),
		];
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		mocks.commandLine = "OFF";
		mocks.poolPlaybackAction.mockResolvedValue({ status: "no_change" });
		render(<PlaybackFaderBank count={2} />);

		fireEvent.click(screen.getByRole("button", { name: "Turn off Neighbor" }));

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 2 },
				"off",
				{
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 20,
					},
				},
			),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalledWith(
			7,
			"off",
			expect.anything(),
		);
		expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("opens an empty slot without fabricating a playback number and allocates a changed draft on Apply", async () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			playbackSetArmed: true,
		});
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-topology", "3 buttons · fader");
		expect(screen.getByRole("radio", { name: "None" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled();
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "New Playback" },
		});
		expect(screen.getByRole("button", { name: "Apply" })).toBeEnabled();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
		expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			expect.objectContaining({
				button_count: 3,
				has_fader: true,
			}),
			{
				expectedPageRevision: 3,
				expectedPageObjectId: "page-object-1",
			},
		);
	});

	it("assigns the armed Group to an empty page slot as a Group Master", async () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			playbackSetArmed: false,
		});
		mocks.commandLine = "ASSIGN GROUP 21";
		render(<PlaybackFaderBank count={1} />);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);

		await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
		expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			expect.objectContaining({
				name: "Group 21",
				target: { type: "group", group_id: "21" },
			}),
			expect.objectContaining({
				expectedPageRevision: 3,
				expectedPageObjectId: "page-object-1",
			}),
		);
		expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("uses the scoped Cuelist authority for an empty slot default", () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			playbackSetArmed: true,
		});
		mocks.playbacks.cue_lists = [
			{ ...mocks.playbacks.cue_lists[0], id: "legacy", name: "Legacy" },
		];
		mocks.scopedCueLists = [
			{
				...mocks.playbacks.cue_lists[0],
				id: "scoped",
				name: "Scoped",
				pool_number: 1,
			},
		];

		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);

		expect(screen.getByRole("radio", { name: "1 Scoped" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(screen.queryByRole("radio", { name: "1 Legacy" })).toBeNull();
	});
});

describe("PlaybackFaderBank configuration shortcuts", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("opens configuration when SHIFT is followed by the first playback button", () => {
		assignPlayback();
		mocks.state.shiftArmed = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("routes Update before playback execution with concrete playback and current Cue context", () => {
		assignPlayback();
		mocks.state.updateArmed = true;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-2",
				number: 2,
				name: "Look",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		const selected = vi.fn();
		const release = registerControlSurfaceTarget({
			id: "update-test",
			priority: 1,
			accepts: ({ type }) => type === "update_target",
			handle: selected,
		});
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		expect(selected.mock.calls[0][0]).toEqual({
			type: "update_target",
			source: "touch",
			target: {
				family: { type: "cue" },
				object_id: "front",
				playback_address: { page: 1, position: 1 },
				cue_id: "cue-2",
				cue_number: 2,
				validate_active_context: true,
			},
		});
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		release();
	});
});

describe("PlaybackFaderBank selection and Record targets", () => {
	beforeEach(resetPlaybackFaderMocks);
	it("uses SELECT then any playback touch without firing the mapped button", async () => {
		assignPlayback();
		mocks.commandLine = "SELECT";
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"select",
				{
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 10,
					},
				},
			),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("makes the hardware card one display-only Cuelist selection surface", async () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-1",
				number: 1,
				name: "Opening",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(
			screen.queryByRole("button", { name: /Playback representation/ }),
		).not.toBeInTheDocument();
		const card = container.querySelector(".hardware-playback-card")!;
		fireEvent.click(card.querySelector("header b")!);
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"select",
				{
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 10,
					},
				},
			),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN_CUELIST",
			number: 1,
		});
	});

	it("separates configured-color running, explicit selection, and empty states", () => {
		assignPlayback({ color: "#f6e58d" });
		mocks.playbacks.active = [
			{
				playback_number: 7,
				enabled: true,
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		mocks.playbacks.selected_playback = 7;
		const { container } = render(<PlaybackFaderBank count={2} />);
		expect(container.querySelector('[data-playback-slot="1"]')).toHaveClass(
			"playback-colored",
			"running",
			"selected",
		);
		expect(container.querySelector('[data-playback-slot="1"]')).toHaveStyle({
			"--playback-color": "#f6e58d",
		});
		expect(container.querySelector('[data-playback-slot="2"]')).toHaveClass(
			"empty",
		);
		expect(container.querySelector('[data-playback-slot="2"]')).not.toHaveClass(
			"playback-colored",
			"running",
			"selected",
		);
	});

	it("does not select the hardware card when a real button or fader operates", () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		fireEvent.input(
			screen.getByRole("slider", { name: "Page 1 playback 1 fader" }),
			{ target: { value: "42" } },
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			"button",
			{
				button: 1,
				pressed: true,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 11,
				},
			},
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			"master",
			{
				value: 0.42,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 10,
				},
			},
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			"select",
			{
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 10,
				},
			},
		);
	});
});

describe("PlaybackFaderBank Record targets", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("makes the entire touch playback area one explicit-page Record target", async () => {
		assignPlayback();
		mocks.hardwareConnected = false;
		mocks.state.storeArmed = true;
		mocks.playbacks.pages.push({
			number: 3,
			name: "Page 3",
			slots: { "1": 7 },
		});
		const { container } = render(
			<PlaybackFaderBank pageNumber={3} count={1} />,
		);
		const card = container.querySelector("article")!;
		expect(card).toHaveClass(
			"store-target",
			"playback-command-target",
			"command-target-record",
		);
		expect(screen.getByText("RECORD TARGET")).toBeInTheDocument();
		const surfaces = [
			screen.getByRole("button", {
				name: "Playback representation page 3 playback 1",
			}),
			screen.getByRole("button", { name: "GO +" }),
			screen.getByRole("button", { name: "GO −" }),
			screen.getByRole("button", { name: "FLASH" }),
			screen.getByRole("slider", { name: "Master" }),
		];
		for (const surface of surfaces) {
			expect(fireEvent.pointerDown(surface, { pointerId: 4 })).toBe(true);
			fireEvent.click(surface);
		}
		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledTimes(surfaces.length),
		);
		for (const call of mocks.recordCue.mock.calls)
			expect(call).toEqual([
				{
					target: { kind: "playback", page: 3, position: 1 },
					operation: "overwrite",
					timing: {},
					cueOnly: false,
					capturePolicy: "current_capture",
					activationPolicy: "go_to_if_normal",
				},
			]);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledTimes(surfaces.length);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
		mocks.playbacks.pages.pop();
	});

	it("intercepts only the attached hardware top button and never its fader", async () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		mocks.state.storeArmed = true;
		mocks.playbacks.pages.push({
			number: 3,
			name: "Page 3",
			slots: { "1": 7 },
		});
		render(<PlaybackFaderBank pageNumber={3} count={1} />);

		fireEvent.click(screen.getByRole("button", { name: "GO −" }));
		fireEvent.click(screen.getByRole("button", { name: "FLASH" }));
		fireEvent.input(
			screen.getByRole("slider", { name: "Page 3 playback 1 fader" }),
			{
				target: { value: "42" },
			},
		);
		expect(mocks.recordCue).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.recordCue).toHaveBeenCalledWith({
			target: { kind: "playback", page: 3, position: 1 },
			operation: "overwrite",
			timing: {},
			cueOnly: false,
			capturePolicy: "current_capture",
			activationPolicy: "go_to_if_normal",
		});
		expect(mocks.poolPlaybackAction).toHaveBeenCalled();
		mocks.playbacks.pages.pop();
	});

	it.each([
		["Add Cue", "overwrite", false],
		["Merge Cue", "merge", true],
		["Overwrite Cue", "overwrite", true],
	] as const)("asks for %s when the target contains exactly one Cue", async (label, operation, exactCue) => {
		assignPlayback();
		mocks.state.storeArmed = true;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-2-0",
				number: "2.0",
				name: "Look",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		expect(
			await screen.findByRole("dialog", { name: "Record Cue choice" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: label }));
		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.recordCue).toHaveBeenCalledWith({
			target: { kind: "playback", page: 1, position: 1 },
			operation,
			...(exactCue ? { cueNumber: "2.0" } : {}),
			timing: {},
			cueOnly: false,
			capturePolicy: "current_capture",
			activationPolicy: "go_to_if_normal",
		});
	});

	it.each([
		["merge", "RECORD ", "merge"],
		["add_existing", "RECORD ", "add_missing"],
		["add_cue", "RECORD ", "add_cue"],
		["merge", "RECORD ADD CUE ", "add_cue"],
		["add_cue", "RECORD SMART ", null],
	] as const)("records with the stored %s default or the line's one-off option (%s)", async (recordDefault, commandLine, operation) => {
		assignPlayback();
		mocks.state.storeArmed = true;
		mocks.recordDefault = recordDefault;
		mocks.commandLine = commandLine;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-2-0",
				number: "2.0",
				name: "Look",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		if (operation === null) {
			expect(
				await screen.findByRole("dialog", { name: "Record Cue choice" }),
			).toBeInTheDocument();
			expect(mocks.recordCue).not.toHaveBeenCalled();
			return;
		}
		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(
			screen.queryByRole("dialog", { name: "Record Cue choice" }),
		).toBeNull();
		expect(mocks.recordCue).toHaveBeenCalledWith(
			expect.objectContaining({
				target: { kind: "playback", page: 1, position: 1 },
				operation,
			}),
		);
		expect(mocks.recordCue.mock.calls[0][0]).not.toHaveProperty("cueNumber");
	});

	it.each([
		["touch", false],
		["hardware-connected", true],
	] as const)("records an empty %s playback card instead of requiring a child control", async (_surface, hardware) => {
		mocks.hardwareConnected = hardware;
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			storeArmed: true,
		});
		const { container } = render(
			<PlaybackFaderBank pageNumber={4} count={1} />,
		);
		fireEvent.click(
			hardware
				? screen.getAllByRole("button", { name: "DISABLED" })[0]
				: container.querySelector("article")!,
		);
		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledWith({
				target: { kind: "playback", page: 4, position: 1 },
				operation: "overwrite",
				timing: {},
				cueOnly: false,
				capturePolicy: "current_capture",
				activationPolicy: "go_to_if_normal",
			}),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("keeps Record armed and the command intact when the typed action fails", async () => {
		mocks.state.storeArmed = true;
		mocks.recordCue.mockResolvedValueOnce(null);
		const { container } = render(
			<PlaybackFaderBank pageNumber={4} count={1} />,
		);

		fireEvent.click(container.querySelector("article")!);
		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.dispatch).not.toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
		expect(mocks.resetCommandLine).not.toHaveBeenCalled();
	});
});

describe("PlaybackFaderBank action dispatch and persistence", () => {
	beforeEach(resetPlaybackFaderMocks);
	it.each([
		false,
		true,
	])("uses the controllable master for software and physical position for hardware (%s)", async (hardware) => {
		assignPlayback();
		mocks.hardwareConnected = hardware;
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				master: 1,
				fader_position: 0,
				fader_pickup_required: true,
				fader_pickup_target: 1,
			},
		];
		render(<PlaybackFaderBank count={1} />);
		const slider = screen.getByRole("slider", {
			name: hardware ? "Page 1 playback 1 fader" : "Master",
		});
		expect(slider).toHaveValue(hardware ? "0" : "100");
		if (hardware) {
			expect(document.querySelector(".hardware-fader")).toHaveAttribute(
				"data-pickup-physical",
				"0",
			);
			expect(document.querySelector(".hardware-fader")).toHaveAttribute(
				"data-pickup-target",
				"1",
			);
		} else {
			expect(slider.closest("label")).toHaveTextContent("100%");
			vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
				top: 100,
				bottom: 500,
				height: 400,
				width: 100,
				left: 0,
				right: 100,
				x: 0,
				y: 100,
				toJSON: () => undefined,
			});
			fireEvent.pointerDown(slider, {
				clientX: 50,
				clientY: 120,
				pointerId: 1,
			});
			fireEvent.pointerUp(slider, { clientX: 50, clientY: 120, pointerId: 1 });
			await waitFor(() =>
				expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
					{ page: 1, position: 1 },
					"master",
					{
						value: 1,
						control_source: {
							surface_id: expect.stringMatching(/^software:/),
							control_id: 10,
						},
					},
				),
			);
		}
	});

	it("commits rapid and slow touch fader changes as absolute values with retained surface source", async () => {
		assignPlayback({ target: { type: "group", group_id: "group-front" } });
		render(<PlaybackFaderBank count={1} />);
		const fader = screen.getByRole("slider", { name: "Group master" });

		fireEvent.input(fader, { target: { value: "87" } });
		fireEvent.input(fader, { target: { value: "13" } });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
				{ page: 1, position: 1 },
				"master",
				{
					value: 0.13,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 10,
					},
				},
			),
		);

		fireEvent.input(fader, { target: { value: "42" } });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
				{ page: 1, position: 1 },
				"master",
				{
					value: 0.42,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 10,
					},
				},
			),
		);
		expect(
			mocks.poolPlaybackAction.mock.calls.every(
				([address, action, options]) =>
					address.page === 1 &&
					address.position === 1 &&
					action === "master" &&
					options.control_source.surface_id ===
						mocks.poolPlaybackAction.mock.calls[0][2].control_source.surface_id,
			),
		).toBe(true);
	});
	it("keeps the zero Group Master warning with the intensity instead of the summary", () => {
		assignPlayback({ target: { type: "group", group_id: "group-front" } });
		mocks.playbacks.active = [{ playback_number: 7, master: 0 }];
		const { container } = render(<PlaybackFaderBank count={1} />);

		expect(screen.getByText("0% master · ▲")).toBeInTheDocument();
		expect(container.querySelector(".playback-summary")).not.toHaveTextContent(
			"0%",
		);
		expect(
			container.querySelector(".vertical-touch-fader > strong"),
		).toHaveTextContent("0% master · ▲");
	});
	it("dispatches the configured index and the concrete held Flash lifetime", async () => {
		assignPlayback();
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass(
			"action-count-3",
		);
		const go = screen.getByRole("button", { name: "GO +" });
		fireEvent.pointerDown(go, { pointerId: 3 });
		expect(go).toHaveClass("playback-button-active");
		fireEvent.click(go);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
			{ page: 1, position: 1 },
			"button",
			{
				button: 1,
				pressed: true,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 11,
				},
			},
		);
		const flash = screen.getByRole("button", { name: "FLASH" });
		fireEvent.pointerDown(flash, { pointerId: 4 });
		fireEvent.pointerUp(flash, { pointerId: 4 });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"flash",
				{
					pressed: true,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 13,
					},
				},
			),
		);
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"flash",
				{
					pressed: false,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 13,
					},
				},
			),
		);
	});

	it.each([
		"cancel",
		"lost capture",
	])("releases a held Flash once after pointer %s", async (release) => {
		assignPlayback();
		render(<PlaybackFaderBank count={1} />);
		const flash = screen.getByRole("button", { name: "FLASH" });
		fireEvent.pointerDown(flash, { pointerId: 4 });
		if (release === "cancel") fireEvent.pointerCancel(flash, { pointerId: 4 });
		else {
			fireEvent.lostPointerCapture(flash, { pointerId: 4 });
			await waitFor(() =>
				expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1),
			);
			fireEvent.pointerUp(window, { pointerId: 4 });
		}
		fireEvent.lostPointerCapture(flash, { pointerId: 4 });

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
			{ page: 1, position: 1 },
			"flash",
			{
				pressed: false,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 13,
				},
			},
		);
	});

	it("keeps two mounted controls of the same configured surface independently held", async () => {
		assignPlayback();
		const held = new Set<string>();
		mocks.poolPlaybackAction.mockImplementation(
			async (_address, action, input) => {
				if (action === "flash") {
					const key = JSON.stringify(input.control_source);
					if (input.pressed) held.add(key);
					else held.delete(key);
				}
				return { status: "changed" };
			},
		);
		const first = render(<PlaybackFaderBank surfaceId="main" count={1} />);
		const second = render(<PlaybackFaderBank surfaceId="main" count={1} />);
		fireEvent(
			within(first.container).getByRole("button", { name: "FLASH" }),
			Object.assign(new Event("pointerdown", { bubbles: true }), {
				pointerId: 4,
			}),
		);
		fireEvent(
			within(second.container).getByRole("button", { name: "FLASH" }),
			Object.assign(new Event("pointerdown", { bubbles: true }), {
				pointerId: 5,
			}),
		);
		await waitFor(() => expect(held.size).toBe(2));
		const presses = mocks.poolPlaybackAction.mock.calls.slice(0, 2);
		expect(presses.map(([address]) => address)).toEqual([
			{ page: 1, position: 1 },
			{ page: 1, position: 1 },
		]);
		expect(presses[0][2].control_source).not.toEqual(
			presses[1][2].control_source,
		);
		first.rerender(
			<PlaybackFaderBank surfaceId="main" pageNumber={2} count={1} />,
		);
		await Promise.resolve();
		expect(held.size).toBe(2);
		expect(
			within(first.container).queryByRole("button", { name: "FLASH" }),
		).not.toBeInTheDocument();
		fireEvent(window, Object.assign(new Event("pointerup"), { pointerId: 99 }));
		await Promise.resolve();
		expect(held.size).toBe(2);
		fireEvent(window, Object.assign(new Event("pointerup"), { pointerId: 4 }));
		await waitFor(() => expect(held.size).toBe(1));
		first.unmount();
		expect(mocks.poolPlaybackAction.mock.calls[2][0]).toEqual({
			page: 1,
			position: 1,
		});
		expect(mocks.poolPlaybackAction.mock.calls[2][2]).toEqual({
			pressed: false,
			control_source: presses[0][2].control_source,
		});
		second.unmount();
		await waitFor(() => expect(held.size).toBe(0));
		expect(mocks.poolPlaybackAction.mock.calls[3][2]).toEqual({
			pressed: false,
			control_source: presses[1][2].control_source,
		});
	});

	it("retains a keyboard hold through an empty page until the actual key release", async () => {
		assignPlayback();
		const bank = render(<PlaybackFaderBank count={1} />);
		fireEvent.keyDown(screen.getByRole("button", { name: "FLASH" }), {
			key: " ",
			repeat: false,
		});
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1),
		);
		const press = mocks.poolPlaybackAction.mock.calls[0];
		bank.rerender(<PlaybackFaderBank pageNumber={2} count={1} />);
		await Promise.resolve();
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		fireEvent.keyUp(window, { key: "Enter" });
		await Promise.resolve();
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		fireEvent.keyUp(window, { key: " " });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
		);
		expect(mocks.poolPlaybackAction.mock.calls[1]).toEqual([
			press[0],
			"flash",
			{ pressed: false, control_source: press[2].control_source },
		]);
	});

	it("releases a held Flash when the bank unmounts", async () => {
		assignPlayback();
		const { unmount } = render(<PlaybackFaderBank count={1} />);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		unmount();

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
			{ page: 1, position: 1 },
			"flash",
			{
				pressed: false,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 13,
				},
			},
		);
	});

	it("orders release after a delayed press and retains its original semantic", async () => {
		assignPlayback();
		let resolvePress!: (value: { status: string }) => void;
		mocks.poolPlaybackAction
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						resolvePress = resolve;
					}),
			)
			.mockResolvedValue({ status: "no_change" });
		const { rerender } = render(<PlaybackFaderBank count={1} />);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"flash",
				{
					pressed: true,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 13,
					},
				},
			),
		);
		mocks.playbacks.pool = [
			playbackDefinition(7, { buttons: ["go", "go_minus", "swap"] }),
		];
		mocks.hardwareConnected = true;
		rerender(<PlaybackFaderBank count={1} />);

		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		fireEvent.pointerUp(window, { pointerId: 4 });
		resolvePress({ status: "changed" });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(
				{ page: 1, position: 1 },
				"flash",
				{
					pressed: false,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 13,
					},
				},
			),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
		expect(screen.getByRole("button", { name: "SWAP" })).toBeInTheDocument();
	});

	it("omits disabled touch buttons while preserving configured button indices", async () => {
		assignPlayback({ buttons: ["go", "none", "flash"], button_count: 3 });
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(
			screen.queryByRole("button", { name: "DISABLED" }),
		).not.toBeInTheDocument();
		expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass(
			"action-count-2",
		);
		expect(
			container.querySelectorAll(".vertical-touch-fader-actions .ui-button"),
		).toHaveLength(2);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				"flash",
				{
					pressed: true,
					control_source: {
						surface_id: expect.stringMatching(/^software:/),
						control_id: 13,
					},
				},
			),
		);
	});

	it("makes one configured faderless touch action fill its playback section", () => {
		assignPlayback({
			buttons: ["flash", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		const { container } = render(<PlaybackFaderBank count={1} />);
		const action = screen.getByRole("button", { name: "FLASH" });
		expect(action).toHaveClass("single-button-playback-action");
		expect(action).toHaveTextContent("1 · Front Wash");
		expect(action).toHaveTextContent("FLASH");
		expect(
			container.querySelector(".faderless-playback-actions"),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "DISABLED" }),
		).not.toBeInTheDocument();
	});

	it("compacts only the one-unit software row so every card fits its track", () => {
		assignPlayback({
			buttons: ["go", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		const { container, rerender } = render(
			<PlaybackFaderBank
				count={1}
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 1,
							presentation: "buttons" as const,
							button_count: 1,
						},
					],
				}}
			/>,
		);
		expect(container.querySelector(".touch-playback-card")).toHaveClass(
			"playback-row-compact",
		);

		rerender(
			<PlaybackFaderBank
				count={1}
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 1,
							presentation: "buttons" as const,
							button_count: 2,
						},
					],
				}}
			/>,
		);
		expect(container.querySelector(".touch-playback-card")).not.toHaveClass(
			"playback-row-compact",
		);
	});

	it.each([
		["two", ["go", "flash", "none"], 2],
		["three", ["go", "go_minus", "flash"], 3],
	] as const)("lays out %s faderless touch buttons side by side", (_count, actions, visibleCount) => {
		assignPlayback({
			buttons: actions,
			button_count: visibleCount,
			has_fader: false,
		});
		const { container } = render(<PlaybackFaderBank count={1} />);
		const actionRow = container.querySelector(".faderless-playback-actions")!;
		expect(actionRow).toHaveStyle({
			"--playback-action-count": String(visibleCount),
		});
		expect(actionRow.querySelectorAll(".ui-button")).toHaveLength(visibleCount);
		expect(
			container.querySelector(".single-button-playback-action"),
		).not.toBeInTheDocument();
	});
});

describe("PlaybackFaderBank faderless controls and runtime feedback", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("dispatches TEMP as a press-to-toggle action on successive clicks", () => {
		assignPlayback({
			buttons: ["temp", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		render(<PlaybackFaderBank count={1} />);
		const temp = screen.getByRole("button", { name: "TEMP" });
		fireEvent.click(temp);
		fireEvent.click(temp);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(
			1,
			{ page: 1, position: 1 },
			"button",
			{
				button: 1,
				pressed: true,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 11,
				},
			},
		);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(
			2,
			{ page: 1, position: 1 },
			"button",
			{
				button: 1,
				pressed: true,
				control_source: {
					surface_id: expect.stringMatching(/^software:/),
					control_id: 11,
				},
			},
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
	});

	it("renders only persisted controls for a one-button faderless playback", () => {
		assignPlayback({
			buttons: ["toggle", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		render(<PlaybackFaderBank count={1} />);
		expect(screen.getByRole("button", { name: "TOGGLE" })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "GO −" }),
		).not.toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("shows X-fade direction/progress without pickup presentation in touch mode", () => {
		assignPlayback({ fader: "x_fade" });
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				current_cue_number: 1,
				effective_next_cue_number: 2,
				enabled: true,
				master: 1,
				flash: false,
				fader_position: 0.25,
				fader_pickup_required: true,
				fader_pickup_target: 0,
				manual_xfade_position: 0.25,
				manual_xfade_direction: "towards_high",
				manual_xfade_progress: 0.25,
			},
		];
		render(<PlaybackFaderBank count={1} />);
		expect(screen.getByText("Travel towards high")).toBeInTheDocument();
		expect(screen.getByText("Cue 1 → 2 · 25%")).toBeInTheDocument();
		expect(screen.queryByText(/Physical/)).not.toBeInTheDocument();
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).not.toBeInTheDocument();
	});

	it("uses the authoritative pickup target only while hardware authority requires it", () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				current_cue_number: 1,
				enabled: false,
				master: 1,
				fader_position: 0.8,
				fader_pickup_required: true,
				fader_pickup_target: 0.1,
			},
		];
		const rendered = render(<PlaybackFaderBank count={1} />);
		const fader = document.querySelector(".hardware-fader");
		expect(fader).toHaveAttribute("data-pickup-physical", "0.8");
		expect(fader).toHaveAttribute("data-pickup-target", "0.1");
		expect(fader).toHaveAttribute("data-pickup-direction", "lower");
		expect(
			screen.getByRole("slider", { name: "Page 1 playback 1 fader" }),
		).toHaveAttribute(
			"aria-description",
			"Physical 80%. Target 10%. Lower to 10%.",
		);
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).toBeInTheDocument();
		expect(document.querySelector("article")).not.toHaveClass(
			"pickup-required",
		);

		mocks.playbacks.active[0] = {
			...mocks.playbacks.active[0],
			fader_position: 0.8,
			fader_pickup_required: false,
			fader_pickup_target: null,
		};
		rendered.rerender(
			<PlaybackFaderBank key="replacement-authority" count={1} />,
		);
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).not.toBeInTheDocument();
		expect(screen.queryByText(/Physical/)).not.toBeInTheDocument();
	});

	it("shows authoritative pickup feedback for a physical Group Master", () => {
		assignPlayback({ target: { type: "group", group_id: "group-front" } });
		mocks.hardwareConnected = true;
		mocks.playbacks.active = [
			{
				playback_number: 7,
				master: 0.75,
				fader_position: 0.2,
				fader_pickup_required: true,
				fader_pickup_target: 0.75,
			},
		];

		render(<PlaybackFaderBank count={1} />);

		const fader = document.querySelector(".hardware-fader");
		expect(fader).toHaveAttribute("data-pickup-physical", "0.2");
		expect(fader).toHaveAttribute("data-pickup-target", "0.75");
		expect(fader).toHaveAttribute("data-pickup-direction", "raise");
	});

	it("routes a playback right-click directly to that card", () => {
		assignPlayback();
		const { container } = render(<PlaybackFaderBank count={1} />);
		fireEvent.contextMenu(container.querySelector("article")!);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
	});

	it("clears atomically through None plus Apply", async () => {
		assignPlayback();
		mocks.state.playbackSetArmed = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.clearPlaybackSlot).toHaveBeenCalledWith(
				{ page: 1, position: 1 },
				{
					expectedPageRevision: 3,
					expectedPageObjectId: "page-object-1",
				},
			),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Playback Configuration" }),
			).not.toBeInTheDocument(),
		);
	});
});

function soloBank(
	initial: readonly PlaybackSoloRegion[] = [],
	presentation: "buttons" | "push_encoder" = "buttons",
) {
	let regions = initial;
	let revision = 1;
	const save = vi.fn<PlaybackSoloTransport["save"]>(
		async (_scope, expectedRevision, next, requestId) => {
			expect(expectedRevision).toBe(revision);
			regions = structuredClone(next);
			revision += 1;
			return {
				showId: "show-1",
				revision,
				regions,
				requestId,
				changed: true,
				replayed: false,
			};
		},
	);
	const transport: PlaybackSoloTransport = {
		loadSnapshot: async () => ({ showId: "show-1", revision, regions }),
		save,
		subscribe: (_scope, observer) => {
			queueMicrotask(() => observer.ready?.());
			return { close: () => {} };
		},
	};
	const view = () => (
		<PlaybackSoloProvider
			authority={{ authorityId: "desk-session", scope: { showId: "show-1" } }}
			transport={transport}
		>
			<PlaybackFaderBank
				pageNumber={mocks.playbacks.active_page}
				count={4}
				playbackLayout={{
					rows: [
						{
							first_position: 1,
							count: 4,
							presentation,
							button_count: 1,
						},
					],
				}}
			/>
		</PlaybackSoloProvider>
	);
	const rendered = render(view());
	return {
		...rendered,
		save,
		regions: () => regions,
		rerenderBank: () => rendered.rerender(view()),
	};
}

describe("shared Playback grid Solo configuration", () => {
	beforeEach(() => {
		resetPlaybackFaderMocks();
		mocks.playbacks.pages = [
			{ number: 1, name: "Main", slots: { "1": 1, "2": 2, "3": 3, "4": 4 } },
			{ number: 2, name: "Other", slots: { "1": 1 } },
		];
		mocks.playbacks.active_page = 1;
		mocks.playbacks.pool = [1, 2, 3, 4].map((position) =>
			playbackDefinition(position, {
				buttons: ["flash", "none", "none"],
				button_count: 1,
				has_fader: false,
			}),
		);
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
	});

	it.each([
		false,
		true,
	])("selects and creates a named region without operating controls with hardware=%s", async (hardware) => {
		mocks.hardwareConnected = hardware;
		const bank = soloBank();
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Select Solo members" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Select Solo members" }),
		);
		for (const position of [1, 2]) {
			const card = bank.container.querySelector<HTMLElement>(
				`[data-playback-slot="${position}"]`,
			)!;
			const target =
				within(card).queryByRole("button", { name: /FLASH/ }) ?? card;
			fireEvent.pointerDown(target, { pointerId: position, button: 0 });
			fireEvent.pointerMove(target, { pointerId: position, clientY: 50 });
			fireEvent.pointerUp(target, { pointerId: position });
			fireEvent.click(target);
		}
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.executeCommandLine).not.toHaveBeenCalled();
		const draftFirst = bank.container.querySelector(
			'[data-playback-slot="1"]',
		)!;
		const draftSecond = bank.container.querySelector(
			'[data-playback-slot="2"]',
		)!;
		expect(draftFirst).toHaveClass(
			"playback-solo-selected",
			"playback-solo-member",
			"solo-fence-top",
		);
		expect(draftFirst).not.toHaveClass("solo-fence-right");
		expect(draftSecond).not.toHaveClass("solo-fence-left");
		fireEvent.change(screen.getByLabelText("Solo Region name"), {
			target: { value: "Grid pair" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Create Solo Region" }));
		await waitFor(() => expect(bank.save).toHaveBeenCalledOnce());
		expect(bank.regions()[0]).toMatchObject({
			name: "Grid pair",
			members: [
				{ page: 1, position: 1 },
				{ page: 1, position: 2 },
			],
		});
		await waitFor(() =>
			expect(
				bank.container.querySelector('[data-playback-slot="1"]'),
			).toHaveClass("playback-solo-member"),
		);
		const first = bank.container.querySelector<HTMLElement>(
			'[data-playback-slot="1"]',
		)!;
		const second = bank.container.querySelector<HTMLElement>(
			'[data-playback-slot="2"]',
		)!;
		expect(first).not.toHaveClass("solo-fence-right");
		expect(second).not.toHaveClass("solo-fence-left");
		expect(first).toHaveClass(
			"solo-fence-top",
			"solo-fence-bottom",
			"solo-fence-left",
		);
		if (!hardware) {
			const flash = within(first).getByRole("button", { name: /FLASH/ });
			fireEvent.pointerDown(flash, { pointerId: 43, button: 0 });
			fireEvent.pointerUp(flash, { pointerId: 43 });
			await waitFor(() =>
				expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(
					{ page: 1, position: 1 },
					"flash",
					{ pressed: false, control_source: expect.any(Object) },
				),
			);
		}
	});

	it("keeps encoder drag, wheel, absolute entry and keyboard gestures inert during configuration selection", async () => {
		mocks.playbacks.pool = [1, 2, 3, 4].map((position) =>
			playbackDefinition(position, {
				buttons: ["go", "none", "none"],
				button_count: 1,
				has_fader: true,
			}),
		);
		const bank = soloBank([], "push_encoder");
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Select Solo members" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Select Solo members" }),
		);
		const card = bank.container.querySelector<HTMLElement>(
			'[data-playback-slot="1"]',
		)!;
		const encoder = within(card).getByRole("group", { name: "Master" });
		fireEvent.pointerDown(encoder, { pointerId: 81, button: 0, clientY: 100 });
		fireEvent.pointerMove(encoder, { pointerId: 81, clientY: 160 });
		fireEvent.pointerUp(encoder, { pointerId: 81 });
		fireEvent.wheel(encoder, { deltaY: 100 });
		fireEvent.keyDown(encoder, { key: "ArrowUp" });
		fireEvent.keyUp(encoder, { key: "ArrowUp" });
		fireEvent.click(
			within(card).getByRole("button", { name: "Set Master value" }),
		);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(card).toHaveClass("playback-solo-selected");
		const second = bank.container.querySelector<HTMLElement>(
			'[data-playback-slot="2"]',
		)!;
		const button = within(second).getByRole("button", { name: "GO +" });
		fireEvent.keyDown(button, { key: " ", code: "Space" });
		fireEvent.keyDown(button, { key: " ", code: "Space", repeat: true });
		fireEvent.keyUp(button, { key: " ", code: "Space" });
		expect(second).toHaveClass("playback-solo-selected");
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Cancel Solo selection" }),
		);
		expect(bank.save).not.toHaveBeenCalled();
		expect(card).not.toHaveClass("playback-solo-selected");
	});

	it("creates one region from draft members selected on different pages", async () => {
		const bank = soloBank();
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Select Solo members" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Select Solo members" }),
		);
		fireEvent.pointerDown(
			bank.container.querySelector('[data-playback-slot="1"]')!,
			{ pointerId: 91, button: 0 },
		);
		mocks.playbacks.active_page = 2;
		bank.rerenderBank();
		fireEvent.pointerDown(
			bank.container.querySelector('[data-playback-slot="1"]')!,
			{ pointerId: 92, button: 0 },
		);
		expect(screen.getByText(/2 selected across pages/)).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("Solo Region name"), {
			target: { value: "Two pages" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Create Solo Region" }));
		await waitFor(() => expect(bank.save).toHaveBeenCalledOnce());
		expect(bank.regions()[0].members).toEqual([
			{ page: 1, position: 1 },
			{ page: 2, position: 1 },
		]);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("retains disconnected islands and cross-page membership through page changes and general editing", async () => {
		const bank = soloBank([
			{
				id: "region-a",
				name: "Across pages",
				members: [
					{ page: 1, position: 1 },
					{ page: 1, position: 2 },
					{ page: 1, position: 4 },
					{ page: 2, position: 1 },
				],
			},
		]);
		await waitFor(() =>
			expect(
				bank.container.querySelector('[data-playback-slot="4"]'),
			).toHaveClass("playback-solo-member"),
		);
		expect(
			bank.container.querySelector('[data-playback-slot="4"]'),
		).toHaveClass(
			"solo-fence-top",
			"solo-fence-right",
			"solo-fence-bottom",
			"solo-fence-left",
		);
		mocks.playbacks.active_page = 2;
		bank.rerenderBank();
		await waitFor(() =>
			expect(
				bank.container.querySelector('[data-playback-slot="1"]'),
			).toHaveClass("playback-solo-member"),
		);
		expect(bank.regions()[0].members).toHaveLength(4);
		expect(bank.save).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Manage Solo Regions" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Across pages" }));
		fireEvent.change(screen.getByLabelText("Members (page.position)"), {
			target: { value: "1.1, 2.1" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save region" }));
		await waitFor(() =>
			expect(bank.regions()[0].members).toEqual([
				{ page: 1, position: 1 },
				{ page: 2, position: 1 },
			]),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});
});
