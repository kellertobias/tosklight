import { cleanup, render as rtlRender, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryPage } from "../features/media-library/LibraryPage";
import { anOutput, stubServer } from "../testing/server";
import { previewWrites, restoreWrites } from "./libraryPreview";
import {
	PlaybackTakeoverProvider,
	PlaybackTakeoverToggle,
} from "./PlaybackTakeoverContext";

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

afterEach(() => {
	cleanup();
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

function aPlayingOutput() {
	const output = anOutput();
	output.master.dimmer = 0.5;
	output.layers[0].dimmer = 0.8;
	output.layers[1].dimmer = 0.6;
	output.layers[1].playModeDmx = 40;
	return output;
}

describe("the Library's live preview", () => {
	it("plays the chosen slot alone on Layer 1 and puts everything back when turned off", async () => {
		const server = stubServer({ outputs: [aPlayingOutput()] });
		render(
			<PlaybackTakeoverProvider>
				<PlaybackTakeoverToggle preview />
				<LibraryPage />
			</PlaybackTakeoverProvider>,
		);
		const preview = await screen.findByRole("switch", { name: "Enable preview" });
		expect(screen.getByRole("switch", { name: "Take over playback" })).toBeInTheDocument();
		// Choosing a slot while preview is off plays nothing.
		await userEvent.click((await screen.findByText("Static grid")).closest("button")!);
		expect(server.writes.some((path) => /layers|master/u.test(path))).toBe(false);

		await userEvent.click(preview);
		await waitFor(() => expect(server.outputs[0].layers.map((layer) => layer.dimmer)).toEqual([0, 0]));
		const output = server.outputs[0];
		expect(output.playbackTakeover).toBe(true);
		expect(output.master.dimmer).toBe(1);

		await userEvent.click(screen.getByText("Static grid").closest("button")!);
		await waitFor(() => expect(output.layers[0].dimmer).toBe(1));
		expect(output.layers[0].address).toMatchObject({ folder: 1, file: 2 });
		expect(output.layers[0].playModeDmx).toBe(0);
		expect(output.layers[1].dimmer).toBe(0);
		expect(output.master.dimmer).toBe(1);

		// A plain click on another slot switches the preview to it at once.
		await userEvent.click(screen.getByText("Blue haze").closest("button")!);
		await waitFor(() => expect(output.layers[0].address.file).toBe(1));

		await userEvent.click(preview);
		await waitFor(() => expect(output.playbackTakeover).toBe(false));
		expect(output.layers.map((layer) => layer.dimmer)).toEqual([0.8, 0.6]);
		expect(output.layers.map((layer) => layer.address.file)).toEqual([1, 2]);
		expect(output.layers[1].playModeDmx).toBe(40);
		expect(output.master.dimmer).toBe(0.5);
		expect(preview).not.toBeChecked();
	});

	it("keeps playback taken over after preview when it was taken over before", () => {
		const before = { ...aPlayingOutput(), playbackTakeover: true };
		const during = structuredClone(before);
		during.layers[0].address.file = 7;
		during.layers[0].dimmer = 1;
		during.layers[1].dimmer = 0;
		during.master.dimmer = 1;
		expect(restoreWrites(before, during)).toEqual({
			layers: [
				{ index: 0, change: { folder: 1, file: 1, dimmer: 0.8 } },
				{ index: 1, change: { dimmer: 0.6 } },
			],
			master: { dimmer: 0.5 },
		});
		expect(previewWrites(during, null)).toEqual({ layers: [{ index: 0, change: { dimmer: 0 } }], master: null });
	});
});
