import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HardwareControlSummary } from "./HardwareControlSummary";

const dispatch = vi.fn();
const state = { playbackPage: 0, playbackSetArmed: false };
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
};

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state,
		dispatch: (action: { type: string; value?: boolean }) => {
			if (action.type === "SET_PLAYBACK_SET_ARMED")
				state.playbackSetArmed = Boolean(action.value);
			dispatch(action);
		},
	}),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
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
	state.playbackSetArmed = false;
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
		expect(
			within(pages).getByRole("button", { name: /Main/ }),
		).toBeDisabled();
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
