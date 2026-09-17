import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaServerDiscovery } from "../../api/client/mediaOutput";
import type {
	FixtureDefinition,
	MediaServerFixture,
	OutputRoute,
	PatchedFixture,
	VersionedObject,
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
	clearMediaThumbnailCache: vi.fn(),
	inspectMediaServer: vi.fn(),
	discoverMediaServers: vi.fn(),
	updateDiscoveredMediaAddress: vi.fn(),
	patchFixtures: vi.fn(),
	deleteFixture: vi.fn(),
	fixtureLibrary: [] as FixtureDefinition[],
	patchError: null as string | null,
	routes: [] as VersionedObject<OutputRoute>[],
}));

const server = {
	mediaServers: [] as MediaServerFixture[],
	mediaPreviewUrls: {},
	refreshMediaPreview: mocks.refreshMediaPreview,
	refreshMediaThumbnails: mocks.refreshMediaThumbnails,
	clearMediaThumbnailCache: mocks.clearMediaThumbnailCache,
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
vi.mock("../../features/dmxDiagnostics/DmxDiagnosticsContext", () => ({
	useDmxDiagnostics: () => ({ outputRoutes: mocks.routes }),
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
	mocks.refreshMediaThumbnails.mockResolvedValue(true);
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
	mocks.routes = [
		route(4, "sacn", 4),
		route(7, "sacn", 7),
		route(9, "art_net", 12),
	];
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
		expect(screen.queryByRole("table")).toBeNull();
		expect(mocks.updateFixture).not.toHaveBeenCalled();
		expect(mocks.usePatchView).toHaveBeenCalledWith(true);
	});

	it("lists every patched server as one table row with its type", () => {
		patchFixtures = [
			fixture,
			{
				...mediaFixture(),
				fixture_id: "fixture-tosk",
				fixture_number: 2,
				name: "Pixel Rack",
				definition: toskMediaDefinition(),
			},
		];
		render(<MediaServerSetup />);

		const table = screen.getByRole("table", { name: "Patched Media Servers" });
		const headers = within(table)
			.getAllByRole("columnheader")
			.map((cell) => cell.textContent);
		expect(headers).toEqual([
			"#",
			"Name",
			"Type",
			"Protocol",
			"IP address",
			"Port",
			"Status",
			"Actions",
		]);
		const generic = rowFor("Media One");
		const tosk = rowFor("Pixel Rack");
		expect(generic).toHaveAttribute("data-server-type", "citp");
		expect(within(generic).getByText("CITP media server")).toBeInTheDocument();
		expect(tosk).toHaveAttribute("data-server-type", "tosklight");
		expect(within(tosk).getByText("ToskLight Media")).toBeInTheDocument();
		expect(within(generic).getByText("● Off")).toBeInTheDocument();
	});

	it("saves an endpoint through one typed Patch action and no generic mutation", async () => {
		render(<MediaServerSetup />);
		chooseProtocol("Media One", "CITP");
		fireEvent.change(screen.getByLabelText("Media One IP address"), {
			target: { value: "192.168.1.50" },
		});
		fireEvent.click(
			within(rowFor("Media One")).getByRole("button", { name: "Apply" }),
		);

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
		expect(
			await within(rowFor("Media One")).findByText(
				"Now using 192.168.1.50:4809.",
			),
		).toBeInTheDocument();
	});

	it("validates the IP address and port before a row can apply", () => {
		render(<MediaServerSetup />);
		const row = rowFor("Media One");
		const apply = within(row).getByRole("button", { name: "Apply" });
		chooseProtocol("Media One", "CITP");
		expect(within(row).getByRole("alert")).toHaveTextContent(
			"Enter the server's IP address, or set Protocol to Off.",
		);
		expect(apply).toBeDisabled();

		for (const invalid of ["300.1.1.1", "media-rack.local", "10.0.0"]) {
			fireEvent.change(screen.getByLabelText("Media One IP address"), {
				target: { value: invalid },
			});
			expect(within(row).getByRole("alert")).toHaveTextContent(
				"Enter an IPv4 or IPv6 address",
			);
			expect(apply).toBeDisabled();
		}
		fireEvent.change(screen.getByLabelText("Media One IP address"), {
			target: { value: "fe80::1" },
		});
		fireEvent.change(screen.getByLabelText("Media One port"), {
			target: { value: "70000" },
		});
		expect(within(row).getByRole("alert")).toHaveTextContent(
			"Choose a port from 1 to 65535.",
		);
		expect(apply).toBeDisabled();
		fireEvent.change(screen.getByLabelText("Media One port"), {
			target: { value: "4811" },
		});
		expect(within(row).queryByRole("alert")).toBeNull();
		expect(apply).toBeEnabled();
		expect(mocks.updateFixture).not.toHaveBeenCalled();
	});

	it("turns network control off through one typed Patch action", async () => {
		fixture = withEndpoint("192.168.1.60");
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		chooseProtocol("Media One", "Off");
		expect(screen.getByLabelText("Media One IP address")).toBeDisabled();
		fireEvent.click(
			within(rowFor("Media One")).getByRole("button", { name: "Apply" }),
		);

		await waitFor(() =>
			expect(mocks.updateFixture).toHaveBeenCalledWith("fixture-media", {
				direct_control: null,
			}),
		);
		expect(mocks.updateFixture).toHaveBeenCalledOnce();
	});

	it("shows the desk's refusal on the row it belongs to", async () => {
		mocks.updateFixture.mockResolvedValue(false);
		mocks.patchError =
			"Fixture 1 profile does not support Citp direct control.";
		render(<MediaServerSetup />);
		chooseProtocol("Media One", "CITP");
		fireEvent.change(screen.getByLabelText("Media One IP address"), {
			target: { value: "10.0.0.9" },
		});
		fireEvent.click(
			within(rowFor("Media One")).getByRole("button", { name: "Apply" }),
		);

		expect(
			await within(rowFor("Media One")).findByText(
				/does not support Citp direct control\. Check the address and retry\./,
			),
		).toBeInTheDocument();
	});

	it("does not show stale online status for a replaced endpoint", () => {
		fixture = withEndpoint("192.168.1.70");
		patchFixtures = [fixture];
		mocks.inspectMediaServer.mockReturnValue(new Promise(() => undefined));
		server.mediaServers = [status("192.168.1.60", true, null)];

		render(<MediaServerSetup />);

		expect(
			within(rowFor("Media One")).getByText("● Checking…"),
		).toBeInTheDocument();
		expect(screen.queryByText("● Connected")).toBeNull();
	});

	it("follows the desk's connection state from unchecked to connected to offline", async () => {
		fixture = withEndpoint("192.168.1.80");
		patchFixtures = [fixture];
		server.mediaServers = [
			{
				...status("192.168.1.80", false, null),
				status: { online: false, last_success: null, last_error: null },
			},
		];
		const view = render(<MediaServerSetup />);

		// The desk had nothing to report, so the row asks the server once.
		await waitFor(() =>
			expect(mocks.inspectMediaServer).toHaveBeenCalledWith("fixture-media"),
		);
		expect(
			await within(rowFor("Media One")).findByText("● Not checked"),
		).toBeInTheDocument();

		server.mediaServers = [status("192.168.1.80", true, null)];
		view.rerender(<MediaServerSetup />);
		expect(
			within(rowFor("Media One")).getByText("● Connected"),
		).toBeInTheDocument();

		server.mediaServers = [
			status("192.168.1.80", false, "CITP connection refused"),
		];
		view.rerender(<MediaServerSetup />);
		const row = rowFor("Media One");
		expect(within(row).getByText("● Offline")).toBeInTheDocument();
		expect(within(row).getByRole("alert")).toHaveTextContent(
			"CITP connection refused Check the IP address, port, and that the server is running, then Check connection to retry.",
		);
		expect(mocks.inspectMediaServer).toHaveBeenCalledOnce();
	});

	it("says whether discovery found the row's address", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		fixture = withEndpoint("192.168.1.40");
		patchFixtures = [
			fixture,
			{
				...withEndpoint("192.168.1.99"),
				fixture_id: "fixture-b",
				name: "Media Two",
			},
		];
		render(<MediaServerSetup />);

		expect(
			await within(rowFor("Media One")).findByText("Found on network"),
		).toBeInTheDocument();
		expect(
			within(rowFor("Media Two")).getByText("Not found by discovery"),
		).toBeInTheDocument();
	});

	it("uses the server-advertised Program source for live preview", async () => {
		fixture = withEndpoint("127.0.0.1");
		patchFixtures = [fixture];
		render(<MediaServerSetup />);
		const start = within(rowFor("Media One")).getByRole("button", {
			name: "Start live preview",
		});
		await waitFor(() => expect(start).toBeEnabled());
		fireEvent.click(start);

		await waitFor(() =>
			expect(mocks.refreshMediaPreview).toHaveBeenCalledWith(
				"fixture-media",
				42,
			),
		);
	});

	it("refreshes one row's advertised thumbnails without holding up another row", async () => {
		fixture = withEndpoint("127.0.0.1");
		patchFixtures = [
			fixture,
			{
				...withEndpoint("127.0.0.2"),
				fixture_id: "fixture-b",
				name: "Media Two",
			},
		];
		let finish: (value: boolean) => void = () => undefined;
		mocks.refreshMediaThumbnails.mockReturnValue(
			new Promise<boolean>((resolve) => {
				finish = resolve;
			}),
		);
		render(<MediaServerSetup />);
		const one = rowFor("Media One");
		const two = rowFor("Media Two");
		const refresh = within(one).getByRole("button", {
			name: "Refresh Thumbnails",
		});
		await waitFor(() => expect(refresh).toBeEnabled());
		fireEvent.click(refresh);

		await waitFor(() =>
			expect(mocks.refreshMediaThumbnails).toHaveBeenCalledWith(
				"fixture-media",
				1,
				[1],
			),
		);
		expect(
			within(one).getByRole("button", { name: "Refreshing…" }),
		).toBeDisabled();
		expect(
			within(two).getByRole("button", { name: "Refresh Thumbnails" }),
		).toBeEnabled();
		expect(mocks.refreshMediaThumbnails).not.toHaveBeenCalledWith(
			"fixture-b",
			expect.anything(),
			expect.anything(),
		);
		finish(true);
		expect(
			await within(one).findByText("Refreshed 1 thumbnails."),
		).toBeInTheDocument();
	});

	it("reports a thumbnail refresh the server stopped answering", async () => {
		fixture = withEndpoint("127.0.0.1");
		patchFixtures = [fixture];
		mocks.refreshMediaThumbnails.mockResolvedValue(false);
		render(<MediaServerSetup />);
		const refresh = within(rowFor("Media One")).getByRole("button", {
			name: "Refresh Thumbnails",
		});
		await waitFor(() => expect(refresh).toBeEnabled());
		fireEvent.click(refresh);

		expect(
			await within(rowFor("Media One")).findByText(
				/the server stopped answering\. Refresh Thumbnails to retry\./,
			),
		).toBeInTheDocument();
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
				protocol: "sacn",
			}),
		);
		expect(mocks.deleteFixture).toHaveBeenCalledOnce();
		expect(
			await screen.findByText(
				/The desk patch was restored\. Refresh Discovery/,
			),
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

		await screen.findByText(/restart it to activate the new DMX input/);
		expect(screen.getByText(/Suggested DMX 7\.201/)).toBeInTheDocument();
		expect(screen.getAllByText(/listens to sACN 7/).length).toBeGreaterThan(0);
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
				"Suggested DMX 4.101 · 2 layers · listens to sACN 4 · Tempo from Speed Group 3",
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
			await screen.findByText(
				/Check free space and write access/,
				{},
				{ timeout: 3000 },
			),
		).toHaveTextContent("The desk patch was restored");
		expect(mocks.deleteFixture).toHaveBeenCalledOnce();
	});
});

