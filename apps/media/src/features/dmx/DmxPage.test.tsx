import { render, screen, waitFor } from "@testing-library/react";
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
		await userEvent.click(
			screen.getByRole("button", { name: "Configure DMX input" }),
		);
		expect(window.location.pathname).toBe("/settings");
		expect(window.location.search).toBe("?section=dmx");
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
			screen.getByText(/not configured for eight layers/),
		).toBeInTheDocument();
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
			screen.getByText(/eight distinct layer previews/),
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
				name: "Download tosklight@pixel_layer@39ch.xml",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByRole("link", {
				name: "Download tosklight@pixel_master@41ch.xml",
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

	it("derives all eight layer blocks and the independent master from the running map", async () => {
		const server = stubServer();
		vi.stubGlobal("WebSocket", undefined);
		const output = server.outputs[0];
		const map = aDmxMap(output.id, output.name);
		const channel = map.channels[0];
		map.layerCount = 8;
		map.personality = "eightLayers";
		map.startAddress = 10;
		map.channels = Array.from({ length: 353 }, (_, index) => ({
			...channel,
			absoluteChannel: 10 + index,
			localOffset: index < 312 ? index % 39 : index - 312,
			group:
				index < 312
					? { kind: "layer" as const, number: Math.floor(index / 39) + 1 }
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
		).toEqual(["Layer 1", "3", "10", "48", "39"]);
		expect(
			[...rows[7].querySelectorAll("td")].map((cell) => cell.textContent),
		).toEqual(["Layer 8", "3", "283", "321", "39"]);
		expect(
			[...rows[8].querySelectorAll("td")].map((cell) => cell.textContent),
		).toEqual(["Master", "3", "322", "362", "41"]);
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
});
