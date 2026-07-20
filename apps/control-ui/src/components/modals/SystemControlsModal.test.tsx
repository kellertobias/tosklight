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
const scopedValues = vi.hoisted(() => ({ count: 3 as number | null }));
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
vi.mock("../../features/programmerValues/useProgrammerValuesActivity", () => ({
	useNormalProgrammerValueCount: () => scopedValues.count,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	scopedValues.count = 3;
	server.bootstrap.active_programmers.splice(1);
});

describe("SystemControlsModal", () => {
	it("shows every running source and stops each one from the modal", () => {
		render(<SystemControlsModal />);

		expect(screen.getByText("Main playback")).toBeInTheDocument();
		expect(screen.getByText("Virtual Cuelist")).toBeInTheDocument();
		expect(screen.getByText("Operator · Current user")).toBeInTheDocument();
		expect(screen.getByText("Main Cuelist · Dynamic 1")).toBeInTheDocument();
		expect(
			screen.getByText("1 fixtures · 3 values · Connected"),
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

	it("uses scoped values for every current-user desk and legacy values for foreign users", () => {
		server.bootstrap.active_programmers.push(
			{
				session_id: "session-2",
				user_id: "operator",
				selected: [],
				values: [],
				group_values: {},
				connected: false,
			},
			{
				session_id: "session-3",
				user_id: "other-user",
				selected: [],
				values: [{}, {}],
				group_values: { rear: { intensity: {} } },
				connected: true,
			},
		);

		render(<SystemControlsModal />);

		expect(screen.getAllByText(/3 values/)).toHaveLength(3);
		expect(
			screen.getByText("0 fixtures · 3 values · Disconnected"),
		).toBeInTheDocument();
		expect(
			screen.getByText("0 fixtures · 3 values · Connected"),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer other-user" }),
		);
		expect(clearProgrammer).toHaveBeenCalledWith("session-3");
	});

	it("never falls back to stale current-user bootstrap values while loading", () => {
		scopedValues.count = null;
		render(<SystemControlsModal />);

		expect(
			screen.getByText("1 fixtures · Values loading… · Connected"),
		).toBeInTheDocument();
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
