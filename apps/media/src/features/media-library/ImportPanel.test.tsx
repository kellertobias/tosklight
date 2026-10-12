import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../shared/api/client";
import { anImportState, stubServer } from "../../testing/server";
import { LibraryPage } from "./LibraryPage";

// The panel opens a telemetry socket for job progress. These tests are about what it shows and
// what it starts, so the socket never connects — which is also the case worth covering: the
// snapshot has to be enough on its own.
beforeEach(() => {
	vi.stubGlobal("WebSocket", undefined);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function observeImportRead(gate = Promise.resolve()) {
	const imports = api.imports;
	const started = deferred();
	const completed = deferred();
	vi.spyOn(api, "imports").mockImplementation(async () => {
		started.resolve();
		const snapshot = await imports();
		await gate;
		completed.resolve();
		return snapshot;
	});
	return { started: started.promise, completed: completed.promise };
}

async function renderHydratedLibrary(read = observeImportRead()) {
	// Catalog hydration mounts the panel; its second snapshot must settle before
	// role queries scan the full folder/file grids. This is not a timing budget.
	let view!: ReturnType<typeof render>;
	await act(async () => {
		view = render(<LibraryPage />);
	});
	await act(async () => {
		await read.completed;
	});
	return view;
}

describe("importing into the library", () => {
	it("waits for the imports snapshot before asserting the full Library", async () => {
		stubServer();
		const gate = deferred();
		const read = observeImportRead(gate.promise);
		let finished = false;
		const hydrated = renderHydratedLibrary(read).then(() => {
			finished = true;
		});
		await read.started;
		expect(finished).toBe(false);
		gate.resolve();
		await hydrated;

		const panel = screen.getByRole("article", { name: "Import" });
		expect(panel).toHaveTextContent("004-LoopTest.mp4");
		expect(panel).toHaveTextContent("001/004");
		expect(screen.getByLabelText("Search Library")).toBeInTheDocument();
	});

	it("cancels polling and ignores a snapshot completed after unmount", async () => {
		stubServer();
		const gate = deferred();
		const read = observeImportRead(gate.promise);
		const intervals = vi.spyOn(globalThis, "setInterval");
		const clear = vi.spyOn(globalThis, "clearInterval");
		let view!: ReturnType<typeof render>;
		await act(async () => {
			view = render(<LibraryPage />);
		});
		await read.started;
		const pollIndex = intervals.mock.calls.findIndex(
			(call) => call[1] === 5_000,
		);
		expect(pollIndex).toBeGreaterThanOrEqual(0);
		const poll = intervals.mock.results[pollIndex].value;
		view.unmount();
		expect(clear).toHaveBeenCalledWith(poll);
		await act(async () => {
			gate.resolve();
			await read.completed;
		});
		expect(view.container).toBeEmptyDOMElement();
		expect(api.imports).toHaveBeenCalledTimes(1);
	});

	it("says what is waiting, and what it will become", async () => {
		stubServer();
		await renderHydratedLibrary();

		const panel = await screen.findByRole("article", { name: "Import" });
		expect(panel).toHaveTextContent("004-LoopTest.mp4");
		expect(panel).toHaveTextContent("001/004");
		expect(panel).toHaveTextContent("LoopTest");
		expect(
			screen.getByText(/Converting them leaves the originals where they are/u),
		).toBeInTheDocument();
	});

	it("converts everything waiting and shows the work happening", async () => {
		const server = stubServer();
		await renderHydratedLibrary();

		await userEvent.click(
			await screen.findByRole("button", { name: "Convert 1 file" }),
		);

		await waitFor(() =>
			expect(
				screen.getByRole("progressbar", { name: /Converting/u }),
			).toBeInTheDocument(),
		);
		expect(server.writes).toContain("/library/import");
		expect(server.imports.pending).toHaveLength(0);
	});

	it("stops an import that is still going", async () => {
		const server = stubServer();
		await renderHydratedLibrary();
		await userEvent.click(
			await screen.findByRole("button", { name: "Convert 1 file" }),
		);
		await screen.findByRole("progressbar", { name: /Converting/u });

		await userEvent.click(screen.getByRole("button", { name: "Stop" }));

		await waitFor(() => expect(server.imports.jobs[0].state).toBe("cancelled"));
	});

	it("says a machine that cannot transcode cannot, before anything is queued", async () => {
		stubServer({ imports: anImportState({ canImport: false }) });
		await renderHydratedLibrary();

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"FFmpeg is not installed",
		);
		expect(
			screen.getByRole("button", { name: "Convert 1 file" }),
		).toBeDisabled();
	});

	it("draws an indeterminate bar rather than inventing a percentage", async () => {
		stubServer({
			imports: anImportState({
				pending: [],
				jobs: [
					{
						id: "job-0",
						batchId: null,
						address: { folder: 1, file: 1, class: "library" },
						filename: "001.mov",
						state: "running",
						attempts: 1,
						fraction: null,
						framesDone: 40,
						framesTotal: null,
						reason: null,
					},
				],
			}),
		});
		await renderHydratedLibrary();

		const bar = await screen.findByRole("progressbar", { name: /Converting/u });
		expect(bar).not.toHaveAttribute("aria-valuenow");
	});

	it("keeps a failed import's reason on screen", async () => {
		stubServer({
			imports: anImportState({
				pending: [],
				jobs: [
					{
						id: "job-0",
						batchId: "batch-1",
						address: { folder: 2, file: 1, class: "library" },
						filename: "001-Unknown.png",
						state: "failed",
						attempts: 2,
						fraction: null,
						framesDone: null,
						framesTotal: null,
						reason: "the file has no video stream",
					},
				],
			}),
		});
		await renderHydratedLibrary();

		expect(
			await screen.findByText("the file has no video stream"),
		).toBeInTheDocument();
		expect(screen.getByText("Failed")).toBeInTheDocument();
		expect(screen.getByText("Conversion report")).toBeInTheDocument();
		expect(screen.getByText("0 converted, 1 failed.")).toBeInTheDocument();
		expect(
			screen.getByText(/Conversion failures were retried once/u),
		).toBeInTheDocument();
	});

	it("identifies the retry while a native batch is still converting", async () => {
		stubServer({
			imports: anImportState({
				pending: [],
				jobs: [
					{
						id: "job-retry",
						batchId: "batch-2",
						address: { folder: 7, file: 254, class: "library" },
						filename: "Loop.mov",
						state: "running",
						attempts: 2,
						fraction: null,
						framesDone: 0,
						framesTotal: null,
						reason: null,
					},
				],
			}),
		});
		await renderHydratedLibrary();

		expect(
			await screen.findByText(/Loop\.mov · Retrying/u),
		).toBeInTheDocument();
		expect(screen.getByText("Conversion progress")).toBeInTheDocument();
		expect(
			screen.getByRole("progressbar", { name: "Overall conversion progress" }),
		).toHaveAttribute("value", "0");
	});

	it("stays out of the way when there is nothing to import", async () => {
		stubServer({ imports: anImportState({ pending: [], jobs: [] }) });
		await renderHydratedLibrary();

		// The library itself still renders; only the panel is absent.
		await screen.findByLabelText("Search Library");
		expect(
			screen.queryByRole("article", { name: "Import" }),
		).not.toBeInTheDocument();
	});
});
