import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { anOutput, aVisualizer, stubServer } from "../../testing/server";
import { VisualizersPage } from "./VisualizersPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the visualizers page", () => {
	it("shows each visualizer with the address a desk reaches it by", async () => {
		stubServer();
		render(<VisualizersPage />);

		expect(
			await screen.findByRole("button", { name: /Equalizer Bars/ }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("figure", { name: "Equalizer Bars preview" }),
		).toBeInTheDocument();
		expect(screen.getAllByText(/250\/0*1/)).not.toHaveLength(0);
		expect(
			screen.getByRole("heading", { name: "Equalizer Bars" }),
		).toBeInTheDocument();
	});

	it("does not offer a separate action that puts a visualizer on an output", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await screen.findByRole("button", { name: /Equalizer Bars/ });
		expect(
			screen.queryByRole("button", { name: /select|put/iu }),
		).not.toBeInTheDocument();
		expect(server.outputs[0].layers[0].address).not.toMatchObject({
			folder: 250,
			file: 1,
		});
	});

	it("tunes a visualizer through the edit path, with a request id", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Tune visualizer" }),
		);
		const name = screen.getByLabelText("Name");
		await userEvent.clear(name);
		await userEvent.type(name, "House bars");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() => expect(server.visualizers[0].name).toBe("House bars"));
		const edit = server.writes.find((path) => path.includes("/visualizers/"));
		expect(edit).toBe("/visualizers/250/1/update");
	});

	it("only offers the controls the kind actually reads", async () => {
		stubServer();
		render(<VisualizersPage />);
		await userEvent.click(
			await screen.findByRole("button", { name: "Tune visualizer" }),
		);

		// The stub publishes count, size, and primary — and nothing else should appear.
		expect(screen.getByLabelText("Count")).toBeInTheDocument();
		expect(screen.getByLabelText("Size")).toBeInTheDocument();
		expect(screen.queryByLabelText("Iterations")).not.toBeInTheDocument();
		expect(screen.queryByLabelText("Gravity")).not.toBeInTheDocument();
	});

	it("says why an edit that could not be stored was not applied", async () => {
		const server = stubServer();
		server.refuseWrites = {
			code: "configuration-not-written",
			message: "the change could not be saved; it has not been applied",
			status: 500,
		};
		render(<VisualizersPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Tune visualizer" }),
		);
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		expect(await screen.findByText(/could not be saved/iu)).toBeInTheDocument();
	});

	it("keeps output-selection absent while a desk is driving the output", async () => {
		stubServer({
			outputs: [anOutput({ dmxActive: true })],
			visualizers: [aVisualizer()],
		});
		render(<VisualizersPage />);

		await screen.findByRole("button", { name: /Equalizer Bars/ });
		expect(
			screen.queryByRole("button", { name: /select|put/iu }),
		).not.toBeInTheDocument();
	});
});
