import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SystemControlsModal } from "./SystemControlsModal";

const dispatch = vi.fn();
const playbackAction = vi.fn().mockResolvedValue(undefined);
const clearProgrammer = vi.fn().mockResolvedValue(undefined);
const lifecycle = vi.hoisted(() => ({
	projection: {
		revision: 1,
		programmers: [
			{
				programmerId: "programmer-1",
				userId: "operator",
				connected: true,
				selectedFixtureCount: 1,
				normalValueCount: 3,
				sessions: [{ sessionId: "session-1" }],
			},
		],
	} as {
		revision: number;
		programmers: Array<{
			programmerId: string;
			userId: string;
			connected: boolean;
			selectedFixtureCount: number;
			normalValueCount: number;
			sessions: Array<{ sessionId: string }>;
		}>;
	} | null,
}));
const server = {
	readVisualization: vi
		.fn()
		.mockResolvedValue({ grand_master: 1, blackout: false }),
	setMaster: vi.fn(),
	setProgrammer: vi.fn(),
	selectedFixtures: [],
	patch: { fixtures: [] },
	session: { user: { id: "operator", name: "Operator" } },
	bootstrap: {
		active_programmers: [
			{
				session_id: "session-1",
				user_id: "operator",
				selected: ["fixture-1"],
				values: [{}],
				group_values: { front: { intensity: {} } },
				connected: true,
			},
		] as Array<{
			session_id: string;
			user_id: string;
			selected: string[];
			values: unknown[];
			group_values: Record<string, Record<string, unknown>>;
			connected: boolean;
		}>,
	},
	playbacks: {
		active: [
			{
				playback_number: 12,
				cue_list_id: "cue-list-1",
				cue_index: 0,
				paused: false,
				master: 0.75,
				flash: false,
			},
			{
				playback_number: null,
				cue_list_id: "cue-list-2",
				cue_index: 0,
				paused: true,
				master: 1,
				flash: false,
			},
		],
		pool: [{ number: 12, name: "Main playback" }],
		cue_lists: [
			{
				id: "cue-list-1",
				name: "Main Cuelist",
				cues: [{ number: 1, phasers: [{}] }],
			},
			{
				id: "cue-list-2",
				name: "Virtual Cuelist",
				cues: [{ number: 3, phasers: [] }],
			},
		],
	},
	playbackAction,
	clearProgrammer,
	preloadAction: vi.fn().mockResolvedValue(undefined),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: { systemControlsOpen: true }, dispatch }),
}));
vi.mock("../../features/programmerLifecycle/ProgrammerLifecycleView", () => ({
	useProgrammerLifecycleView: () => lifecycle.projection,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	lifecycle.projection = {
		revision: 1,
		programmers: [
			{
				programmerId: "programmer-1",
				userId: "operator",
				connected: true,
				selectedFixtureCount: 1,
				normalValueCount: 3,
				sessions: [{ sessionId: "session-1" }],
			},
		],
	};
});

describe("SystemControlsModal", () => {
	it("shows every running source and stops each one from the modal", () => {
		render(<SystemControlsModal />);

		expect(screen.getByText("Main playback")).toBeInTheDocument();
		expect(screen.getByText("Virtual Cuelist")).toBeInTheDocument();
		expect(screen.getByText("Operator · Current user")).toBeInTheDocument();
		expect(screen.getByText("Main Cuelist · Dynamic 1")).toBeInTheDocument();
		expect(
			screen.getByText("1 fixtures · 3 values · 1 session · Connected"),
		).toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", { name: "Stop Playback Main playback" }),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Stop Virtual playback Virtual Cuelist",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer operator" }),
		);

		expect(playbackAction).toHaveBeenCalledWith("cue-list-1", "release");
		expect(playbackAction).toHaveBeenCalledWith("cue-list-2", "release");
		expect(clearProgrammer).toHaveBeenCalledWith("session-1");
	});

	it("groups same-user desks and uses safe lifecycle counts for foreign users", () => {
		lifecycle.projection?.programmers[0].sessions.push({
			sessionId: "session-2",
		});
		lifecycle.projection?.programmers.push({
			programmerId: "programmer-2",
			userId: "other-user",
			connected: true,
			selectedFixtureCount: 0,
			normalValueCount: 3,
			sessions: [{ sessionId: "session-3" }],
		});

		render(<SystemControlsModal />);

		expect(screen.getAllByText(/3 values/)).toHaveLength(2);
		expect(
			screen.getByText("1 fixtures · 3 values · 2 sessions · Connected"),
		).toBeInTheDocument();
		expect(
			screen.getByText("0 fixtures · 3 values · 1 session · Connected"),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer other-user" }),
		);
		expect(clearProgrammer).toHaveBeenCalledWith("session-3");
	});

	it("never falls back to stale bootstrap Programmers while loading", () => {
		lifecycle.projection = null;
		render(<SystemControlsModal />);

		expect(screen.getByText("Programmers loading…")).toBeInTheDocument();
		expect(screen.queryByText(/2 values/)).not.toBeInTheDocument();
	});

	it("stops all playback and programmer sources together", async () => {
		render(<SystemControlsModal />);
		fireEvent.click(screen.getByRole("button", { name: "Stop everything" }));

		await waitFor(() => expect(playbackAction).toHaveBeenCalledTimes(2));
		expect(clearProgrammer).toHaveBeenCalledWith("session-1");
		expect(server.preloadAction).toHaveBeenCalledWith("release");
		expect(dispatch).toHaveBeenCalledWith({ type: "RELEASE_PRELOAD" });
	});
});
