import {
	fireEvent,
	render,
	screen,
	waitFor,
	cleanup,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FixtureLibrarySetup } from "./FixtureLibrarySetup";
import { blankFixtureProfile } from "./fixtureProfileModel";
import { FileApiClient } from "../../api/client/files";
import { FixtureApiClient } from "../../api/client/fixtures";
import { ServerRuntime } from "../../api/ServerRuntime";
const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const SESSION_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const scenario = vi.hoisted(() => ({ status: "connected" }));
function profile() {
	const p = blankFixtureProfile();
	p.id = "ffffffff-ffff-4fff-8fff-ffffffffffff";
	p.revision = 1;
	p.manufacturer = "ROBE";
	p.name = "DLS";
	p.short_name = "DLS";
	return p;
}
vi.mock("../../features/server/useServerPolling", () => ({
	useServerPolling: vi.fn(),
}));
vi.mock("../../features/server/useShowData", () => ({
	useShowObjects: () => vi.fn().mockResolvedValue(undefined),
	useServerRefresh: () => vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../../features/server/useServerConnection", async () => {
	const { useEffect } = await import("react");
	return {
		useServerConnection: (state: {
			setBootstrap(value: unknown): void;
			setSession(value: unknown): void;
			setConnectionGeneration(value: number): void;
			setStatus(value: string): void;
			setFixtureProfiles(value: unknown[]): void;
		}) => {
			useEffect(() => {
				const user = { id: SESSION_ID, name: "Operator", enabled: true };
				const desk = {
					id: DESK_ID,
					name: "Main",
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
					session_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
					client_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
					token: "session-token",
					user,
					desk,
				});
				state.setConnectionGeneration(1);
				state.setStatus(scenario.status);
				state.setFixtureProfiles([profile()]);
			}, []);
		},
	};
});
vi.mock("../../api/useServerFeatureBoundaries", () => ({
	useServerFeatureBoundaries: () => ({
		showObjectsTransport: null,
		playbackTransport: null,
		programmingTransport: null,
		programmerValuesTransport: null,
		programmerCaptureModeTransport: {
			subscribe: vi.fn(),
		},
		programmerValuesAuthorityKey: "server-session-a",
		programmerCaptureModeAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: vi.fn(),
		loadProgrammingInteractionSnapshot: vi.fn(),
		loadProgrammerValuesSnapshot: vi.fn(),
		loadProgrammerCaptureModeSnapshot: vi.fn(),
		applyProgrammerValuesAction: vi.fn(),
		loadShowObjectCollection: vi.fn(),
		loadShowObject: vi.fn(),
		reportShowObjectError: vi.fn(),
		reportPlaybackError: vi.fn(),
		reportProgrammingSessionError: vi.fn(),
		reportProgrammingMutationError: vi.fn(),
		reportProgrammerValuesSessionError: vi.fn(),
		reportProgrammerValuesMutationError: vi.fn(),
		reportProgrammerCaptureModeSessionError: vi.fn(),
	}),
}));

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});
beforeEach(() => {
	scenario.status = "connected";
	vi.spyOn(FileApiClient.prototype, "fileRoots").mockResolvedValue([
		{
			id: "shows",
			label: "Shows",
			writable: true,
			icon: "folder",
			removable: false,
		},
	]);
	vi.spyOn(FileApiClient.prototype, "fileEntries").mockResolvedValue({
		root_id: "shows",
		path: "",
		entries: [],
	});
});
async function exportFromRuntime(role: "primary" | "secondary" = "primary") {
	render(
		<ModalProvider>
			<ServerRuntime sessionRole={role}>
				<FixtureLibrarySetup />
			</ServerRuntime>
		</ModalProvider>,
	);
	fireEvent.click(
		await screen.findByRole("button", { name: "Export fixture" }),
	);
	await waitFor(() =>
		expect(
			screen.getByRole("button", { name: "Save fixture package" }),
		).toBeEnabled(),
	);
	fireEvent.click(screen.getByRole("button", { name: "Save fixture package" }));
}
describe("native scoped fixture export authority", () => {
	it("forwards the embedded connected Operator export through the real runtime provider", async () => {
		const save = vi
			.spyOn(FixtureApiClient.prototype, "exportFixturePackageFile")
			.mockResolvedValue({
				request_id: "save",
				replayed: false,
				profile_id: profile().id,
				revision: 1,
				root_id: "shows",
				path: "robe-dls-1.toskfixture",
			});
		await exportFromRuntime();
		await waitFor(() =>
			expect(save).toHaveBeenCalledWith(profile().id, 1, {
				root_id: "shows",
				path: "",
				name: "robe-dls-1.toskfixture",
			}),
		);
		expect(
			await screen.findByText("Saved Shows / robe-dls-1.toskfixture"),
		).toBeInTheDocument();
	});
	it.each([
		["read-only", "secondary", "connected", "Forbidden: read-only session"],
		[
			"disconnected",
			"primary",
			"disconnected",
			"Desk connection is unavailable",
		],
	] as const)("keeps transport %s refusal local without reporting a saved file", async (_case, role, status, message) => {
		scenario.status = status;
		const save = vi
			.spyOn(FixtureApiClient.prototype, "exportFixturePackageFile")
			.mockRejectedValue(new Error(message));
		await exportFromRuntime(role);
		expect(await screen.findByRole("alert")).toHaveTextContent(message);
		expect(save).toHaveBeenCalledOnce();
		expect(screen.queryByText(/Saved Shows/)).not.toBeInTheDocument();
		expect(
			screen.getByRole("dialog", { name: "Export fixture" }),
		).toBeInTheDocument();
	});
});
