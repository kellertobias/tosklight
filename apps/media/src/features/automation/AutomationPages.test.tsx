import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { automationApi, type AutomationView, type Playlist } from "../../shared/api/automation";
import { resetResources } from "../../shared/api/resource";
import { sendTelemetryAction } from "../../shared/api/telemetry";
import { emptyPlaylist, PlaylistsPage } from "./PlaylistsPage";
import { SchedulePage } from "./SchedulePage";

vi.mock("../../shared/api/telemetry", () => ({ useTelemetry: () => ({ frame: undefined, connected: true }), sendTelemetryAction: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../../shared/api/queries", () => ({
	useCatalog: () => ({ data: { revision: 1, itemCount: 2, folders: [{ folder: 1, name: "Marketing", items: [{ id: "asset-image", file: 1, name: "Opening hours", kind: "image", enabled: true, width: 1920, height: 1080 }, { id: "asset-video", file: 2, name: "Venue film", kind: "video", enabled: true, width: 1920, height: 1080 }] }] } }),
	useOutputs: () => ({ data: [{ id: "main", name: "Main", layerCount: 8 }] }),
	useEffects: () => ({ data: [] }), useModels: () => ({ data: [] }),
}));

let view: AutomationView;
const playlist = (): Playlist => ({ ...emptyPlaylist(), id: "playlist-1", name: "Lobby", enabled: true, entries: [
	{ id: "entry-1", assetId: "asset-image", plays: 1, holdMillis: 10000, fadeMillis: 1000 },
	{ id: "entry-2", assetId: "asset-video", plays: 2, holdMillis: null, fadeMillis: 500 },
] });
function mount(page: React.ReactNode) { return render(<ModalProvider><ToastProvider>{page}</ToastProvider></ModalProvider>); }

beforeEach(() => {
	resetResources();
	view = { automationRevision: 1, configuration: { playlists: [playlist()], events: [], outputs: [{ outputId: "main", enabled: false, fallbackPlaylistId: null, dmxSilenceSeconds: 10 }] }, timeZone: "Europe/Berlin", status: [{ outputId: "main", owner: "disabled", playlistId: null, eventId: null, playlistName: null, entryIndex: null, play: 0, phase: "idle", error: null, dmxAgeMillis: null, resumeInMillis: null, lastOccurrenceOutcome: null, lastOccurrenceName: null }], upcoming: [] };
	vi.spyOn(automationApi, "read").mockImplementation(async () => structuredClone(view));
	vi.spyOn(automationApi, "updatePlaylist").mockImplementation(async (id, change) => { Object.assign(view.configuration.playlists.find((playlist) => playlist.id === id)!, change); return structuredClone(view); });
	vi.spyOn(automationApi, "updateOutput").mockImplementation(async (id, change) => { Object.assign(view.configuration.outputs.find((output) => output.outputId === id)!, change); return structuredClone(view); });
	vi.spyOn(automationApi, "createEvent").mockImplementation(async (event) => { view.configuration.events.push({ ...event, id: "event-new" }); return structuredClone(view); });
});
afterEach(() => { vi.restoreAllMocks(); vi.mocked(sendTelemetryAction).mockClear(); });

