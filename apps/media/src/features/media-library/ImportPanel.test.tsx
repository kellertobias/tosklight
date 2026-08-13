import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

describe("importing into the library", () => {
	it("says what is waiting, and what it will become", async () => {
		stubServer();
		render(<LibraryPage />);

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
		render(<LibraryPage />);

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
		render(<LibraryPage />);
		await userEvent.click(
			await screen.findByRole("button", { name: "Convert 1 file" }),
		);
		await screen.findByRole("progressbar", { name: /Converting/u });

		await userEvent.click(screen.getByRole("button", { name: "Stop" }));

		await waitFor(() => expect(server.imports.jobs[0].state).toBe("cancelled"));
	});

	it("says a machine that cannot transcode cannot, before anything is queued", async () => {
		stubServer({ imports: anImportState({ canImport: false }) });
		render(<LibraryPage />);

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
						address: { folder: 1, file: 1, class: "library" },
						filename: "001.mov",
						state: "running",
						fraction: null,
						framesDone: 40,
						framesTotal: null,
						reason: null,
					},
				],
			}),
		});
		render(<LibraryPage />);

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
						address: { folder: 2, file: 1, class: "library" },
						filename: "001-Unknown.png",
						state: "failed",
						fraction: null,
						framesDone: null,
						framesTotal: null,
						reason: "the file has no video stream",
					},
				],
			}),
		});
		render(<LibraryPage />);

		expect(
			await screen.findByText("the file has no video stream"),
		).toBeInTheDocument();
		expect(screen.getByText("Failed")).toBeInTheDocument();
	});

	it("stays out of the way when there is nothing to import", async () => {
		stubServer({ imports: anImportState({ pending: [], jobs: [] }) });
		render(<LibraryPage />);

		// The library itself still renders; only the panel is absent.
		await screen.findByLabelText("Search Library");
		expect(
			screen.queryByRole("article", { name: "Import" }),
		).not.toBeInTheDocument();
	});
});
