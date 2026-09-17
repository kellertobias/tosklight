import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaServerDiscovery } from "../../api/client/mediaOutput";
import type {
	FixtureDefinition,
	MediaServerFixture,
	PatchedFixture,
} from "../../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "./fixtureProfileModel";
import { MediaServerSetup } from "./MediaServerSetup";

const mocks = vi.hoisted(() => ({
	status: "ready" as "loading" | "ready",
	updateFixture: vi.fn(),
	usePatchView: vi.fn(),
	putObject: vi.fn(),
	deleteObject: vi.fn(),
	refresh: vi.fn(),
	refreshMediaPreview: vi.fn(),
	refreshMediaThumbnails: vi.fn(),
	inspectMediaServer: vi.fn(),
	discoverMediaServers: vi.fn(),
	updateDiscoveredMediaAddress: vi.fn(),
	patchFixtures: vi.fn(),
	deleteFixture: vi.fn(),
	fixtureLibrary: [] as FixtureDefinition[],
	patchError: null as string | null,
}));

const server = {
	mediaServers: [] as MediaServerFixture[],
	mediaPreviewUrls: {},
	refreshMediaPreview: mocks.refreshMediaPreview,
	refreshMediaThumbnails: mocks.refreshMediaThumbnails,
	inspectMediaServer: mocks.inspectMediaServer,
	discoverMediaServers: mocks.discoverMediaServers,
	updateDiscoveredMediaAddress: mocks.updateDiscoveredMediaAddress,
	putObject: mocks.putObject,
	deleteObject: mocks.deleteObject,
	refresh: mocks.refresh,
};

let fixture: PatchedFixture;
let patchFixtures: readonly PatchedFixture[];

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => server,
}));
vi.mock("../../features/fixtureLibrary/FixtureLibraryContext", () => ({
	useFixtureLibrary: () => ({
		fixtureLibrary: mocks.fixtureLibrary,
		fixtureProfiles: [],
	}),
}));
vi.mock("../../features/patch/PatchContext", () => ({
	usePatch: () => ({
		status: mocks.status,
		fixtures: patchFixtures,
		error: mocks.patchError,
		updateFixture: mocks.updateFixture,
		patchFixtures: mocks.patchFixtures,
		deleteFixture: mocks.deleteFixture,
	}),
	usePatchView: mocks.usePatchView,
}));

beforeEach(() => {
	vi.clearAllMocks();
	mocks.status = "ready";
	mocks.patchError = null;
	mocks.updateFixture.mockResolvedValue(true);
	mocks.inspectMediaServer.mockResolvedValue({
		library_revision: "test",
		server: { name: "Media", layer_count: 2 },
		folders: [{ id: 1, name: "CITP Test", element_count: 1 }],
		files: [
			{
				folder_id: 1,
				id: 1,
				name: "CITP Test Pattern",
				width: 128,
				height: 72,
				length_frames: 8,
				fps: 8,
			},
		],
		preview_sources: [
			{
				id: 42,
				name: "Program",
				physical_output: 0,
				layer: null,
				width: 128,
				height: 72,
			},
		],
		layers: [],
		capabilities: { provider: "citp_msex", native_action: null, layers: [] },
	});
	mocks.refreshMediaPreview.mockResolvedValue(true);
	mocks.discoverMediaServers.mockResolvedValue({
		servers: [],
		discoveryError: null,
	});
	mocks.updateDiscoveredMediaAddress.mockResolvedValue({
		id: "00000000-0000-4000-8000-000000000040",
		name: "Main",
		personality: "two-layers",
		protocol: "sacn",
		universe: 4,
		startAddress: 101,
		dmxPendingRestart: false,
		mode: "2 layers",
		tempoSource: "playback-bpm-channel",
		speedGroup: null,
		issue: null,
	});
	mocks.patchFixtures.mockResolvedValue([
		{ fixtureId: "patched-media", selectionFixtureIds: ["patched-media"] },
	]);
	mocks.deleteFixture.mockResolvedValue(true);
	mocks.fixtureLibrary = [toskMediaDefinition()];
	server.mediaServers = [];
	fixture = mediaFixture();
	patchFixtures = [fixture];
});

afterEach(cleanup);

