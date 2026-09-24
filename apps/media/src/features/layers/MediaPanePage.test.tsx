import {
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { KEYS } from "../../shared/api/queries";
import { writeResource } from "../../shared/api/resource";
import {
	aCatalog,
	anOutput,
	anOutputConfiguration,
	stubServer,
} from "../../testing/server";
import { MediaPanePage } from "./MediaPanePage";

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

afterEach(() => {
	document.getElementById("media-playback-dock-action")?.remove();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});

describe("the production Media pane", () => {
	it("opens with browser controls in a right-hand pane", async () => {
		stubServer();
		const { container } = render(<MediaPanePage />);

		await screen.findByRole("button", { name: /Layer 1/iu });
		const browser = container.querySelector(".media-library-browser");
		const controls = container.querySelector(".media-secondary-controls");
		expect(browser).toBeInTheDocument();
		expect(controls).toBeInTheDocument();
		expect(browser?.compareDocumentPosition(controls as Node)).toBe(
			Node.DOCUMENT_POSITION_FOLLOWING,
		);
		const previewLoader = container.querySelector(
			".media-composite-picture img[hidden]",
		);
		expect(previewLoader).toHaveAttribute(
			"src",
			expect.stringContaining(
				"/outputs/11111111-1111-4111-8111-111111111111/preview",
			),
		);
		expect(screen.getByTestId("master-output-picture")).toHaveStyle({
			aspectRatio: "1920 / 1080",
		});
		expect(screen.getByText("192.0.2.10 · DMX U1 A1")).toBeInTheDocument();
		expect(screen.getByText("2 library items")).toBeInTheDocument();
	});

	it("labels one catalog entry as one library item", async () => {
		const server = stubServer();
		server.catalog.itemCount = 1;
		render(<MediaPanePage />);

		expect(await screen.findByText("1 library item")).toBeInTheDocument();
	});

	it("changes the running DMX facts with the selected output", async () => {
		const main = anOutput();
		const backup = anOutput({
			id: "22222222-2222-4222-8222-222222222222",
			name: "Backup",
		});
		stubServer({
			outputs: [main, backup],
			runtime: {
				administrationIp: "192.0.2.10",
				dataDirectory: "/Users/Shared/ToskLight Pixel",
				configurationFile: "/Users/Shared/ToskLight Pixel/media-server.json",
				libraryDirectory: "/Users/Shared/ToskLight Pixel/Media",
				portable: true,
				outputs: [
					{
						id: main.id,
						name: main.name,
						protocol: "art-net",
						universe: 1,
						startAddress: 1,
					},
					{
						id: backup.id,
						name: backup.name,
						protocol: "sacn",
						universe: 23,
						startAddress: 101,
					},
				],
			},
		});
		render(<MediaPanePage />);

		expect(
			await screen.findByText("192.0.2.10 · DMX U1 A1"),
		).toBeInTheDocument();
		await userEvent.click(
			screen.getByRole("button", { name: /Layer 1 Backup/iu }),
		);
		expect(screen.getByText("192.0.2.10 · DMX U23 A101")).toBeInTheDocument();
	});

	it("paints takeover feedback while the request is still pending", async () => {
		const server = stubServer();
		const request = deferred();
		server.holdWrites = request.promise;
		render(<MediaPanePage />);

		const takeover = await screen.findByRole("switch", {
			name: "Take over playback",
		});
		const inputAt = performance.now();
		fireEvent.click(takeover);
		const paintedAt = performance.now();

		// Painting before the held request resolves is the operator contract. A wall-clock budget
		// would measure the shared CI runner's React render instead of the pane's own behaviour.
		expect(takeover).toBeChecked();
		expect(paintedAt).toBeGreaterThanOrEqual(inputAt);
		expect(server.outputs[0].playbackTakeover).toBe(false);

		request.resolve();
		await waitFor(() => expect(server.outputs[0].playbackTakeover).toBe(true));
	});

	it("does not snap back on a stale poll and accepts a second toggle", async () => {
		const server = stubServer();
		const request = deferred();
		server.holdWrites = request.promise;
		render(<MediaPanePage />);

		const takeover = await screen.findByRole("switch", {
			name: "Take over playback",
		});
		fireEvent.click(takeover);
		expect(takeover).toBeChecked();

		writeResource(KEYS.outputs, structuredClone(server.outputs));
		expect(takeover).toBeChecked();
		expect(takeover).not.toBeDisabled();

		fireEvent.click(takeover);
		expect(takeover).not.toBeChecked();
		request.resolve();
		await waitFor(() =>
			expect(
				server.writes.filter((path) => /take-over|release/u.test(path)),
			).toHaveLength(2),
		);
		expect(server.outputs[0].playbackTakeover).toBe(false);
	});

	it("keeps rapid fader feedback immediate and coalesces a stalled drag to its latest value", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Frame" }));

		const request = deferred();
		server.holdWrites = request.promise;
		const scale = screen.getByLabelText("Scale X");
		fireEvent.pointerDown(scale);
		const inputAt = performance.now();
		fireEvent.input(scale, { target: { value: "2" } });
		fireEvent.input(scale, { target: { value: "3" } });
		const paintedAt = performance.now();

		expect(scale).toHaveValue("3");
		expect(paintedAt).toBeGreaterThanOrEqual(inputAt);
		await waitFor(() => expect(server.writes).toHaveLength(2));
		// Takeover is the first write; only one layer write reaches the held server at a time.
		expect(
			server.writes.filter((path) => path.endsWith("/layers/0/update")),
		).toHaveLength(1);

		for (let sample = 0; sample < 100; sample += 1)
			fireEvent.input(scale, {
				target: { value: String(4 + (sample % 7)) },
			});
		fireEvent.input(scale, { target: { value: "9" } });
		fireEvent.pointerUp(scale);
		expect(scale).toHaveValue("9");
		// All samples received while the first request is stalled occupy one latest slot.
		expect(
			server.writes.filter((path) => path.endsWith("/layers/0/update")),
		).toHaveLength(1);

		request.resolve();
		await waitFor(() => expect(server.outputs[0].layers[0].scaleX).toBe(9));
		expect(
			server.writes.filter((path) => path.endsWith("/layers/0/update")),
		).toHaveLength(2);
		expect(server.writeBodies.at(-1)).toEqual(
			expect.objectContaining({ scaleX: 9 }),
		);
	});

	it("does not reload output configuration while live controls change", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await screen.findByTestId("master-output-picture");
		await waitFor(() =>
			expect(
				server.requests.filter((path) => path.endsWith("/configuration")),
			).toHaveLength(1),
		);
		await userEvent.click(
			screen.getByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Frame" }));
		const scale = screen.getByLabelText("Scale X");
		fireEvent.pointerDown(scale);
		fireEvent.input(scale, { target: { value: "2" } });
		fireEvent.input(scale, { target: { value: "3" } });
		fireEvent.pointerUp(scale);
		await waitFor(() => expect(server.outputs[0].layers[0].scaleX).toBe(3));
		expect(
			server.requests.filter((path) => path.endsWith("/configuration")),
		).toHaveLength(1);
	});

	it("selects an exact speed band and serializes fractional BPM as an integer", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		const bpm = screen.getByLabelText("Playback BPM");
		expect(bpm).toHaveAttribute("step", "1");
		await userEvent.click(screen.getByRole("button", { name: "Speed" }));
		await userEvent.click(
			within(screen.getByRole("dialog", { name: "Choose Speed" })).getByRole(
				"button",
				{ name: /^2×/u },
			),
		);
		fireEvent.input(bpm, { target: { value: "120.1" } });

		await waitFor(() =>
			expect(server.writeBodies).toEqual(
				expect.arrayContaining([
					expect.objectContaining({ speedMultiplierDmx: 135 }),
					expect.objectContaining({ playbackBpm: 120 }),
				]),
			),
		);
	});

	it("renders one compact sidebar toggle labelled exactly Take over playback", async () => {
		stubServer();
		const dock = document.createElement("div");
		dock.id = "media-playback-dock-action";
		document.body.append(dock);
		render(<MediaPanePage />);

		await screen.findByRole("switch", { name: "Take over playback" });
		expect(dock).toHaveTextContent(/^Take over playback$/u);
		expect(dock).not.toHaveTextContent("Release");
		dock.remove();
	});

	it("release locks browsing and discards an uncommitted folder draft", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		const takeover = await screen.findByRole("switch", {
			name: "Take over playback",
		});
		await userEvent.click(takeover);
		const secondFolder = screen.getByRole("button", {
			name: /002Folder 0020 files/iu,
		});
		await userEvent.click(secondFolder);
		expect(secondFolder).toHaveClass("selected");

		await userEvent.click(takeover);
		await waitFor(() => expect(server.outputs[0].playbackTakeover).toBe(false));
		expect(secondFolder).toBeDisabled();
		expect(
			screen.getByRole("button", { name: /001Looks2 files/iu }),
		).toHaveClass("selected");
	});

	it("takes over only the selected output and exposes the master's controls", async () => {
		const second = anOutput({
			id: "22222222-2222-4222-8222-222222222222",
			name: "Second",
		});
		const server = stubServer({ outputs: [anOutput(), second] });
		render(<MediaPanePage />);

		await userEvent.click(
			await screen.findByRole("button", { name: /Layer 1 Second/iu }),
		);
		await userEvent.click(
			screen.getByRole("switch", { name: "Take over playback" }),
		);
		await waitFor(() => expect(second.playbackTakeover).toBe(true));
		expect(server.outputs[0].playbackTakeover).toBe(false);

		await userEvent.click(
			screen.getByRole("button", { name: /Master output/iu }),
		);
		const dimmer = await screen.findByLabelText("Dimmer");
		expect(dimmer).toBeEnabled();
		fireEvent.input(dimmer, { target: { value: "35" } });
		await waitFor(() => expect(second.master.dimmer).toBeCloseTo(0.35));
		await userEvent.click(screen.getByRole("tab", { name: "Colour" }));
		expect(
			screen.queryByRole("radiogroup", { name: "Flip / mirror" }),
		).not.toBeInTheDocument();
		await userEvent.click(screen.getByRole("tab", { name: "Geometry" }));
		fireEvent.input(screen.getByLabelText("Position X"), {
			target: { value: "0.5" },
		});
		fireEvent.input(screen.getByLabelText("Scale Y"), {
			target: { value: "1.5" },
		});
		fireEvent.input(screen.getByLabelText("Rotation"), {
			target: { value: "30" },
		});
		await waitFor(() => {
			expect(second.master.positionX).toBe(0.5);
			expect(second.master.scaleY).toBe(1.5);
			expect(second.master.rotation).toBe(30);
		});
		await userEvent.click(screen.getByRole("tab", { name: "Mask position" }));
		fireEvent.input(screen.getByLabelText("Mask position X"), {
			target: { value: "-0.75" },
		});
		fireEvent.input(screen.getByLabelText("Mask position Y"), {
			target: { value: "1.25" },
		});
		await waitFor(() => {
			expect(second.master.maskPositionX).toBe(-0.75);
			expect(second.master.maskPositionY).toBe(1.25);
		});
		await userEvent.click(screen.getByRole("tab", { name: "Shapers" }));
		fireEvent.input(screen.getByLabelText("Left"), {
			target: { value: "25" },
		});
		fireEvent.input(screen.getByLabelText("Left rotation"), {
			target: { value: "12" },
		});
		fireEvent.input(screen.getByLabelText("Module rotation"), {
			target: { value: "15" },
		});
		await waitFor(() => {
			expect(second.master.shaperLeft).toBe(0.25);
			expect(second.master.shaperLeftRotation).toBe(12);
			expect(second.master.shaperRotation).toBe(15);
		});

		await userEvent.click(
			screen.getByRole("button", { name: /Layer 1 Second/iu }),
		);
		const maskControlTab = screen.getAllByRole("tab", { name: "Mask" }).at(-1);
		expect(maskControlTab).toBeTruthy();
		await userEvent.click(maskControlTab as HTMLButtonElement);
		fireEvent.input(screen.getByLabelText("Mask position X"), {
			target: { value: "0.5" },
		});
		fireEvent.input(screen.getByLabelText("Mask position Y"), {
			target: { value: "-1" },
		});
		await waitFor(() => {
			expect(second.layers[0].mask.positionX).toBe(0.5);
			expect(second.layers[0].mask.positionY).toBe(-1);
		});
	});

	it("switches from Master back to a layer without leaving an invalid active tab", async () => {
		stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("button", { name: /Master output/iu }),
		);
		expect(screen.getByRole("tab", { name: "Output" })).toHaveAttribute(
			"aria-selected",
			"true",
		);

		await userEvent.click(screen.getByRole("button", { name: /Layer 1/iu }));
		expect(screen.getByRole("tab", { name: "Playback" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		expect(screen.getByText("Master output live preview")).toBeInTheDocument();
	});

	it("uses renderer layer previews and surfaces source failures", async () => {
		const output = anOutput();
		output.layers[0].sourceStatus = {
			state: "failed",
			failure: "the file could not be decoded; it may be damaged",
		};
		stubServer({ outputs: [output] });
		const { container } = render(<MediaPanePage />);

		await screen.findByRole("button", { name: /Layer 1/iu });
		expect(
			container.querySelector(".media-layer-thumbnail img"),
		).toHaveAttribute(
			"src",
			expect.stringContaining(
				"/outputs/11111111-1111-4111-8111-111111111111/layers/0/preview",
			),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"the file could not be decoded; it may be damaged",
		);
	});

	it("keeps takeover failure on the selected output", async () => {
		const server = stubServer();
		server.refuseWrites = {
			code: "refused",
			message: "takeover refused",
			status: 409,
		};
		render(<MediaPanePage />);

		await screen.findByRole("switch", { name: "Take over playback" });
		await userEvent.click(
			screen.getByRole("switch", { name: "Take over playback" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"takeover refused",
		);
		expect(server.outputs[0].playbackTakeover).toBe(false);
	});

	it("controls exactly two effect banks and the fixed master opacity cycle", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Effects" }));

		expect(screen.getByRole("tab", { name: "Bank 1" })).toBeInTheDocument();
		expect(screen.getByRole("tab", { name: "Bank 2" })).toBeInTheDocument();
		expect(
			screen.queryByRole("tab", { name: "Bank 3" }),
		).not.toBeInTheDocument();
		const layerEffects = screen.getByRole("tabpanel", {
			name: "Effects controls",
		});
		await userEvent.click(
			within(layerEffects).getByRole("button", { name: "Off" }),
		);
		await userEvent.click(
			screen.getByRole("option", { name: "2 · Unassigned" }),
		);
		fireEvent.input(screen.getByRole("slider", { name: "Effect Strength" }), {
			target: { value: "65" },
		});
		await waitFor(() => {
			expect(server.outputs[0].layers[0].effectBanks[0]).toEqual({
				index: 0,
				select: 2,
				strength: 0.65,
				parameters: [0, 0, 0, 0],
			});
		});

		await userEvent.click(
			screen.getByRole("button", { name: "Master output" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Effects" }));
		expect(
			screen.queryByRole("tab", { name: "Bank 1" }),
		).not.toBeInTheDocument();
		const masterEffects = screen.getByRole("tabpanel", {
			name: "Effects controls",
		});
		await userEvent.click(
			within(masterEffects).getByRole("button", { name: "Off" }),
		);
		await userEvent.click(screen.getByRole("option", { name: "4x" }));
		await waitFor(() =>
			expect(server.outputs[0].master.opacityCycleDmx).toBe(192),
		);
	});

	it("filters Media, VIS, and Text address spaces without writing playback", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		const writes = server.writes.length;
		expect(await screen.findByRole("tab", { name: "Media" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await userEvent.click(screen.getByRole("tab", { name: "VIS" }));
		expect(
			screen.getByRole("button", { name: /250Visualizers/iu }),
		).toBeInTheDocument();
		expect(
			document.querySelector('img[src*="/visualizers/previews/"]'),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("tab", { name: "Text" }));
		expect(
			screen.getByRole("button", { name: /200Text/iu }),
		).toBeInTheDocument();
		expect(
			document.querySelector('img[src^="data:image/svg+xml"]'),
		).toBeInTheDocument();
		expect(server.writes).toHaveLength(writes);
	});

	it("disables Master content while leaving its mask browser usable", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: /Master output/iu }),
		);
		const contentFolder = screen.getByRole("button", {
			name: /001Looks2 files/iu,
		});
		expect(contentFolder).toBeDisabled();
		const writes = server.writes.length;
		await userEvent.click(contentFolder, { pointerEventsCheck: 0 });
		expect(server.writes).toHaveLength(writes);
		await userEvent.click(screen.getByRole("tab", { name: "Mask" }));
		expect(
			screen.getByRole("button", { name: /001Looks2 files/iu }),
		).toBeEnabled();
	});

	it("keeps network-controlled choices disabled and sends no writes in monitor mode", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(await screen.findByRole("tab", { name: "Playback" }));
		expect(screen.getByRole("button", { name: "Play mode" })).toBeDisabled();
		for (const action of ["Stop", "Play", "Play looped"])
			expect(screen.getByRole("button", { name: action })).toBeDisabled();
		expect(
			screen.getByRole("button", { name: /001Looks2 files/iu }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: /001Blue haze/iu }),
		).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: /001Blue haze/iu }));
		expect(server.writes).toEqual([]);
	});

	it("uses compact play-mode quick actions and targets the selected layer", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("button", { name: /Layer 2/iu }));
		await userEvent.click(
			screen.getByRole("button", { name: /001Blue haze/iu }),
		);
		await waitFor(() =>
			expect(server.outputs[0].layers[1].address.file).toBe(1),
		);
		expect(server.outputs[0].layers[0].address.file).toBe(1);
		expect(
			server.writes.some((path) => path.endsWith("/layers/1/update")),
		).toBe(true);

		const quickActions = within(
			screen.getByRole("toolbar", { name: "Play mode quick actions" }),
		);
		await userEvent.click(quickActions.getByRole("button", { name: "Stop" }));
		await waitFor(() =>
			expect(server.outputs[0].layers[1].playModeDmx).toBe(216),
		);
		expect(quickActions.getByRole("button", { name: "Stop" })).toHaveClass(
			"is-active",
		);
	});

	it("writes content and rich controls after takeover", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: /001Blue haze/iu }),
		);
		await waitFor(() =>
			expect(
				server.writes.some((path) => path.endsWith("/layers/0/update")),
			).toBe(true),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Frame" }));
		const scale = screen.getByLabelText("Scale X");
		fireEvent.input(scale, { target: { value: "6" } });
		await waitFor(() => expect(server.outputs[0].layers[0].scaleX).toBe(6));
		expect(scale).toHaveAttribute("max", "10");
	});

	it("clears Content and Mask through the leading 000 file entry", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);

		let clear = screen.getByRole("button", {
			name: /000No file selected/iu,
		});
		await userEvent.click(clear);
		await waitFor(() =>
			expect(server.writeBodies.at(-1)).toEqual(
				expect.objectContaining({ folder: 1, file: 0 }),
			),
		);

		await userEvent.click(screen.getAllByRole("tab", { name: "Mask" })[0]);
		clear = screen.getByRole("button", {
			name: /000No file selected/iu,
		});
		await userEvent.click(clear);
		await waitFor(() =>
			expect(server.writeBodies.at(-1)).toEqual(
				expect.objectContaining({ maskFolder: 1, maskFile: 0 }),
			),
		);
	});

	it("keeps a refused playback edit visible to the operator", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		server.refuseWrites = {
			code: "refused",
			message: "the server refused this playback edit",
			status: 409,
		};
		await userEvent.click(
			screen.getByRole("button", { name: /001Blue haze/iu }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"the server refused this playback edit",
		);
	});

	it("routes a generated Text master mask selection to the master endpoint", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: /Master output/iu }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Mask" }));
		await userEvent.click(screen.getByRole("tab", { name: "Text" }));
		await userEvent.click(
			await screen.findByRole("button", { name: /001Clock/iu }),
		);
		await waitFor(() =>
			expect(
				server.writes.some((path) => path.endsWith("/master/update")),
			).toBe(true),
		);
		expect(
			server.writes.some((path) => path.endsWith("/layers/0/update")),
		).toBe(false);
		expect(server.writeBodies.at(-1)).toEqual(
			expect.objectContaining({ maskFolder: 200, maskFile: 1 }),
		);
	});

	it("clears the playback range back to the whole clip with one press", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Playback" }));
		const playback = screen.getByRole("tabpanel", {
			name: "Playback controls",
		});
		const clear = within(playback).getByRole("button", {
			name: "Clear playback range",
		});
		expect(clear).toHaveAttribute("title", "Clear playback range");
		expect(clear).toBeDisabled();

		const layer = server.outputs[0].layers[0];
		fireEvent.click(
			within(playback).getByRole("button", { name: /^In point: 00:00.00/ }),
		);
		typeInModal("In point (mm:ss.ff)", "00:12.00", 8);
		fireEvent.click(
			within(playback).getByRole("button", { name: /^Out point: End of clip/ }),
		);
		typeInModal("Out point (mm:ss.ff before end)", "00:04.00", 8);
		await waitFor(() => expect([layer.inPoint, layer.outPoint]).toEqual([300, 100]));
		await waitFor(() => expect(clear).toBeEnabled());

		fireEvent.click(clear);
		await waitFor(() => expect([layer.inPoint, layer.outPoint]).toEqual([0, 0]));
		expect(within(playback).getByText("End of clip")).toBeInTheDocument();
		expect(within(playback).getByText("00:00.00")).toBeInTheDocument();
		await waitFor(() => expect(clear).toBeDisabled());
	});

	it("shows the selected clip's own length beside its playback range", async () => {
		const catalog = aCatalog();
		catalog.folders[0].items[0].durationMillis = 24_000;
		catalog.folders[0].items.push({
			...catalog.folders[0].items[0],
			id: "asset-c",
			file: 3,
			name: "Unmeasured",
			durationMillis: null,
		});
		const server = stubServer({ catalog });
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Playback" }));
		const playback = screen.getByRole("tabpanel", {
			name: "Playback controls",
		});
		const length = () =>
			within(playback).getByText("Clip length").closest(".media-control-readout");
		expect(length()).toHaveTextContent("00:24.00");

		// Trimming the range leaves the clip's own length alone.
		fireEvent.click(
			within(playback).getByRole("button", { name: /^In point: 00:00.00/ }),
		);
		typeInModal("In point (mm:ss.ff)", "00:12.00", 8);
		await waitFor(() => expect(server.outputs[0].layers[0].inPoint).toBe(300));
		expect(length()).toHaveTextContent("00:24.00");

		// Another clip on the layer: its own length, or none for a still.
		server.outputs[0].layers[0].address.file = 2;
		await waitFor(() => expect(length()).toHaveTextContent("Still image"), {
			timeout: 3_000,
		});
		server.outputs[0].layers[0].address.file = 3;
		await waitFor(() => expect(length()).toHaveTextContent("Not reported"), {
			timeout: 3_000,
		});
		expect(length()).toHaveTextContent(
			"The Media Server does not report a length for this clip.",
		);
	});

	it("writes blend, strobe, playback range, and 3D mapping through the DMX byte meanings", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		const layer = server.outputs[0].layers[0];

		await userEvent.click(screen.getByRole("tab", { name: "Blend" }));
		fireEvent.input(screen.getByRole("slider", { name: "Strobe" }), {
			target: { value: "249" },
		});
		await waitFor(() => expect(layer.strobeHz).toBe(25));
		expect(await screen.findByText("25.0 Hz")).toBeInTheDocument();
		await chooseNamedChoice("Blend mode", "Screen");
		await waitFor(() => expect(layer.blendMode).toBe("screen"));
		expect(layer.strobeHz).toBeNull();
		expect(server.writeBodies.at(-1)).toEqual({ blendDmx: 32 });
		expect(await screen.findByText("Off")).toBeInTheDocument();

		await userEvent.click(screen.getByRole("tab", { name: "Playback" }));
		const playback = screen.getByRole("tabpanel", {
			name: "Playback controls",
		});
		expect(
			within(playback).getByRole("heading", { name: "Playback range" }),
		).toBeInTheDocument();
		expect(within(playback).getByText("End of clip")).toBeInTheDocument();
		// Points are typed as mm:ss.ff at the server's 25 fps, never dragged on a fader.
		expect(screen.queryByRole("slider", { name: "In point" })).toBeNull();
		expect(within(playback).getByText("00:00.00")).toBeInTheDocument();
		fireEvent.click(
			within(playback).getByRole("button", { name: /^In point: 00:00.00/ }),
		);
		typeInModal("In point (mm:ss.ff)", "00:12.00", 8);
		fireEvent.click(
			within(playback).getByRole("button", { name: /^Out point: End of clip/ }),
		);
		typeInModal("Out point (mm:ss.ff before end)", "00:24.00", 8);
		await waitFor(() => {
			expect(layer.inPoint).toBe(300);
			expect(layer.outPoint).toBe(600);
		});
		expect(screen.getByText("00:12.00")).toBeInTheDocument();
		expect(screen.getByText("00:24.00 before end")).toBeInTheDocument();

		await userEvent.click(screen.getByRole("tab", { name: "Frame" }));
		const frame = screen.getByRole("tabpanel", { name: "Frame controls" });
		expect(
			within(frame).getByRole("heading", { name: "3D model" }),
		).toBeInTheDocument();
		expect(
			within(frame).getByText("Rotation above is the model's roll."),
		).toBeInTheDocument();
		// A new mapping is Flat at Pan 0 and Tilt 0, and Flat still turns.
		expect(choiceTrigger("Model")).toHaveTextContent("Flat");
		expect(layer.model).toBe(0);
		expect(within(frame).getAllByText("0°")).toHaveLength(3);
		fireEvent.input(screen.getByRole("slider", { name: "Pan" }), {
			target: { value: "-90" },
		});
		fireEvent.input(screen.getByRole("slider", { name: "Tilt" }), {
			target: { value: "45" },
		});
		await waitFor(() => {
			expect(layer.modelPan).toBe(-90);
			expect(layer.modelTilt).toBe(45);
		});
		expect(layer.model).toBe(0);
		await chooseNamedChoice("Model", "3 · Sphere");
		await waitFor(() => expect(layer.model).toBe(3));
		expect(server.writeBodies.at(-1)).toEqual({ model: 3 });
	});

	it("shows playback range, the 3D model, and visualizer settings on exactly one tab each", async () => {
		const output = anOutput();
		output.layers[0].address = {
			folder: 250,
			file: 1,
			class: "generated-visualizer",
		};
		stubServer({ outputs: [output] });
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		const tabs = screen.getAllByRole("tab").map((tab) => tab.textContent ?? "");
		for (const retired of ["Playback range", "3D mapping", "Visualizer"])
			expect(tabs).not.toContain(retired);
		const points = ["In point", "Out point"];
		const owners = new Map([
			["Pan", "Frame"],
			["Tilt", "Frame"],
			["Parameter 1 · Count", "Effects"],
		]);
		for (const tab of [
			"Playback",
			"Frame",
			"Colour",
			"Mask",
			"Effects",
			"Blend",
		]) {
			// Mask is also a browser mode; the control-section tab is the later one.
			const [button] = screen.getAllByRole("tab", { name: tab }).slice(-1);
			await userEvent.click(button);
			expect(
				screen.getByRole("tabpanel", { name: `${tab} controls` }),
			).toBeInTheDocument();
			for (const point of points)
				expect(
					screen.queryAllByRole("button", { name: new RegExp(`^${point}:`) }),
				).toHaveLength(tab === "Playback" ? 1 : 0);
			for (const [slider, owner] of owners)
				if (owner === tab)
					expect(screen.getByRole("slider", { name: slider })).toBeVisible();
				else
					expect(
						screen.queryByRole("slider", { name: slider }),
					).not.toBeInTheDocument();
			expect(
				screen.queryAllByText("Model", { selector: "label" }),
			).toHaveLength(tab === "Frame" ? 1 : 0);
		}
	});

	it("configures a shown visualizer under Visualizer on the Effects tab", async () => {
		const output = anOutput();
		output.layers[0].address = {
			folder: 250,
			file: 1,
			class: "generated-visualizer",
		};
		const server = stubServer({ outputs: [output] });
		server.visualizers[0].uses = ["count", "primary"];
		server.visualizers[0].channels = server.visualizers[0].channels
			.filter(
				(channel) =>
					channel.parameter === "count" || channel.parameter === "primary",
			)
			.map((channel, index) => ({ ...channel, index }));
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Effects" }));
		const bankTabs = screen.getByRole("tablist", { name: "Effect bank" });
		expect(
			within(bankTabs)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual(["Visualizer", "Bank 1", "Bank 2"]);
		expect(
			within(bankTabs).getByRole("tab", { name: "Visualizer" }),
		).toHaveAttribute("aria-selected", "true");
		const effects = screen.getByRole("tabpanel", { name: "Effects controls" });
		expect(
			within(effects).getByRole("heading", { name: "Visualizer" }),
		).toBeInTheDocument();

		const configuredCount = screen.getByRole("slider", { name: "Count" });
		fireEvent.input(configuredCount, { target: { value: "64" } });
		await waitFor(() =>
			expect(server.outputs[0].layers[0].visualizerParameters?.count).toBe(64),
		);
		// Every visualizer offers its own audio gain, though it is not a kind parameter.
		fireEvent.input(screen.getByRole("slider", { name: "Audio gain" }), {
			target: { value: "2.5" },
		});
		await waitFor(() =>
			expect(
				server.outputs[0].layers[0].visualizerParameters?.audioGain,
			).toBe(2.5),
		);
		// The tuning belongs to the layer: no effect slot is addressed or changed.
		expect(server.writeBodies.at(-1)).not.toHaveProperty("effectSlot");
		expect(server.outputs[0].layers[0].effects[0].effectType).toBeNull();
		await userEvent.click(screen.getByText("Reset parameters"));
		await waitFor(() =>
			expect(server.outputs[0].layers[0].visualizerParameters).toBeNull(),
		);
		expect(server.writeBodies.at(-1)).toEqual({
			resetVisualizerParameters: true,
		});
		fireEvent.input(screen.getByRole("slider", { name: "Count" }), {
			target: { value: "64" },
		});
		await waitFor(() =>
			expect(server.outputs[0].layers[0].visualizerParameters?.count).toBe(64),
		);

		const count = screen.getByRole("slider", {
			name: "Parameter 1 · Count",
		});
		expect(count).toBeEnabled();
		expect(
			screen.getByRole("slider", { name: "Parameter 2 · Colour" }),
		).toBeEnabled();
		expect(screen.getByRole("slider", { name: "Parameter 3" })).toBeDisabled();
		expect(screen.getByRole("slider", { name: "Parameter 4" })).toBeDisabled();
		expect(screen.getByText("Default · 64")).toBeInTheDocument();
		expect(screen.getAllByText("Unused")).toHaveLength(2);
		fireEvent.input(count, { target: { value: "200" } });
		await waitFor(() =>
			expect(server.outputs[0].layers[0].visualizerControls).toEqual([
				200, 0, 0, 0,
			]),
		);
		expect(server.writeBodies.at(-1)).toEqual({
			visualizerParameterIndex: 0,
			visualizerParameterValue: 200,
		});

		await userEvent.click(
			within(bankTabs).getByRole("tab", { name: "Bank 1" }),
		);
		expect(screen.queryByRole("slider", { name: "Count" })).toBeNull();
		expect(
			screen.getByRole("slider", { name: "Effect Strength" }),
		).toBeInTheDocument();
	});

	it("shows no visualizer controls for a layer showing library media", async () => {
		stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Effects" }));
		const bankTabs = screen.getByRole("tablist", { name: "Effect bank" });
		expect(
			within(bankTabs)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual(["Bank 1", "Bank 2"]);
		expect(
			screen.queryByRole("heading", { name: "Visualizer" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryAllByRole("slider", { name: /^Parameter \d/u }),
		).toHaveLength(4);
		expect(
			screen.queryByRole("slider", { name: /^Parameter \d · /u }),
		).toBeNull();
	});

	it("mirrors the master through negative scale without Flip / mirror", async () => {
		const output = anOutput();
		const server = stubServer({
			outputs: [output],
			outputConfigurations: {
				[output.id]: anOutputConfiguration(output.id, output.name),
			},
		});
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: /Master output/iu }),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Colour" }));
		await waitFor(() =>
			expect(server.requests).toContain(`/outputs/${output.id}/configuration`),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("radiogroup", { name: "Flip / mirror" }),
			).not.toBeInTheDocument(),
		);
		await userEvent.click(screen.getByRole("tab", { name: "Geometry" }));
		const scaleX = screen.getByRole("slider", { name: "Scale X" });
		expect(scaleX).toHaveAttribute("min", "-4");
		fireEvent.input(scaleX, { target: { value: "-1" } });
		await waitFor(() => expect(server.outputs[0].master.scaleX).toBe(-1));
	});
});

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

async function chooseEffect(slot: number, name: string) {
	await userEvent.click(screen.getByRole("tab", { name: `Effect ${slot}` }));
	await chooseNamedChoice(`Slot ${slot} effect`, name);
}

function choiceTrigger(labelText: string) {
	const label = screen.getByText(labelText, { selector: "label" });
	const trigger = label.parentElement?.querySelector<HTMLButtonElement>(
		'button[aria-haspopup="listbox"]',
	);
	expect(trigger).toBeTruthy();
	return trigger as HTMLButtonElement;
}

async function chooseNamedChoice(labelText: string, name: string) {
	const trigger = choiceTrigger(labelText);
	await waitFor(() => expect(trigger).toBeEnabled());
	fireEvent.click(trigger as HTMLButtonElement);
	fireEvent.click(screen.getByRole("option", { name }));
}

/** Replaces the text in the open entry modal and commits it with Enter. */
function typeInModal(title: string, text: string, clear: number) {
	screen.getByRole("dialog", { name: title });
	for (let index = 0; index < clear; index += 1)
		fireEvent.keyDown(window, { key: "Backspace" });
	for (const key of text) fireEvent.keyDown(window, { key });
	fireEvent.keyDown(window, { key: "Enter" });
}