describe("Coordinated Media Server patching", () => {
	const OUTPUT = "00000000-0000-4000-8000-000000000040";

	function boundFixture(
		overrides: Partial<PatchedFixture> = {},
	): PatchedFixture {
		return {
			...mediaFixture(),
			fixture_id: "patched-media",
			name: "Pixel Rack Main",
			definition: toskMediaDefinition(),
			universe: 4,
			address: 101,
			split_patches: [{ split: 1, universe: 4, address: 101 }],
			direct_control: {
				protocol: "citp",
				ip_address: "192.168.1.40",
				port: 4809,
			},
			internal_bindings: { library: null, output: OUTPUT },
			...overrides,
		};
	}

	function confirmAddress(universe: number, address: number) {
		fireEvent.click(screen.getByRole("button", { name: "Patch address" }));
		fireEvent.change(screen.getByLabelText("Universe"), {
			target: { value: String(universe) },
		});
		fireEvent.change(screen.getByLabelText("Address"), {
			target: { value: String(address) },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Confirm patch address" }),
		);
	}

	it("names each discovered server by identity, address, type, and connection", async () => {
		const discovery = discoveredServer();
		discovery.servers.push({
			...discovery.servers[0],
			key: "192.168.1.41:4809",
			name: "Pixel Rack B",
			host: "192.168.1.41",
			instance: "pixel-rack-b",
			outputs: [
				{
					...discovery.servers[0].outputs[0],
					id: "00000000-0000-4000-8000-000000000041",
					name: "Side",
				},
			],
		});
		discovery.servers.push({
			key: "192.168.1.42:4809",
			name: "Pixel Spare",
			host: "192.168.1.42",
			citpPort: 4809,
			status: "Unavailable",
			instance: null,
			error:
				"The discovered Media Server did not answer its configuration API.",
			outputs: [],
		});
		mocks.discoverMediaServers.mockResolvedValue(discovery);
		patchFixtures = [boundFixture()];
		server.mediaServers = [
			{
				...status("192.168.1.40", true, null),
				fixture_id: "patched-media",
			},
		];
		render(<MediaServerSetup />);

		const main = (await screen.findByText("Pixel Rack · Main")).closest(
			"article",
		) as HTMLElement;
		expect(main).toHaveTextContent(
			"192.168.1.40 · ToskLight Media · CITP 4809 · Online · Desk connection: Connected",
		);
		expect(main).toHaveAttribute("data-patch-state", "patched");
		expect(within(main).getByText("Patched")).toBeInTheDocument();
		expect(main).not.toHaveTextContent("192.168.1.40:4809");

		const side = screen
			.getByText("Pixel Rack B · Side")
			.closest("article") as HTMLElement;
		expect(side).toHaveTextContent(
			"192.168.1.41 · ToskLight Media · CITP 4809 · Online · Desk connection: Not connected (not patched)",
		);
		expect(within(side).getByText("Not patched")).toBeInTheDocument();

		const spare = screen
			.getByText("Pixel Spare")
			.closest("article") as HTMLElement;
		expect(spare).toHaveTextContent(
			"192.168.1.42 · ToskLight Media · CITP 4809 · Offline",
		);
		expect(within(spare).getByText("Unavailable")).toBeInTheDocument();

		fireEvent.click(
			within(side).getByRole("button", { name: "Patch suggested" }),
		);
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		expect(mocks.patchFixtures.mock.calls[0][0][0].input).toMatchObject({
			directControl: { ipAddress: "192.168.1.41" },
			internalBindings: { output: "00000000-0000-4000-8000-000000000041" },
		});
		expect(within(main).queryByRole("status")).toBeNull();
	});

	it("suggests the desk universe whose route reaches the server", async () => {
		const discovery = discoveredServer();
		discovery.servers[0].outputs[0].protocol = "art-net";
		discovery.servers[0].outputs[0].universe = 12;
		mocks.discoverMediaServers.mockResolvedValue(discovery);
		patchFixtures = [];
		render(<MediaServerSetup />);

		expect(
			await screen.findByText(
				/Suggested DMX 9\.101 · 2 layers · listens to Art-Net 12/,
			),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Patch suggested" }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		expect(mocks.patchFixtures.mock.calls[0][0][0].input).toMatchObject({
			splitPatches: [{ split: 1, universe: 9, address: 101 }],
		});
		expect(
			await screen.findByText("Patched at DMX 9.101."),
		).toBeInTheDocument();
		expect(mocks.updateDiscoveredMediaAddress).not.toHaveBeenCalled();
	});

	it("patches a suggestion no route delivers, and says the server receives nothing yet", async () => {
		mocks.routes = [];
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [];
		render(<MediaServerSetup />);

		expect(
			await screen.findByText(/listens to sACN 4, which no desk route sends/),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Patch suggested" }));
		expect(
			await screen.findByText(
				/Patched at DMX 4\.101\. No desk output route sends sACN 4, so the server receives nothing yet/,
			),
		).toBeInTheDocument();
	});

	it("moves the server onto the protocol and universe the desk route sends", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.updateDiscoveredMediaAddress.mockImplementation(async (input) => ({
			...discoveredServer().servers[0].outputs[0],
			protocol: input.protocol,
			universe: input.universe,
			startAddress: input.startAddress,
		}));
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		confirmAddress(9, 33);

		expect(
			await screen.findByText(
				"Desk and Media Server now use DMX 9.33; the server listens to Art-Net 12.",
			),
		).toBeInTheDocument();
		expect(mocks.patchFixtures.mock.calls[0][0][0].input).toMatchObject({
			splitPatches: [{ split: 1, universe: 9, address: 33 }],
		});
		expect(mocks.updateDiscoveredMediaAddress).toHaveBeenCalledWith({
			host: "192.168.1.40",
			outputId: OUTPUT,
			universe: 12,
			startAddress: 33,
			protocol: "art-net",
		});
		expect(
			screen.getByText(
				"Suggested DMX 9.33 · 2 layers · listens to Art-Net 12",
				{
					exact: false,
				},
			),
		).toBeInTheDocument();
	});

	it("refuses a chosen universe the desk does not send before changing anything", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		confirmAddress(20, 1);

		expect(
			await screen.findByText(
				/The desk sends no network output for universe 20.*Nothing was changed\./,
			),
		).toBeInTheDocument();
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
		expect(mocks.updateDiscoveredMediaAddress).not.toHaveBeenCalled();
	});

	it("rejects a colliding chosen address without touching the server", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.patchFixtures.mockResolvedValue(null);
		mocks.patchError =
			"Fixtures 1 and 3 overlap on universe 7. Move or unpatch one fixture.";
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		confirmAddress(7, 1);

		expect(
			await screen.findByText(/overlap on universe 7/),
		).toBeInTheDocument();
		expect(mocks.updateDiscoveredMediaAddress).not.toHaveBeenCalled();
	});

	it("restores an existing patch and reports a failed restore as a possible mismatch", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		mocks.updateDiscoveredMediaAddress.mockRejectedValue(
			new Error("The Media Server is unreachable."),
		);
		patchFixtures = [boundFixture()];
		mocks.patchFixtures
			.mockResolvedValueOnce([{ fixtureId: "patched-media" }])
			.mockResolvedValueOnce(null);
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		confirmAddress(7, 1);

		expect(
			await screen.findByText(
				/unreachable\. Restoring the desk patch also failed, so the desk and the Media Server may differ\. Refresh Discovery/,
			),
		).toBeInTheDocument();
		expect(mocks.patchFixtures).toHaveBeenCalledTimes(2);
		expect(mocks.patchFixtures.mock.calls[1][0][0].input).toMatchObject({
			fixtureId: "patched-media",
			splitPatches: [{ split: 1, universe: 4, address: 101 }],
		});
		expect(mocks.deleteFixture).not.toHaveBeenCalled();
	});

	it("reports a server that kept another address instead of claiming success", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [];
		render(<MediaServerSetup />);

		await screen.findByText("Pixel Rack · Main");
		confirmAddress(7, 1);

		expect(
			await screen.findByText(
				"The desk is patched at DMX 7.1, but the Media Server kept sACN 4 at address 101. Patch suggested or retry Patch address.",
			),
		).toBeInTheDocument();
	});

	it("never reports a differing address or endpoint as patched", async () => {
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [
			boundFixture({
				universe: 7,
				address: 1,
				split_patches: [{ split: 1, universe: 7, address: 1 }],
			}),
		];
		const { unmount } = render(<MediaServerSetup />);

		const card = (await screen.findByText("Pixel Rack · Main")).closest(
			"article",
		) as HTMLElement;
		expect(card).toHaveAttribute("data-patch-state", "address-differs");
		expect(within(card).getByText("Address differs")).toBeInTheDocument();
		expect(within(card).getByRole("alert")).toHaveTextContent(
			"The desk sends DMX 7.1 as sACN 7, but this output listens to sACN 4 at address 101.",
		);
		expect(within(card).queryByText("Patched")).toBeNull();
		unmount();

		patchFixtures = [
			boundFixture({
				direct_control: {
					protocol: "citp",
					ip_address: "192.168.1.99",
					port: 4809,
				},
			}),
		];
		render(<MediaServerSetup />);
		const moved = (await screen.findByText("Pixel Rack · Main")).closest(
			"article",
		) as HTMLElement;
		expect(within(moved).getByText("Endpoint differs")).toBeInTheDocument();
		expect(within(moved).getByRole("alert")).toHaveTextContent(
			"The desk controls this server at 192.168.1.99:4809, but discovery found it at 192.168.1.40:4809.",
		);
		fireEvent.click(
			within(moved).getByRole("button", { name: "Patch suggested" }),
		);
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledOnce());
		expect(mocks.patchFixtures.mock.calls[0][0][0].input).toMatchObject({
			fixtureId: "patched-media",
			directControl: { ipAddress: "192.168.1.40", port: 4809 },
		});
	});

	it("says a patch whose universe no route sends is not received", async () => {
		mocks.routes = [];
		mocks.discoverMediaServers.mockResolvedValue(discoveredServer());
		patchFixtures = [boundFixture()];
		render(<MediaServerSetup />);

		const card = (await screen.findByText("Pixel Rack · Main")).closest(
			"article",
		) as HTMLElement;
		expect(within(card).getByText("Not received")).toBeInTheDocument();
		expect(within(card).getByRole("alert")).toHaveTextContent(
			"Add an output route for universe 4 under Setup › Outputs",
		);
	});

	it("checks one patched server's connection on request", async () => {
		patchFixtures = [withEndpoint("192.168.1.50")];
		server.mediaServers = [status("192.168.1.50", false, "Timed out.")];
		render(<MediaServerSetup />);
		mocks.inspectMediaServer.mockClear();

		fireEvent.click(
			within(rowFor("Media One")).getByRole("button", {
				name: "Check connection",
			}),
		);

		await waitFor(() =>
			expect(mocks.inspectMediaServer).toHaveBeenCalledWith("fixture-media"),
		);
		expect(
			await within(rowFor("Media One")).findByText("The server answered."),
		).toBeInTheDocument();
	});
});