describe("Media server Patch authority", () => {
	it("hides retained fixtures and refuses configuration while Patch loads", () => {
		mocks.status = "loading";
		render(<MediaServerSetup />);

		expect(screen.getByText("Patch authority loading…")).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Disable CITP" })).toBeNull();
		expect(mocks.updateFixture).not.toHaveBeenCalled();
		expect(mocks.usePatchView).toHaveBeenCalledWith(true);
	});

	it("saves an endpoint through one typed Patch action and no generic mutation", async () => {
		render(<MediaServerSetup />);
		fireEvent.change(screen.getByLabelText("Acme Media One CITP IP address"), {
			target: { value: "192.168.1.50" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save endpoint" }));

		await waitFor(() =>
			expect(mocks.updateFixture).toHaveBeenCalledWith("fixture-media", {
				direct_control: {
					protocol: "citp",
					ip_address: "192.168.1.50",
					port: 4809,
				},
			}),
		);
		expect(mocks.updateFixture).toHaveBeenCalledOnce();
		expect(mocks.putObject).not.toHaveBeenCalled();
		expect(mocks.deleteObject).not.toHaveBeenCalled();
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("disables CITP through one typed Patch action", async () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "192.168.1.60",
				port: 4809,
			},
		};
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		fireEvent.change(screen.getByLabelText("Acme Media One CITP IP address"), {
			target: { value: "" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Disable CITP" }));

		await waitFor(() =>
			expect(mocks.updateFixture).toHaveBeenCalledWith("fixture-media", {
				direct_control: null,
			}),
		);
		expect(mocks.updateFixture).toHaveBeenCalledOnce();
	});

	it("does not show stale online status for a replaced endpoint", () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "192.168.1.70",
				port: 4809,
			},
		};
		patchFixtures = [fixture];
		server.mediaServers = [
			{
				fixture_id: fixture.fixture_id,
				name: "Media One",
				endpoint: {
					protocol: "citp",
					ip_address: "192.168.1.60",
					port: 4809,
				},
				layers: [],
				status: {
					online: true,
					last_success: "2026-07-21T00:00:00Z",
					last_error: null,
				},
			},
		];

		render(<MediaServerSetup />);

		expect(screen.getByText("● Offline")).toBeInTheDocument();
		expect(screen.queryByText("● Online")).toBeNull();
		expect(screen.getByText(/No successful response yet/)).toBeInTheDocument();
	});

	it("uses the server-advertised Program source for live preview", async () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "127.0.0.1",
				port: 4809,
			},
		};
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		fireEvent.click(screen.getByRole("button", { name: "Start live preview" }));

		await waitFor(() =>
			expect(mocks.refreshMediaPreview).toHaveBeenCalledWith(
				"fixture-media",
				42,
			),
		);
	});

	it("refreshes real advertised files instead of invalid zero addresses", async () => {
		fixture = {
			...fixture,
			direct_control: {
				protocol: "citp",
				ip_address: "127.0.0.1",
				port: 4809,
			},
		};
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		fireEvent.click(
			screen.getByRole("button", { name: "Refresh thumbnails 1–16" }),
		);

		await waitFor(() =>
			expect(mocks.refreshMediaThumbnails).toHaveBeenCalledWith(
				"fixture-media",
				1,
				[1],
			),
		);
	});

	it("discovers an unpatched ToskLight server and applies its suggested address", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		expect(screen.getByText("Not patched")).toBeInTheDocument();
		expect(screen.getByText(/Suggested DMX 4\.101/)).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Patch suggested" }));

		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		const candidate = mocks.patchFixtures.mock.calls[0][0][0];
		expect(candidate.input).toMatchObject({
			fixtureNumber: 1,
			directControl: {
				protocol: "citp",
				ipAddress: "192.168.1.40",
				port: 4809,
			},
			internalBindings: {
				output: "00000000-0000-4000-8000-000000000040",
			},
			splitPatches: [{ split: 1, universe: 4, address: 101 }],
		});
		expect(mocks.updateDiscoveredMediaAddress).not.toHaveBeenCalled();
	});

	it("rolls a new desk patch back when the selected server rejects a chosen address", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.updateDiscoveredMediaAddress.mockRejectedValue(
			new Error("Media Server disconnected"),
		);
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		fireEvent.click(screen.getByRole("button", { name: "Patch address" }));
		fireEvent.change(screen.getByLabelText("Universe"), {
			target: { value: "7" },
		});
		fireEvent.change(screen.getByLabelText("Address"), {
			target: { value: "201" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Confirm patch address" }),
		);

		await waitFor(() =>
			expect(mocks.updateDiscoveredMediaAddress).toHaveBeenCalledWith({
				host: "192.168.1.40",
				outputId: "00000000-0000-4000-8000-000000000040",
				universe: 7,
				startAddress: 201,
			}),
		);
		expect(mocks.deleteFixture).toHaveBeenCalledOnce();
		expect(
			screen.getByText(/The desk patch was restored; retry/),
		).toBeInTheDocument();
	});

	it("shows the selected output's authoritative address and restart state after a successful update", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.updateDiscoveredMediaAddress.mockResolvedValue({
			id: "00000000-0000-4000-8000-000000000040",
			name: "Main",
			personality: "two-layers",
			protocol: "sacn",
			universe: 7,
			startAddress: 201,
			dmxPendingRestart: true,
			mode: "2 layers",
			tempoSource: "playback-bpm-channel",
			speedGroup: null,
			issue: null,
		});
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		fireEvent.click(screen.getByRole("button", { name: "Patch address" }));
		fireEvent.change(screen.getByLabelText("Universe"), {
			target: { value: "7" },
		});
		fireEvent.change(screen.getByLabelText("Address"), {
			target: { value: "201" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Confirm patch address" }),
		);

		await screen.findByText(/Restart the Media Server to activate/);
		expect(screen.getByText(/Suggested DMX 7\.201/)).toBeInTheDocument();
		expect(screen.getByText(/DMX change pending restart/)).toBeInTheDocument();
	});

	it("keeps the server untouched when normal desk collision validation rejects the patch", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.patchFixtures.mockResolvedValue(null);
		mocks.patchError =
			"Fixtures 1 and 2 overlap on universe 4. Move or unpatch one fixture.";
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		fireEvent.click(screen.getByRole("button", { name: "Patch suggested" }));

		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		expect(mocks.updateDiscoveredMediaAddress).not.toHaveBeenCalled();
		expect(screen.getAllByRole("alert")[0]).toHaveTextContent("overlap");
	});

	it("shows the output's current personality, protocol, and tempo source", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [];
		render(<MediaServerSetup />);

		expect(
			await screen.findByText(
				"Suggested DMX 4.101 · 2 layers · sACN · Tempo from Speed Group 3",
			),
		).toBeInTheDocument();
		expect(screen.queryByText(/two-layers/)).not.toBeInTheDocument();
	});

	it("lists an outdated Media Server without offering a patch", async () => {
		const outdated = discoveredServer();
		outdated.servers[0].error =
			"This Media Server needs an update before the desk can patch it. Update ToskLight Media to the current version, then refresh discovery.";
		Object.assign(outdated.servers[0].outputs[0], {
			mode: null,
			tempoSource: null,
			speedGroup: null,
			issue: outdated.servers[0].error,
		});
		mocks.discoverMediaServers.mockResolvedValue(outdated);
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		expect(screen.getByText("Needs update")).toBeInTheDocument();
		expect(screen.getByText(/Unsupported personality/)).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Update ToskLight Media",
		);
		expect(
			screen.getByRole("button", { name: "Patch suggested" }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Patch address" }),
		).toBeDisabled();
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
	});

	it("explains an unreachable Media Server", async () => {
		const unavailable = discoveredServer();
		unavailable.servers[0].outputs = [];
		unavailable.servers[0].status = "Unavailable";
		unavailable.servers[0].error =
			"The discovered Media Server did not answer its configuration API. Check that it is running and reachable on port 8080, then refresh discovery.";
		mocks.discoverMediaServers.mockResolvedValue(unavailable);
		render(<MediaServerSetup />);

		expect(await screen.findByText("Unavailable")).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"reachable on port 8080",
		);
	});

	it("switches a patched fixture to the output's personality", async () => {
		const [twoLayers, eightLayers] = toskMediaDefinitions();
		mocks.fixtureLibrary = [twoLayers, eightLayers];
		const server = discoveredServer();
		Object.assign(server.servers[0].outputs[0], {
			personality: "eight-layers",
			mode: "8 layers",
			startAddress: 1,
		});
		mocks.discoverMediaServers.mockResolvedValue(server);
		patchFixtures = [
			{
				...mediaFixture(),
				fixture_id: "patched-media",
				definition: twoLayers,
				universe: 4,
				address: 1,
				split_patches: [{ split: 1, universe: 4, address: 1 }],
				direct_control: {
					protocol: "citp",
					ip_address: "192.168.1.40",
					port: 4809,
				},
			},
		];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		expect(screen.getByText("Mode differs")).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"The desk patch uses 2 layers, but this output uses 8 layers.",
		);
		fireEvent.click(screen.getByRole("button", { name: "Patch suggested" }));

		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		const candidate = mocks.patchFixtures.mock.calls[0][0][0];
		expect(candidate.input).toMatchObject({
			fixtureId: "patched-media",
			modeId: "b134a5f3-1adf-5bba-a2be-dbfb2c654395",
			splitPatches: [{ split: 1, universe: 4, address: 1 }],
		});
	});

	it("words a Media Server save refusal as an operator action and restores the desk", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.updateDiscoveredMediaAddress.mockRejectedValue(
			new Error(
				"The Media Server could not save this change, so it did not apply it. Check free space and write access for its configuration folder, then retry.",
			),
		);
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		fireEvent.click(screen.getByRole("button", { name: "Patch address" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Confirm patch address" }),
		);

		expect(
			await screen.findByText(/Check free space and write access/),
		).toHaveTextContent("The desk patch was restored");
		expect(mocks.deleteFixture).toHaveBeenCalledOnce();
	});
});

