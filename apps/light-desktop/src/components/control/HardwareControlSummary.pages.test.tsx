import {
	act,
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SpeedGroupId, SpeedGroupSoundState } from "../../api/types";
import { HardwareControlSummary } from "./HardwareControlSummary";

const render = (ui: Parameters<typeof rtlRender>[0]) => rtlRender(ui);
const renderWithModals = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

const dispatch = vi.fn();
const state = {
	playbackPage: 0,
	playbackSetArmed: false,
	shiftArmed: false,
};
let playbackDesk: { active_page: number } | null = { active_page: 1 };
let runtimeStatus: "ready" | "loading" | "error" = "ready";
let topologyReady = true;
let pageObjects = [
	{
		kind: "playback_page" as const,
		id: "legacy-page-one",
		revision: 3,
		updated_at: "",
		body: { number: 1, name: "Main", slots: {} },
	},
];
const topologyActions = {
	createPage: vi.fn(async (): Promise<object | null> => ({})),
	renamePage: vi.fn(async (): Promise<object | null> => ({})),
	error: null as Error | null,
};
const runtimeActions = { setActivePage: vi.fn(async () => true) };
const server = {
	session: null as { session_id: string; desk: { id: string } } | null,
	playbacks: {
		active_page: 9,
		pages: [{ number: 9, name: "Stale", slots: {} }],
	},
	configuration: {
		speed_groups_bpm: [120, 90, 60, 30, 15],
		programmer_fade_millis: 3_000,
		sequence_master_fade_millis: 3_000,
	},
	setControlTiming: vi.fn(),
	highlightError: null,
	dismissHighlightError: vi.fn(),
	speedGroup: vi.fn(),
	updateSpeedGroup: vi.fn(),
	observeSpeedGroup: vi.fn(),
	speedGroupAction: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state,
		dispatch: (action: { type: string; value?: boolean }) => {
			if (action.type === "SET_PLAYBACK_SET_ARMED")
				state.playbackSetArmed = Boolean(action.value);
			if (action.type === "SET_SHIFT_ARMED")
				state.shiftArmed = Boolean(action.value);
			dispatch(action);
		},
	}),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock(
	"../../features/soundToLight/SoundToLightContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useSoundToLightActions: () => ({
			speedGroup: server.speedGroup,
			updateSpeedGroup: server.updateSpeedGroup,
			observeSpeedGroup: server.observeSpeedGroup,
			speedGroupAction: server.speedGroupAction,
		}),
	}),
);
vi.mock(
	"../../features/deskSnapshot/DeskSnapshotState",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useSessionSnapshot: () => server.session,
	}),
);
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: () => playbackDesk,
	usePlaybackRuntimeActions: () => runtimeActions,
	usePlaybackRuntimeStatus: () => ({ status: runtimeStatus, error: null }),
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => topologyActions,
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => ({
		ready: topologyReady,
		error: null,
		pages: topologyReady ? pageObjects : [],
	}),
}));

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	server.session = null;
	state.playbackSetArmed = false;
	state.shiftArmed = false;
	playbackDesk = { active_page: 1 };
	runtimeStatus = "ready";
	topologyReady = true;
	topologyActions.error = null;
	pageObjects = [
		{
			kind: "playback_page",
			id: "legacy-page-one",
			revision: 3,
			updated_at: "",
			body: { number: 1, name: "Main", slots: {} },
		},
	];
	vi.clearAllMocks();
});

function soundState(group: SpeedGroupId): SpeedGroupSoundState {
	const bpm = server.configuration.speed_groups_bpm[group.charCodeAt(0) - 65];
	return {
		group,
		configuration: {
			enabled: false,
			analysis_mode: "tempo_bpm",
			frequency: { type: "preset", preset: "low" },
			input_gain_db: 0,
			confidence_threshold: 0.65,
			smoothing: 0.35,
			minimum_bpm: 40,
			maximum_bpm: 240,
			signal_hold_millis: 2_000,
			multiplier: 1,
		},
		snapshot: {
			manual_bpm: bpm,
			sound_bpm: null,
			effective_bpm: bpm,
			source: "manual",
			sound_status: { state: "disabled" },
			paused: false,
			phase_advancing: true,
			speed_master_scale: 1,
			sound_multiplier: 1,
			source_available: false,
			usable_signal: false,
			input_level: 0,
			selected_band_level: 0,
		},
	};
}

