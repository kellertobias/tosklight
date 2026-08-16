import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { stubServer } from "../../testing/server";
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
});

describe("DMX diagnostics", () => {
	it("is a dedicated Diagnostics window with a link to DMX input settings", async () => {
		stubServer();
		vi.stubGlobal("WebSocket", undefined);
		const { container } = render(<DmxPage />);

		expect(screen.getByText("Diagnostics")).toHaveClass("ui-window-title");
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
		expect(
			await screen.findByRole("link", {
				name: "Download ToskLight Media Layer.gdtf",
			}),
		).toHaveAttribute(
			"href",
			"/api/v2/fixtures/ToskLight%20Media%20Layer.gdtf",
		);
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
