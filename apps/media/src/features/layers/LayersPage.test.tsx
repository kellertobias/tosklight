import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { anOutput, stubServer } from "../../testing/server";
import { LayersPage } from "./LayersPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the layer page", () => {
	it("shows every layer of every output once the projection arrives", async () => {
		stubServer();
		render(<LayersPage />);

		expect(await screen.findByRole("heading", { name: "Main" })).toBeInTheDocument();
		expect(await screen.findByRole("article", { name: "Layer 1" })).toBeInTheDocument();
		expect(screen.getByRole("article", { name: "Layer 2" })).toBeInTheDocument();
	});

	it("sends only the dimmer when the dimmer moves, and leaves the selection alone", async () => {
		const server = stubServer();
		render(<LayersPage />);
		const sliders = await screen.findAllByLabelText("Dimmer");

		fireEvent.change(sliders[0], { target: { value: "40" } });

		await waitFor(() => expect(server.outputs[0].layers[0].dimmer).toBeCloseTo(0.4));
		expect(server.outputs[0].layers[0].address).toEqual({
			folder: 1,
			file: 1,
			class: "library",
		});
	});

	it("disables every control and says why while a desk owns the output", async () => {
		stubServer({ outputs: [anOutput({ dmxActive: true })] });
		render(<LayersPage />);

		expect(
			await screen.findByText(/a lighting desk is driving this output/iu),
		).toBeInTheDocument();
		const restart = await screen.findAllByRole("button", { name: "Restart media" });
		for (const button of restart) expect(button).toBeDisabled();
	});

	it("rolls a refused change back and explains the refusal", async () => {
		const server = stubServer();
		server.refuseWrites = {
			code: "dmx-owns-this",
			message: "a lighting desk is currently driving this output",
			status: 409,
		};
		render(<LayersPage />);

		const restart = await screen.findAllByRole("button", { name: "Restart media" });
		await userEvent.click(restart[0]);

		expect(
			await screen.findByText(/that change was not applied/iu),
		).toBeInTheDocument();
	});

	it("restarts a layer through the payload-free action the API exposes", async () => {
		const server = stubServer();
		render(<LayersPage />);

		const restart = await screen.findAllByRole("button", { name: "Restart media" });
		await userEvent.click(restart[0]);

		await waitFor(() =>
			expect(server.writes.some((path) => path.endsWith("/layers/0/reset"))).toBe(true),
		);
	});
});