describe("Pixel playlist editing", () => {
	it("keeps invalid edits visibly unsaved and stores all corrections together", async () => {
		mount(<PlaylistsPage />);
		await screen.findByRole("article", { name: "Edit Lobby" });
		fireEvent.change(screen.getByLabelText("Playlist name"), { target: { value: "" } });
		fireEvent.change(screen.getByLabelText("Priority"), { target: { value: "7" } });
		expect(screen.getByText(/Changes are not saved yet/)).toBeVisible();
		expect(automationApi.updatePlaylist).not.toHaveBeenCalled();
		fireEvent.change(screen.getByLabelText("Playlist name"), { target: { value: "Evening lobby" } });
		await waitFor(() => expect(automationApi.updatePlaylist).toHaveBeenCalledWith("playlist-1", { name: "Evening lobby", priority: 7 }));
		await waitFor(() => expect(screen.queryByText(/Changes are not saved yet/)).not.toBeInTheDocument());
	});

	it("reorders stable clip identities and persists plays and transition duration", async () => {
		mount(<PlaylistsPage />);
		await screen.findByRole("article", { name: "Edit Lobby" });
		await userEvent.click(screen.getByRole("button", { name: "Move clip 1 down" }));
		await waitFor(() => expect(view.configuration.playlists[0].entries.map((entry) => entry.id)).toEqual(["entry-2", "entry-1"]));
		fireEvent.change(screen.getByLabelText("Clip 1 plays"), { target: { value: "3" } });
		fireEvent.change(screen.getByLabelText("Clip 1 fade to next (s)"), { target: { value: "2" } });
		await waitFor(() => expect(view.configuration.playlists[0].entries[0]).toMatchObject({ id: "entry-2", plays: 3, fadeMillis: 2000 }));
		expect(automationApi.updatePlaylist).toHaveBeenLastCalledWith("playlist-1", expect.objectContaining({ entries: expect.any(Array) }));
	});

	it("flushes a pending image duration edit when navigating away", async () => {
		const mounted = mount(<PlaylistsPage />);
		await screen.findByLabelText("Clip 1 image duration (s)");
		fireEvent.change(screen.getByLabelText("Clip 1 image duration (s)"), { target: { value: "7.5" } });
		mounted.unmount();
		await waitFor(() => expect(view.configuration.playlists[0].entries[0].holdMillis).toBe(7500));
	});

	it("excludes the other playback layer and configures a fixed auxiliary layer", async () => {
		mount(<PlaylistsPage />);
		await screen.findByRole("article", { name: "Edit Lobby" });
		await userEvent.click(screen.getByRole("button", { name: "Playback layer A" }));
		expect(screen.getByRole("option", { name: "Layer 3" })).toBeDisabled();
		await userEvent.click(screen.getByRole("option", { name: "Layer 4" }));
		await waitFor(() => expect(view.configuration.playlists[0].layers).toEqual([4, 3]));
		await userEvent.click(screen.getByRole("button", { name: "Layer 1 behavior" }));
		await userEvent.click(screen.getByRole("option", { name: "Fixed content" }));
		const layer = screen.getByRole("generic", { name: "Layer 1 settings" });
		await userEvent.click(within(layer).getByRole("button", { name: "Layer 1 content" }));
		await userEvent.click(screen.getByRole("option", { name: "001 · Opening hours" }));
		fireEvent.change(within(layer).getByLabelText("Dimmer"), { target: { value: "0.6" } });
		await waitFor(() => expect(view.configuration.playlists[0].auxiliaryLayers[0]).toMatchObject({ layer: 1, mode: "fixed", controls: { dimmer: 0.6 } }));
	});
});

describe("Pixel calendar scheduling", () => {
	it("creates a weekly overnight window without confusing validity dates with looping", async () => {
		mount(<SchedulePage />);
		await screen.findByRole("heading", { name: "Automatic playback" });
		await userEvent.click(screen.getByRole("button", { name: "New event" }));
		await userEvent.click(screen.getByRole("button", { name: "Event type" }));
		await userEvent.click(screen.getByRole("option", { name: "Weekly" }));
		fireEvent.change(screen.getByLabelText("Start time"), { target: { value: "22:00" } });
		fireEvent.change(screen.getByLabelText("First date (optional)"), { target: { value: "2026-11-23" } });
		await userEvent.click(screen.getByLabelText("Loop until an end time"));
		fireEvent.change(screen.getByLabelText("End time"), { target: { value: "02:00" } });
		fireEvent.change(screen.getByLabelText("End day offset"), { target: { value: "1" } });
		await userEvent.click(screen.getByRole("button", { name: "Create event" }));
		await waitFor(() => expect(automationApi.createEvent).toHaveBeenCalledWith(expect.objectContaining({ trigger: { kind: "weekly", weekdays: [0], startTime: "22:00:00", endTime: "02:00:00", endDayOffset: 1, firstDate: "2026-11-23", lastDate: null } })));
		expect(screen.getAllByText(/Europe\/Berlin/).length).toBeGreaterThan(0);
	});

	it("sends enabling as one live action, while timeout is a configuration edit", async () => {
		mount(<SchedulePage />);
		await screen.findByLabelText("DMX silence timeout (s)");
		fireEvent.change(screen.getByLabelText("DMX silence timeout (s)"), { target: { value: "20" } });
		await waitFor(() => expect(automationApi.updateOutput).toHaveBeenCalledWith("main", { dmxSilenceSeconds: 20 }));
		await userEvent.click(screen.getByLabelText("Scheduling enabled"));
		await waitFor(() => expect(sendTelemetryAction).toHaveBeenCalledWith({ type: "automation-set-enabled", outputId: "main", enabled: true }));
		expect(automationApi.updateOutput).not.toHaveBeenCalledWith("main", expect.objectContaining({ enabled: true }));
	});

	it("shows DMX ownership and automatic resume countdown from server status", async () => {
		view.status[0] = { ...view.status[0], owner: "dmx", dmxAgeMillis: 2500, resumeInMillis: 7500 };
		mount(<SchedulePage />);
		expect(await screen.findByRole("heading", { name: "DMX controlling" })).toBeVisible();
		expect(screen.getByText(/Scheduling resumes in 7.5 s/)).toBeVisible();
	});
});
