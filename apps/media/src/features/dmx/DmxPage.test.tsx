import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../shared/api/client";
import { aDmxMap, stubServer } from "../../testing/server";
import { DmxPage } from "./DmxPage";

class FakeSocket {
	static opened: FakeSocket[] = [];
	onopen: (() => void) | undefined;
	onclose: (() => void) | undefined;
	onerror: (() => void) | undefined;
	onmessage: ((event: { data: string }) => void) | undefined;
	constructor(_url: string) {
		FakeSocket.opened.push(this);
	}
	close() {}
}

afterEach(() => {
	FakeSocket.opened = [];
	window.history.replaceState(null, "", "/");
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("DMX diagnostics", () => {
	it("is a dedicated Diagnostics window with a link to DMX input settings", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		const { container } = render(<DmxPage />);

		expect(screen.getByText("Diagnostics")).toHaveClass("ui-window-title");
		expect(
			screen.getByRole("button", { name: "Connect to Console" }),
		).toHaveClass("ui-primary", "media-connect-console");
		expect(container.querySelector(".media-dmx-window")).toBeInTheDocument();
		expect(container.querySelector(".media-dmx-content")).toBeInTheDocument();
		const configure = screen.getByRole("button", {
			name: "Configure DMX input",
		});
		expect(configure).toHaveClass("media-external-link-action");
		expect(
			configure.querySelector('svg[data-icon="external-link"]'),
		).toHaveAttribute("aria-hidden", "true");
		expect(
			container.querySelector(".ui-window-action-groups"),
		).toContainElement(configure);
		await userEvent.click(configure);
		expect(window.location.pathname).toBe("/settings");
		expect(window.location.search).toBe("?section=network");
		expect(window.location.hash).toBe("#dmx-input");
	});

	it("renders the canonical absolute channel map", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		render(<DmxPage />);

		expect(
			await screen.findByRole("cell", { name: "Folder" }),
		).toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "100" })).toBeInTheDocument();
		expect(
			screen.getByRole("cell", { name: "No frame received" }),
		).toBeInTheDocument();
	});

	it("opens actual generated downloads, live patch and console-specific instructions", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		render(<DmxPage />);
		expect(
			screen.queryByRole("link", {
				name: "Download ToskLight Pixel Layer.hed",
			}),
		).not.toBeInTheDocument();
		await userEvent.click(
			screen.getByRole("button", { name: "Connect to Console" }),
		);
		expect(
			await screen.findByRole("link", {
				name: "Download ToskLight Pixel Layer.hed",
			}),
		).toHaveAttribute("href", "/api/v2/fixtures/ToskLight%20Pixel%20Layer.hed");
		expect(
			screen.getByRole("link", { name: "Download ToskLight Pixel Master.hed" }),
		).toHaveAttribute("download");
		expect(
			await screen.findByRole("table", { name: /Suggested .* patch/ }),
		).toBeInTheDocument();
		expect(
			screen.getByText(/2 layers \(158\s+slots\) and 8 layers \(512 slots\)/),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				"2-layer personality: patch two Layer fixtures and one Master.",
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(/patch two heads\. Choose Pixel Master/),
		).toBeInTheDocument();
		expect(screen.queryByText(/Effect banks|full master/)).toBeNull();
		expect(await screen.findByText("0.0.0.0:5568")).toBeInTheDocument();
		expect(screen.getByText(/Same computer:/)).toBeInTheDocument();
		expect(
			screen.getByText(
				/called Send to applications on this PC in older versions/,
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(/Different computers: disable/),
		).toBeInTheDocument();
		expect(
			screen.getByText(/two distinct layer previews/),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Connect to Console" }),
		).toHaveClass("is-active");
		expect(
			screen.getByText(/Hold SHIFT and press VIEW SERVERS/),
		).toBeInTheDocument();
		expect(screen.getByText(/static thumbnail indicators/)).toBeInTheDocument();
		expect(
			screen.getByText(
				/press RELOAD THUMBS in MagicQ; it does not refresh automatically/,
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(/Net host options to Normal, without Loopback IP/),
		).toBeInTheDocument();
		const selector = screen.getByRole("combobox", { name: "Console" });
		await userEvent.selectOptions(selector, "grandMA2");
		expect(
			screen.getByRole("link", {
				name: "Download tosklight@pixel_layer@59ch.xml",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByRole("link", {
				name: "Download tosklight@pixel_master@40ch.xml",
			}),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("link", {
				name: "Download ToskLight Pixel Layer.hed",
			}),
		).not.toBeInTheDocument();
		for (const consoleName of ["grandMA3", "GDTF"]) {
			await userEvent.selectOptions(selector, consoleName);
			expect(
				screen.getByRole("link", {
					name: "Download ToskLight Pixel Layer.gdtf",
				}),
			).toHaveAttribute(
				"href",
				"/api/v2/fixtures/ToskLight%20Pixel%20Layer.gdtf",
			);
			expect(
				screen.getByRole("link", {
					name: "Download ToskLight Pixel Master.gdtf",
				}),
			).toBeInTheDocument();
		}
		await userEvent.click(
			screen.getByRole("button", { name: "Connect to Console" }),
		);
		expect(
			screen.queryByRole("combobox", { name: "Console" }),
		).not.toBeInTheDocument();
	});

	it("derives the 512-slot eight-layer blocks and the independent master from the running map", async () => {
		const server = stubServer();
		vi.stubGlobal("WebSocket", undefined);
		const output = server.outputs[0];
		const map = aDmxMap(output.id, output.name);
		const channel = map.channels[0];
		map.layerCount = 8;
		map.personality = "eightLayers";
		map.startAddress = 1;
		map.channels = Array.from({ length: 512 }, (_, index) => ({
			...channel,
			absoluteChannel: 1 + index,
			localOffset: index < 472 ? index % 59 : index - 472,
			group:
				index < 472
					? { kind: "layer" as const, number: Math.floor(index / 59) + 1 }
					: { kind: "master" as const },
		}));
		vi.spyOn(api, "dmxMap").mockResolvedValue(map);
		render(<DmxPage />);
		await userEvent.click(
			screen.getByRole("button", { name: "Connect to Console" }),
		);
		const table = await screen.findByRole("table", {
			name: /Suggested .* patch/,
		});
		const rows = table.querySelectorAll("tbody tr");
		expect(rows).toHaveLength(9);
		expect(
			[...rows[0].querySelectorAll("td")].map((cell) => cell.textContent),
		).toEqual(["Layer 1", "3", "1", "59", "59"]);
		expect(
			[...rows[7].querySelectorAll("td")].map((cell) => cell.textContent),
		).toEqual(["Layer 8", "3", "414", "472", "59"]);
		expect(
			[...rows[8].querySelectorAll("td")].map((cell) => cell.textContent),
		).toEqual(["Master", "3", "473", "512", "40"]);
		expect(
			screen.getByText(
				"8-layer personality: patch eight Layer fixtures and one Master.",
			),
		).toBeInTheDocument();
	});

	it("reports generated download failures instead of showing example links", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		vi.spyOn(api, "fixtures").mockRejectedValue(
			new Error("server unavailable"),
		);
		render(<DmxPage />);
		await userEvent.click(
			screen.getByRole("button", { name: "Connect to Console" }),
		);
		expect(
			await screen.findByText(
				/Could not load personalities:.*server unavailable/,
			),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("link", { name: /Download/ }),
		).not.toBeInTheDocument();
	});

	it("shows pushed winning-source diagnostics and exact raw bytes", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", FakeSocket);
		render(<DmxPage />);
		await screen.findByRole("cell", { name: "Folder" });
		const socket = FakeSocket.opened[0];
		act(() =>
			socket.onmessage?.({
				data: JSON.stringify({
					audio: { capturing: false },
					imports: [],
					dmx: [
						{
							outputId: "11111111-1111-4111-8111-111111111111",
							protocol: "art-net",
							universe: 3,
							startAddress: 100,
							source: "10.0.0.8",
							framesPerSecond: 25,
							ageMillis: 40,
							active: true,
							slots: [7],
						},
					],
				}),
			}),
		);

		await waitFor(() =>
			expect(screen.getByText("10.0.0.8")).toBeInTheDocument(),
		);
		expect(screen.getByText("25.0 fps")).toBeInTheDocument();
		expect(screen.getAllByRole("cell", { name: "7" })).toHaveLength(2);
	});

	it("shows where each output listens, and each row's DMX patch apart from its selected media", async () => {
		const server = stubServer();
		vi.stubGlobal("WebSocket", undefined);
		server.outputs[0].layers[0].address = { folder: 0, file: 0, class: "blank" };
		render(<DmxPage />);
		const output = server.outputs[0];
		const configured = await screen.findByRole("group", { name: "Configured DMX input" });
		// The running map is the authority for universe and address; the protocol is the output's.
		await waitFor(() => expect(configured).toHaveTextContent("Start address100"));
		expect(configured).toHaveTextContent("ProtocolArt-Net");
		expect(configured).toHaveTextContent("Universe3");
		expect(screen.getByRole("heading", { name: `Output “${output.name}”` })).toBeInTheDocument();
		expect(screen.getByText(/is this output’s name: one picture this server draws/u)).toBeInTheDocument();
		const table = screen.getByRole("table", { name: `DMX patch and selection of each row of ${output.name}` });
		const layerOne = within(table).getByRole("rowheader", { name: "Layer 1" }).closest("tr") as HTMLElement;
		expect(within(layerOne).getByText("Universe 3 · 100–100")).toBeInTheDocument();
		// No media selected reads as none, never as an address of zero.
		expect(within(layerOne).getByText("None selected")).toHaveClass("media-dmx-unset");
		expect(within(table).getByRole("rowheader", { name: "Master" })).toBeInTheDocument();
	});

	it("says when no start address is set, and follows a changed patch without a reload", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const map = vi.spyOn(api, "dmxMap");
		const base = aDmxMap("11111111-1111-4111-8111-111111111111", "Main");
		map.mockResolvedValue({ ...base, startAddress: 0 });
		render(<DmxPage />);
		const configured = await screen.findByRole("group", { name: "Configured DMX input" });
		await waitFor(() => expect(configured).toHaveTextContent("Not configured"));
		expect(within(configured).getByRole("alert")).toHaveTextContent(/No start address is set/u);

		map.mockResolvedValue({ ...base, universe: 7, startAddress: 200 });
		await act(async () => {
			await vi.advanceTimersByTimeAsync(3100);
		});
		await waitFor(() => expect(configured).toHaveTextContent("Start address200"));
		expect(configured).toHaveTextContent("Universe7");
		vi.useRealTimers();
	});
});
