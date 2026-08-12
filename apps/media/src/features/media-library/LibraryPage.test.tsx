import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aCatalog, stubServer } from "../../testing/server";
import { allocateFreeAddresses, LibraryPage } from "./LibraryPage";

afterEach(() => vi.unstubAllGlobals());

describe("the CITP media library", () => {
	it("shows the whole 255-folder address space and the selected folder pool", async () => {
		stubServer();
		render(<LibraryPage />);
		expect(
			(await screen.findByText("Looks")).closest("button"),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /199Empty folder/u }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /200Text sourcesReserved/u }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /255VisualizersReserved/u }),
		).toBeInTheDocument();
		expect(screen.getByText("Blue haze").closest("button")).toBeInTheDocument();
	});

	it("opens one media item in the preview editor and persists name and BPM", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const media = (await screen.findByText("Blue haze")).closest("button");
		if (!media) throw new Error("media card missing");
		await userEvent.click(media);
		expect(
			screen.getByRole("img", { name: "Blue haze preview" }),
		).toHaveAttribute("src", "/api/v2/library/1/1/thumbnail");
		const editor = screen
			.getByRole("button", { name: "Save media" })
			.closest("form");
		const inputs = editor?.querySelectorAll("input");
		if (!inputs || inputs.length < 2)
			throw new Error("media editor fields missing");
		await userEvent.clear(inputs[0]);
		await userEvent.type(inputs[0], "Opening haze");
		await userEvent.clear(inputs[1]);
		await userEvent.type(inputs[1], "128");
		await userEvent.click(screen.getByRole("button", { name: "Save media" }));
		await vi.waitFor(() => expect(server.writes).toHaveLength(2));
		expect(server.catalog.folders[0].items[0]).toMatchObject({
			name: "Opening haze",
			intrinsicBpm: 128,
		});
	});

	it("right-clicking a media folder opens its configuration", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const folder = (await screen.findByText("Looks")).closest("button");
		if (!folder) throw new Error("folder button missing");
		fireEvent.contextMenu(folder);
		await userEvent.clear(screen.getByLabelText("Folder name"));
		await userEvent.type(screen.getByLabelText("Folder name"), "Act one");
		await userEvent.click(screen.getByRole("button", { name: "Save folder" }));
		await vi.waitFor(() =>
			expect(server.catalog.folders[0].name).toBe("Act one"),
		);
	});

	it("allocates at the first free slot and rolls into the next folder", () => {
		const catalog = aCatalog();
		catalog.folders[0].items = Array.from({ length: 254 }, (_, index) => ({
			...catalog.folders[0].items[0],
			id: `item-${index + 1}`,
			file: index + 1,
		}));
		expect(allocateFreeAddresses(catalog, 1, [], 2)).toEqual([
			{ folder: 2, file: 1 },
			{ folder: 2, file: 2 },
		]);
	});
});
