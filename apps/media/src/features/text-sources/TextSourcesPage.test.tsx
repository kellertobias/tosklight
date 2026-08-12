import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aClock, aCountdown, stubServer } from "../../testing/server";
import { nextFreeAddress, TextSourcesPage } from "./TextSourcesPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the text sources page", () => {
	it("shows each source with the address a desk reaches it by", async () => {
		stubServer();
		render(<TextSourcesPage />);

		expect(
			await screen.findByRole("button", { name: /Clock/ }),
		).toBeInTheDocument();
		expect(screen.getAllByText("200/001")).not.toHaveLength(0);
		expect(
			screen.getByRole("button", { name: /Ten minutes/ }),
		).toHaveTextContent("600 s");
	});

	it("keeps playback selection on the Playback screen", async () => {
		const server = stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		await screen.findByRole("button", { name: /Clock/ });
		expect(
			screen.queryByRole("button", { name: /select on/iu }),
		).not.toBeInTheDocument();
		expect(server.outputs[0].layers[0].address).not.toMatchObject({
			folder: 200,
			file: 1,
		});
	});

	it("only asks for the payload the chosen kind has", async () => {
		stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change" }),
		);
		// A clock has no words and no length.
		expect(screen.queryByLabelText("Words")).not.toBeInTheDocument();
		expect(
			screen.queryByLabelText("Length in seconds"),
		).not.toBeInTheDocument();

		// The kind is a listbox: its trigger shows what is chosen now.
		await userEvent.click(screen.getByRole("button", { name: "Time of day" }));
		await userEvent.click(screen.getByRole("option", { name: "Fixed words" }));
		expect(screen.getByLabelText("Words")).toBeInTheDocument();
	});

	it("writes a new source at a free address in the text range", async () => {
		const server = stubServer();
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "New text source" }),
		);
		await userEvent.type(screen.getByLabelText("Name"), "House open");
		await userEvent.type(screen.getByLabelText("Words"), "Doors in five");
		await userEvent.click(screen.getByRole("button", { name: "Create" }));

		await waitFor(() => expect(server.text).toHaveLength(3));
		const created = server.text[2];
		expect(created.name).toBe("House open");
		expect(created.text).toBe("Doors in five");
		expect(created.address).toMatchObject({ folder: 200, file: 3 });
		expect(server.writes).toContain("/text/create");
	});

	it("changes a countdown's length through the edit path", async () => {
		const server = stubServer({ text: [aCountdown()] });
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change" }),
		);
		const length = screen.getByLabelText("Length in seconds");
		await userEvent.clear(length);
		await userEvent.type(length, "90");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() => expect(server.text[0].durationSeconds).toBe(90));
		expect(server.writes).toContain("/text/200/2/update");
	});

	it("parks a source without losing what it says", async () => {
		const server = stubServer({
			text: [aClock({ kind: "static", text: "Interval" })],
		});
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change" }),
		);
		await userEvent.click(screen.getByLabelText("Available to a desk"));
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() => expect(server.text[0].enabled).toBe(false));
		expect(server.text[0].text).toBe("Interval");
	});

	it("preserves line breaks in fixed words", async () => {
		const server = stubServer({
			text: [aClock({ kind: "static", text: "House open" })],
		});
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change" }),
		);
		const words = screen.getByLabelText("Words");
		expect(words.tagName).toBe("TEXTAREA");
		await userEvent.clear(words);
		await userEvent.type(words, "Doors open{enter}Five minutes");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() =>
			expect(server.text[0].text).toBe("Doors open\nFive minutes"),
		);
	});

	it("removes a source", async () => {
		const server = stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Remove" }),
		);

		await waitFor(() => expect(server.text).toHaveLength(0));
		expect(server.writes).toContain("/text/200/1/delete");
	});

	it("says why a refused edit was not applied", async () => {
		stubServer({
			text: [aClock()],
			refuseWrites: {
				code: "address-taken",
				message: "another text source already answers at that address",
				status: 400,
			},
		});
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Change" }),
		);
		await userEvent.click(screen.getByRole("button", { name: "Save" }));

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"already answers",
		);
	});
});

describe("choosing where a new source goes", () => {
	it("skips the addresses that are already answered", () => {
		expect(nextFreeAddress([aClock(), aCountdown()])).toEqual({
			folder: 200,
			file: 3,
		});
		expect(nextFreeAddress([])).toEqual({ folder: 200, file: 1 });
	});
});
