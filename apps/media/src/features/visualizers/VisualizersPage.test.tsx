import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aVisualizer, anOutput, stubServer } from "../../testing/server";
import { VisualizersPage } from "./VisualizersPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the visualizers page", () => {
	it("shows each visualizer with the address a desk reaches it by", async () => {
		stubServer();
		render(<VisualizersPage />);

		expect(await screen.findByRole("article", { name: "Equalizer Bars" })).toBeInTheDocument();
		expect(screen.getByText("220/001")).toBeInTheDocument();
		expect(screen.getByText(/type 0/u)).toBeInTheDocument();
	});

	it("selects a visualizer onto a layer by its address", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await userEvent.click(await screen.findByRole("button", { name: "Select" }));

		await waitFor(() =>
			expect(server.outputs[0].layers[0].address).toMatchObject({ folder: 220, file: 1 }),
		);
	});

	it("cannot select onto an output a desk is driving", async () => {
		stubServer({
			outputs: [anOutput({ dmxActive: true })],
			visualizers: [aVisualizer()],
		});
		render(<VisualizersPage />);

		expect(await screen.findByRole("button", { name: "Select" })).toBeDisabled();
	});
});
