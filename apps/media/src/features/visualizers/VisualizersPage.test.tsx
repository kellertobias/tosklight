import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	anOutput,
	anOutputConfiguration,
	aVisualizer,
	stubServer,
} from "../../testing/server";
import {
	BUILTIN_VISUALIZER_KINDS,
	hasRenderedVisualizerPreview,
} from "./preview";
import { VisualizersPage } from "./VisualizersPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the visualizers page", () => {
	it("shows each visualizer with the address a desk reaches it by", async () => {
		stubServer();
		const { container } = render(<VisualizersPage />);

		expect(
			(await screen.findAllByRole("button", { name: /Equalizer Bars/ }))[0],
		).toBeInTheDocument();
		expect(container.querySelector(".media-library-folder-pool")).toHaveClass(
			"ui-button-grid",
			"square-grid",
		);
		expect(
			screen.getByRole("figure", { name: "Equalizer Bars preview" }),
		).toBeInTheDocument();
		expect(screen.getAllByText(/250\/0*1/)).not.toHaveLength(0);
		expect(screen.getByLabelText("Built-in visualizer")).toBeInTheDocument();
		expect(screen.getByLabelText("Name")).toHaveValue("Equalizer Bars");
		expect(screen.getByLabelText("Bloom")).toHaveValue("1");
		expect(
			document.querySelector(".media-library-pool .ui-button-grid"),
		).toHaveClass("media-file-pool-grid", "media-library-file-pool-grid");
	});

	it("has an actual renderer frame for every shipped built-in kind", () => {
		expect(BUILTIN_VISUALIZER_KINDS).toHaveLength(23);
		expect(
			BUILTIN_VISUALIZER_KINDS.every(({ typeId }) =>
				hasRenderedVisualizerPreview(typeId),
			),
		).toBe(true);
	});

	it("offers Matrix Digital Rain as a standalone tunable visualizer", async () => {
		stubServer({
			visualizers: [
				aVisualizer({
					typeId: 42,
					kind: "Matrix Digital Rain",
					name: "Matrix Digital Rain",
					uses: ["count", "speed", "amount", "primary", "secondary"],
				}),
			],
		});
		render(<VisualizersPage />);

		expect(
			(
				await screen.findAllByRole("button", { name: /Matrix Digital Rain/iu })
			)[0],
		).toBeInTheDocument();
		expect(screen.getByLabelText("Count")).toBeInTheDocument();
		expect(screen.getByLabelText("Speed")).toBeInTheDocument();
		expect(screen.getByLabelText("Amount")).toBeInTheDocument();
		expect(
			screen.getAllByAltText("Matrix Digital Rain preview")[0],
		).toHaveAttribute(
			"src",
			expect.stringMatching(/042-matrix-digital-rain\.png$/u),
		);
	});

	it("frames previews at the configured main-output ratio", async () => {
		const output = anOutput();
		stubServer({
			outputs: [output],
			outputConfigurations: {
				[output.id]: anOutputConfiguration(output.id, output.name, {
					width: 1000,
					height: 1000,
				}),
			},
		});
		render(<VisualizersPage />);

		const preview = await screen.findByRole("figure", {
			name: "Equalizer Bars preview",
		});
		await waitFor(() =>
			expect(preview.querySelector(".media-preview-picture")).toHaveStyle({
				aspectRatio: "1",
			}),
		);
		expect(screen.getAllByAltText("Equalizer Bars preview")[0]).toHaveClass(
			"pool-card-image",
		);
		expect(screen.getAllByAltText("Equalizer Bars preview")[0]).toHaveAttribute(
			"src",
			expect.stringMatching(/000-equalizer-bars\.png$/u),
		);
		expect(preview.querySelector("img")).toHaveAttribute(
			"src",
			expect.stringMatching(/000-equalizer-bars\.png$/u),
		);
	});

	it("does not offer a separate action that puts a visualizer on an output", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await screen.findAllByRole("button", { name: /Equalizer Bars/ });
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

		const name = await screen.findByLabelText("Name");
		await userEvent.clear(name);
		await userEvent.type(name, "House bars");
		await waitFor(() => expect(server.visualizers[0].name).toBe("House bars"));
		expect(
			screen.queryByRole("button", { name: "Save" }),
		).not.toBeInTheDocument();
		const edit = server.writes.find((path) => path.includes("/visualizers/"));
		expect(edit).toBe("/visualizers/250/1/update");
	});

	it("only offers the controls the kind actually reads", async () => {
		stubServer();
		render(<VisualizersPage />);
		await screen.findByLabelText("Name");

		// The stub publishes count, size, and primary — and nothing else should appear.
		expect(screen.getByLabelText("Count")).toBeInTheDocument();
		expect(screen.getByLabelText("Size")).toBeInTheDocument();
		expect(screen.queryByLabelText("Iterations")).not.toBeInTheDocument();
		expect(screen.queryByLabelText("Gravity")).not.toBeInTheDocument();
	});

	it("configures Grid Landscape roadside scenery independently", async () => {
		const base = aVisualizer();
		const server = stubServer({
			visualizers: [
				aVisualizer({
					kind: "Grid Landscape",
					name: "Grid Landscape",
					typeId: 53,
					uses: [
						"speed",
						"count",
						"size",
						"radius",
						"amount",
						"primary",
						"secondary",
						"mode",
						"iterations",
					],
					parameters: { ...base.parameters, mode: 1, iterations: 2 },
				}),
			],
		});
		render(<VisualizersPage />);
		await screen.findByLabelText("Name");

		await chooseOption("Left scenery", "Palm trees");
		await waitFor(() => expect(server.visualizers[0].parameters.mode).toBe(2));
		await chooseOption("Right scenery", "Off");
		await waitFor(() =>
			expect(server.visualizers[0].parameters.iterations).toBe(0),
		);
		expect(server.visualizers[0].parameters.mode).toBe(2);
	});

	it("says why an edit that could not be stored was not applied", async () => {
		const server = stubServer();
		server.refuseWrites = {
			code: "configuration-not-written",
			message: "the change could not be saved; it has not been applied",
			status: 500,
		};
		render(<VisualizersPage />);

		const name = await screen.findByLabelText("Name");
		await userEvent.type(name, " refused");

		expect(await screen.findByText(/could not be saved/iu)).toBeInTheDocument();
	});

	it("creates another independent instance from a built-in visualizer", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "New visualizer" }),
		);
		expect(
			screen.getByRole("figure", { name: "Empty visualizer preview" }),
		).toHaveTextContent("250/002");
		await userEvent.type(screen.getByLabelText("Name"), "Thin bars");
		await userEvent.click(
			screen.getByRole("button", { name: "Built-in visualizer" }),
		);
		await userEvent.click(
			screen.getByRole("option", { name: "Equalizer Bars" }),
		);

		await waitFor(() => expect(server.visualizers).toHaveLength(2));
		expect(server.visualizers[1]).toMatchObject({
			typeId: 0,
			name: "Thin bars",
			address: { folder: 250, file: 2 },
		});
		expect(server.writes).toContain("/visualizers/create");
		expect(server.writeBodies).toContainEqual(
			expect.objectContaining({ name: "Thin bars", folder: 250, file: 2 }),
		);
	});

	it("uses shared colour, checkbox, and switch controls", async () => {
		stubServer({
			visualizers: [aVisualizer({ uses: ["primary", "mirror", "wireframe"] })],
		});
		render(<VisualizersPage />);

		const colour = await screen.findByRole("button", { name: /#1ad6ed/iu });
		expect(colour).toHaveAttribute("aria-haspopup", "listbox");
		expect(screen.getByLabelText("Mirror")).toHaveAttribute("type", "checkbox");
		expect(
			screen.getByRole("switch", { name: "Wireframe" }),
		).toBeInTheDocument();
	});

	it("uses one address line, a responsive identity row, separator, and sticky preview", async () => {
		stubServer();
		render(<VisualizersPage />);

		const preview = await screen.findByRole("figure", {
			name: "Equalizer Bars preview",
		});
		expect(preview).toHaveTextContent("250/001");
		expect(preview.parentElement).toHaveClass("media-generated-sticky-preview");
		const identity = document.querySelector(".media-source-identity-grid");
		expect(identity).toContainElement(screen.getByLabelText("Name"));
		expect(identity).toHaveTextContent("Built-in visualizer");
		expect(
			document.querySelector(".media-source-editor-separator"),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("heading", { name: "Equalizer Bars" }),
		).not.toBeInTheDocument();
	});

	it("puts New visualizer before the library mode tabs", async () => {
		stubServer();
		render(<VisualizersPage />);

		const action = await screen.findByRole("button", {
			name: "New visualizer",
		});
		const tabs = screen.getByRole("tablist");
		expect(
			action.compareDocumentPosition(tabs) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("opens a generated folder inspector without selecting its first source", async () => {
		const server = stubServer();
		render(<VisualizersPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: /251Visualizers0\/254/u }),
		);

		expect(
			screen.getByRole("heading", { name: "Configure folder" }),
		).toBeInTheDocument();
		expect(screen.getAllByText("Folder 251")).not.toHaveLength(0);
		await userEvent.type(screen.getByLabelText("Folder name"), "Motion");
		await waitFor(() =>
			expect(
				server.folderPresentations.folders.find(
					(candidate) => candidate.folder === 251,
				)?.name,
			).toBe("Motion"),
		);
	});

	it("keeps output-selection absent while a desk is driving the output", async () => {
		stubServer({
			outputs: [anOutput({ dmxActive: true })],
			visualizers: [aVisualizer()],
		});
		render(<VisualizersPage />);

		await screen.findAllByRole("button", { name: /Equalizer Bars/ });
		expect(
			screen.queryByRole("button", { name: /select|put/iu }),
		).not.toBeInTheDocument();
	});
});

async function chooseOption(labelText: string, option: string) {
	const label = screen.getByText(labelText, { selector: "label" });
	const trigger = label.parentElement?.querySelector<HTMLButtonElement>(
		'button[aria-haspopup="listbox"]',
	);
	expect(trigger).toBeTruthy();
	await userEvent.click(trigger as HTMLButtonElement);
	await userEvent.click(screen.getByRole("option", { name: option }));
}
