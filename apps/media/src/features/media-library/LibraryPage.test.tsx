import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aCatalog, stubServer } from "../../testing/server";
import { LibraryPage } from "./LibraryPage";

afterEach(() => vi.unstubAllGlobals());

describe("the media library page", () => {
	it("lists every item under the address a desk would send", async () => {
		stubServer();
		render(<LibraryPage />);

		expect(
			await screen.findByRole("cell", { name: "001/001" }),
		).toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "Blue haze" })).toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "001/002" })).toBeInTheDocument();
	});

	it("narrows to what a search matches, and says so when nothing does", async () => {
		stubServer();
		render(<LibraryPage />);
		await screen.findByRole("cell", { name: "Blue haze" });

		await userEvent.type(screen.getByLabelText("Search the library"), "grid");
		expect(
			screen.queryByRole("cell", { name: "Blue haze" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("cell", { name: "Static grid" }),
		).toBeInTheDocument();

		await userEvent.clear(screen.getByLabelText("Search the library"));
		await userEvent.type(
			screen.getByLabelText("Search the library"),
			"nothing here",
		);
		expect(
			await screen.findByText(/nothing in the library matches/iu),
		).toBeInTheDocument();
	});

	it("says the library is empty rather than showing an empty table", async () => {
		stubServer({ catalog: { ...aCatalog(), itemCount: 0, folders: [] } });
		render(<LibraryPage />);

		expect(
			await screen.findByText(/nothing in the library can be played yet/iu),
		).toBeInTheDocument();
	});

	it("renames a clip through its stable catalog identity", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const row = (
			await screen.findByRole("cell", { name: "Blue haze" })
		).closest("tr");
		expect(row).not.toBeNull();
		await userEvent.click(screen.getAllByRole("button", { name: "Rename" })[0]);
		const editor = screen
			.getByRole("button", { name: "Save name" })
			.closest("form");
		const name = editor?.querySelector("input");
		expect(name).toBeInstanceOf(HTMLInputElement);
		if (!name) throw new Error("rename input missing");
		await userEvent.clear(name);
		await userEvent.type(name, "Opening haze");
		await userEvent.click(screen.getByRole("button", { name: "Save name" }));

		expect(server.writes[0]).toBe("/library/items/asset-a/update");
		expect(
			await screen.findByRole("cell", { name: "Opening haze" }),
		).toBeInTheDocument();
	});

	it("changes and clears the visible folder name", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		await screen.findByRole("heading", { name: "001 · Looks" });
		await userEvent.click(
			screen.getByRole("button", { name: "Change folder name" }),
		);
		await userEvent.clear(screen.getByLabelText("Folder 1 name"));
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		expect(server.catalog.folders[0].name).toBeNull();
		expect(
			await screen.findByRole("heading", { name: "001" }),
		).toBeInTheDocument();
	});

	it("uploads one chosen source to an explicit unused address", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		await screen.findByRole("heading", { name: "Upload media" });
		await userEvent.clear(screen.getByLabelText("File"));
		await userEvent.type(screen.getByLabelText("File"), "7");
		await userEvent.type(screen.getByLabelText("Media name"), "Opening");
		await userEvent.upload(
			screen.getByLabelText("Source file"),
			new File(["pixels"], "opening.png", { type: "image/png" }),
		);
		const upload = screen.getByRole("button", { name: "Upload and import" });
		expect(upload).toBeEnabled();
		const form = upload.closest("form");
		expect(form).not.toBeNull();
		if (form) fireEvent.submit(form);

		await vi.waitFor(() =>
			expect(server.writes[0]).toMatch(/^\/library\/1\/7\/upload\?/u),
		);
	});
});
