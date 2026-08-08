import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aCatalog, stubServer } from "../../testing/server";
import { LibraryPage } from "./LibraryPage";

afterEach(() => vi.unstubAllGlobals());

describe("the media library page", () => {
	it("lists every item under the address a desk would send", async () => {
		stubServer();
		render(<LibraryPage />);

		expect(await screen.findByRole("cell", { name: "001/001" })).toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "Blue haze" })).toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "001/002" })).toBeInTheDocument();
	});

	it("narrows to what a search matches, and says so when nothing does", async () => {
		stubServer();
		render(<LibraryPage />);
		await screen.findByRole("cell", { name: "Blue haze" });

		await userEvent.type(screen.getByLabelText("Search the library"), "grid");
		expect(screen.queryByRole("cell", { name: "Blue haze" })).not.toBeInTheDocument();
		expect(screen.getByRole("cell", { name: "Static grid" })).toBeInTheDocument();

		await userEvent.clear(screen.getByLabelText("Search the library"));
		await userEvent.type(screen.getByLabelText("Search the library"), "nothing here");
		expect(await screen.findByText(/nothing in the library matches/iu)).toBeInTheDocument();
	});

	it("says the library is empty rather than showing an empty table", async () => {
		stubServer({ catalog: { ...aCatalog(), itemCount: 0, folders: [] } });
		render(<LibraryPage />);

		expect(await screen.findByText(/nothing in the library can be played yet/iu)).toBeInTheDocument();
	});
});