describe("HardwareControlSummary playback pages", () => {
	it("offers Add new page from the hardware-connected page menu", async () => {
		render(<HardwareControlSummary />);
		fireEvent.click(screen.getByRole("button", { name: "Page 1" }));
		fireEvent.click(
			within(screen.getByRole("dialog", { name: "Playback pages" })).getByRole(
				"button",
				{ name: "Add new page" },
			),
		);
		await waitFor(() =>
			expect(topologyActions.createPage).toHaveBeenCalledWith(2),
		);
		expect(runtimeActions.setActivePage).toHaveBeenCalledWith(2);
	});

	it("uses SET then Page to rename instead of opening the page menu", () => {
		state.playbackSetArmed = true;
		render(<HardwareControlSummary />);
		fireEvent.click(screen.getByRole("button", { name: "Page 1" }));
		expect(
			screen.getByRole("dialog", { name: "Rename playback page 1" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("dialog", { name: "Playback pages" }),
		).not.toBeInTheDocument();
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PLAYBACK_SET_ARMED",
			value: false,
		});
	});

	it("renders the scoped Playback desk page instead of stale bootstrap state", () => {
		playbackDesk = { active_page: 2 };
		pageObjects = [
			...pageObjects,
			{
				...pageObjects[0],
				id: "page-two",
				body: { number: 2, name: "Second", slots: {} },
			},
		];
		render(<HardwareControlSummary />);

		expect(screen.getByRole("button", { name: "Page 2" })).toBeVisible();
		expect(
			screen.queryByRole("button", { name: "Page 1" }),
		).not.toBeInTheDocument();
	});

	it("hides a retained desk Page while runtime authority repairs", () => {
		runtimeStatus = "loading";
		playbackDesk = { active_page: 1 };

		render(<HardwareControlSummary />);

		expect(
			screen.getByRole("button", { name: "Playback page loading" }),
		).toBeDisabled();
		expect(screen.queryByRole("button", { name: "Page 1" })).toBeNull();
	});

	it("shows explicit loading and disables an open menu during runtime repair", () => {
		const view = render(<HardwareControlSummary />);
		fireEvent.click(screen.getByRole("button", { name: "Page 1" }));
		const pages = screen.getByRole("dialog", { name: "Playback pages" });

		runtimeStatus = "loading";
		view.rerender(<HardwareControlSummary />);

		expect(within(pages).getByRole("status")).toHaveTextContent(
			"Loading Playback pages…",
		);
		expect(
			within(pages).getByRole("button", { name: "Add new page" }),
		).toBeDisabled();
		expect(within(pages).getByRole("button", { name: /Main/ })).toBeDisabled();
	});

	it("renders loading instead of desk or bootstrap state before Page authority", () => {
		topologyReady = false;

		render(<HardwareControlSummary />);

		expect(
			screen.getByRole("button", { name: "Playback page loading" }),
		).toBeDisabled();
		expect(
			screen.queryByRole("button", { name: "Page 9" }),
		).not.toBeInTheDocument();
	});
});

describe("HardwareControlSummary Speed Groups", () => {
	it("uses the authoritative Learn action for an ordinary touch activation", async () => {
		server.session = { session_id: "session-a", desk: { id: "desk-a" } };
		server.speedGroup.mockImplementation(async (group: SpeedGroupId) =>
			soundState(group),
		);
		server.speedGroupAction.mockImplementation(async (group: SpeedGroupId) =>
			soundState(group),
		);
		renderWithModals(<HardwareControlSummary />);
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));

		fireEvent.click(
			screen.getByRole("button", { name: "Speed group A, 120.0 BPM" }),
		);

		await waitFor(() =>
			expect(server.speedGroupAction).toHaveBeenCalledWith(
				"A",
				expect.objectContaining({
					action: "learn",
					captured_at_millis: expect.any(Number),
				}),
			),
		);
		expect(server.setControlTiming).not.toHaveBeenCalled();
	});

	it("opens settings through Shift, hold, and the marked right-click without Learn", async () => {
		server.session = { session_id: "session-a", desk: { id: "desk-a" } };
		server.speedGroup.mockImplementation(async (group: SpeedGroupId) =>
			soundState(group),
		);
		renderWithModals(<HardwareControlSummary />);
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));
		const speedGroup = screen.getByRole("button", {
			name: "Speed group A, 120.0 BPM",
		});

		fireEvent.click(speedGroup, { shiftKey: true });
		expect(
			screen.getByRole("dialog", { name: "Speed Group A Sound to Light" }),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Close Speed Group settings" }),
		);

		vi.useFakeTimers();
		fireEvent.pointerDown(speedGroup);
		act(() => vi.advanceTimersByTime(650));
		fireEvent.pointerUp(speedGroup);
		fireEvent.click(speedGroup);
		expect(
			screen.getByRole("dialog", { name: "Speed Group A Sound to Light" }),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Close Speed Group settings" }),
		);
		vi.useRealTimers();

		fireEvent.contextMenu(speedGroup);

		expect(
			screen.getByRole("dialog", { name: "Speed Group A Sound to Light" }),
		).toBeInTheDocument();
		expect(server.speedGroupAction).not.toHaveBeenCalled();
	});
});