function route(
	logical: number,
	protocol: OutputRoute["protocol"],
	destination: number,
): VersionedObject<OutputRoute> {
	return {
		kind: "route",
		id: `route-${logical}-${protocol}`,
		revision: 1,
		updated_at: "2026-09-17T00:00:00Z",
		body: {
			protocol,
			logical_universe: logical,
			destination_universe: destination,
			delivery_mode: "multicast",
			destination: null,
			enabled: true,
			minimum_slots: 512,
		},
	};
}

function rowFor(name: string): HTMLElement {
	const header = screen.getByRole("rowheader", { name });
	return header.closest("tr") as HTMLElement;
}

function chooseProtocol(name: string, protocol: "Off" | "CITP") {
	fireEvent.click(screen.getByRole("button", { name: `${name} protocol` }));
	fireEvent.click(screen.getByRole("option", { name: protocol }));
}

function withEndpoint(ip: string): PatchedFixture {
	return {
		...mediaFixture(),
		direct_control: { protocol: "citp", ip_address: ip, port: 4809 },
	};
}

function status(
	ip: string,
	online: boolean,
	lastError: string | null,
): MediaServerFixture {
	return {
		fixture_id: "fixture-media",
		name: "Media One",
		endpoint: { protocol: "citp", ip_address: ip, port: 4809 },
		layers: [],
		status: {
			online,
			last_success: online ? "2026-07-21T00:00:00Z" : null,
			last_error: lastError,
		},
	};
}

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
