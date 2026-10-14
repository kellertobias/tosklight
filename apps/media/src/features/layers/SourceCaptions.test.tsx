import { act, render, waitFor } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { KEYS } from "../../shared/api/queries";
import { writeResource } from "../../shared/api/resource";
import {
	aClock,
	anOutput,
	aVisualizer,
	stubServer,
} from "../../testing/server";
import { LayersPage } from "./LayersPage";
import { MediaPanePage } from "./MediaPanePage";

beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});
afterEach(() => {
	document.getElementById("media-playback-dock-action")?.remove();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const text = aClock({
	address: { folder: 200, file: 5, class: "text-bank" },
	name: "ReleaseAcceptanceBlueBar",
});
const visualizer = aVisualizer({
	address: { folder: 250, file: 24, class: "generated-visualizer" },
	name: "Matrix Digital Rain",
	channels: [],
});
const captions = (container: HTMLElement) =>
	Array.from(
		container.querySelectorAll(".media-layer-source"),
		(element) => element.textContent?.split(" · not drawing")[0],
	);

for (const [name, Page] of [
	["layer cards", LayersPage],
	["Media pane", MediaPanePage],
] as const) {
	describe(`${name} source captions`, () => {
		it("names configured Text and VIS without changing output or requesting per layer", async () => {
			const output = anOutput();
			output.layers[0].address = text.address;
			output.layers[1].address = visualizer.address;
			const server = stubServer({
				outputs: [output],
				text: [text],
				visualizers: [visualizer],
			});
			const original = structuredClone(server.outputs[0]);
			const { container } = render(<Page />, { wrapper: ModalProvider });
			await waitFor(() =>
				expect(captions(container)).toEqual([text.name, visualizer.name]),
			);
			expect(server.outputs[0]).toEqual(original);
			expect(server.writes).toEqual([]);
			expect(
				server.requests.filter((path) => path.endsWith("/text")),
			).toHaveLength(1);
			expect(
				server.requests.filter((path) => path.endsWith("/visualizers")),
			).toHaveLength(1);
		});
		it("distinguishes blank, unresolved nonblank, and uploaded media", async () => {
			const output = anOutput();
			const template = structuredClone(output.layers[0]);
			output.layerCount = 4;
			output.layers = [
				{
					...structuredClone(template),
					index: 0,
					address: { folder: 3, file: 0, class: "blank" },
				},
				{
					...structuredClone(template),
					index: 1,
					address: { folder: 200, file: 99, class: "text-bank" },
				},
				{
					...structuredClone(template),
					index: 2,
					address: { folder: 250, file: 99, class: "generated-visualizer" },
				},
				{
					...structuredClone(template),
					index: 3,
					address: { folder: 1, file: 1, class: "library" },
				},
			];
			stubServer({ outputs: [output] });
			const { container } = render(<Page />, { wrapper: ModalProvider });
			await waitFor(() =>
				expect(captions(container)).toEqual([
					"Nothing selected",
					"200/099",
					"250/099",
					"Blue haze",
				]),
			);
		});
		it("hydrates and renames source snapshots without extra requests", async () => {
			const output = anOutput();
			output.layers[0].address = text.address;
			output.layers[1].address = visualizer.address;
			const server = stubServer({
				outputs: [output],
				text: [],
				visualizers: [],
			});
			const { container } = render(<Page />, { wrapper: ModalProvider });
			await waitFor(() =>
				expect(captions(container)).toEqual(["200/005", "250/024"]),
			);
			const requests = [...server.requests];
			act(() => {
				writeResource(KEYS.text, [text]);
				writeResource(KEYS.visualizers, [visualizer]);
			});
			await waitFor(() =>
				expect(captions(container)).toEqual([text.name, visualizer.name]),
			);
			act(() => {
				writeResource(KEYS.text, [{ ...text, name: "Renamed text" }]);
				writeResource(KEYS.visualizers, [
					{ ...visualizer, name: "Renamed VIS" },
				]);
			});
			await waitFor(() =>
				expect(captions(container)).toEqual(["Renamed text", "Renamed VIS"]),
			);
			expect(server.requests).toEqual(requests);
			expect(server.writes).toEqual([]);
		});
	});
}
