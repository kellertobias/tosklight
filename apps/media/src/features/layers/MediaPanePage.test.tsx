import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { anOutput, stubServer } from "../../testing/server";
import { MediaPanePage } from "./MediaPanePage";

afterEach(() => {
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
	it("paints takeover feedback inside the 50 ms operator budget while the request is still pending", async () => {
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

		expect(takeover).toBeChecked();
		expect(paintedAt - inputAt).toBeLessThan(50);
		expect(server.outputs[0].playbackTakeover).toBe(false);

		request.resolve();
		await waitFor(() => expect(server.outputs[0].playbackTakeover).toBe(true));
	});

	it("keeps rapid fader feedback immediate and serializes authoritative writes", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(screen.getByRole("radio", { name: "Frame" }));

		const request = deferred();
		server.holdWrites = request.promise;
		const scale = screen.getByLabelText("Scale X");
		const inputAt = performance.now();
		fireEvent.input(scale, { target: { value: "2" } });
		fireEvent.input(scale, { target: { value: "3" } });
		const paintedAt = performance.now();

		expect(scale).toHaveValue("3");
		expect(paintedAt - inputAt).toBeLessThan(50);
		await waitFor(() => expect(server.writes).toHaveLength(2));
		// Takeover is the first write; only one layer write reaches the held server at a time.
		expect(
			server.writes.filter((path) => path.endsWith("/layers/0/update")),
		).toHaveLength(1);

		request.resolve();
		await waitFor(() => expect(server.outputs[0].layers[0].scaleX).toBe(3));
		expect(
			server.writes.filter((path) => path.endsWith("/layers/0/update")),
		).toHaveLength(2);
	});

	it("puts Release on the off side and Take over playback on the on side", async () => {
		stubServer();
		render(<MediaPanePage />);

		await screen.findByRole("switch", { name: "Take over playback" });
		expect(document.querySelector(".ui-switch-state-off")).toHaveTextContent(
			"Release",
		);
		expect(document.querySelector(".ui-switch-state-on")).toHaveTextContent(
			"Take over playback",
		);
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
		await userEvent.click(screen.getByRole("radio", { name: "Colour" }));
		expect(
			screen.getByRole("radiogroup", { name: "Flip / mirror" }),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("radio", { name: "both" }));
		await waitFor(() => expect(second.master.flipMirror).toBe("both"));
	});

	it("keeps takeover failure on the selected output and makes effects explicit", async () => {
		const server = stubServer();
		server.refuseWrites = {
			code: "refused",
			message: "takeover refused",
			status: 409,
		};
		render(<MediaPanePage />);

		expect((await screen.findAllByText("Unsupported")).length).toBeGreaterThan(
			0,
		);
		await userEvent.click(
			screen.getByRole("switch", { name: "Take over playback" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"takeover refused",
		);
		expect(server.outputs[0].playbackTakeover).toBe(false);
	});

	it("lists generated Text and Visualizer address folders beside the media library", async () => {
		stubServer();
		render(<MediaPanePage />);
		expect(await screen.findByText("Text")).toBeInTheDocument();
		expect(screen.getByText("Visualizers")).toBeInTheDocument();
	});

	it("keeps network-controlled choices disabled and sends no writes in monitor mode", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("radio", { name: "Playback" }),
		);
		const modes = screen.getByRole("radiogroup", { name: "Play mode" });
		for (const option of modes.querySelectorAll("button"))
			expect(option).toBeDisabled();
		expect(server.writes).toEqual([]);
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
		await userEvent.click(screen.getByRole("radio", { name: "Frame" }));
		const scale = screen.getByLabelText("Scale X");
		fireEvent.input(scale, { target: { value: "6" } });
		await waitFor(() => expect(server.outputs[0].layers[0].scaleX).toBe(6));
		expect(scale).toHaveAttribute("max", "10");
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

	it("routes a master mask selection to the master endpoint", async () => {
		const server = stubServer();
		render(<MediaPanePage />);
		await userEvent.click(
			await screen.findByRole("switch", { name: "Take over playback" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: /Master output/iu }),
		);
		await userEvent.click(screen.getByRole("radio", { name: "Mask" }));
		await userEvent.click(
			await screen.findByRole("button", { name: /001Blue haze/iu }),
		);
		await waitFor(() =>
			expect(
				server.writes.some((path) => path.endsWith("/master/update")),
			).toBe(true),
		);
		expect(
			server.writes.some((path) => path.endsWith("/layers/0/update")),
		).toBe(false);
	});
});

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}