function discoveredServer(): MediaServerDiscovery {
	return {
		servers: [
			{
				key: "192.168.1.40:4809",
				name: "Pixel Rack",
				host: "192.168.1.40",
				citpPort: 4809,
				status: "ready",
				instance: "pixel-rack-a",
				error: null,
				outputs: [
					{
						id: "00000000-0000-4000-8000-000000000040",
						name: "Main",
						personality: "two-layers",
						protocol: "sacn",
						universe: 4,
						startAddress: 101,
						dmxPendingRestart: false,
						mode: "2 layers",
						tempoSource: "speed-group",
						speedGroup: 3,
						issue: null,
					},
				],
			},
		],
		discoveryError: null,
	};
}

function toskMediaDefinitions() {
	const profile = blankFixtureProfile();
	profile.id = "0a14fb60-280d-5ef1-aa4a-2ff11bd06943";
	profile.revision = 9;
	profile.manufacturer = "ToskLight";
	profile.name = "Media Server";
	profile.short_name = "Media Server";
	profile.direct_control_protocols = ["citp"];
	profile.modes[0].id = "a134a5f3-1adf-5bba-a2be-dbfb2c654395";
	profile.modes[0].name = "2 layers";
	profile.modes.push({
		...structuredClone(profile.modes[0]),
		id: "b134a5f3-1adf-5bba-a2be-dbfb2c654395",
		name: "8 layers",
	});
	return fixtureDefinitionsFromProfiles([profile]);
}

function toskMediaDefinition() {
	const profile = blankFixtureProfile();
	profile.id = "0a14fb60-280d-5ef1-aa4a-2ff11bd06943";
	profile.revision = 4;
	profile.manufacturer = "ToskLight";
	profile.name = "Media Server";
	profile.short_name = "Media Server";
	profile.direct_control_protocols = ["citp"];
	profile.modes[0].id = "a134a5f3-1adf-5bba-a2be-dbfb2c654395";
	profile.modes[0].name = "2 layers";
	return fixtureDefinitionsFromProfiles([profile])[0];
}

function mediaFixture(): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = "profile-media";
	profile.revision = 2;
	profile.manufacturer = "Acme";
	profile.name = "Media One";
	profile.short_name = "Media One";
	profile.direct_control_protocols = ["citp"];
	profile.modes[0].id = "mode-media";
	const definition = fixtureDefinitionsFromProfiles([profile])[0];
	return {
		fixture_id: "fixture-media",
		fixture_number: 1,
		virtual_fixture_number: null,
		name: "Media One",
		definition,
		universe: 1,
		address: 1,
		split_patches: [{ split: 1, universe: 1, address: 1 }],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
}
