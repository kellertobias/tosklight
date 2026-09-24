import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	aClock,
	aCountdown,
	anOutput,
	anOutputConfiguration,
	aTextFormat,
	stubServer,
} from "../../testing/server";
import { emptyDraft } from "./TextSourceEditor";
import {
	formatDraftPreview,
	nextFreeAddress,
	TextSourcesPage,
} from "./TextSourcesPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the text sources page", () => {
	it("shows each source with the address a desk reaches it by", async () => {
		stubServer();
		const { container } = render(<TextSourcesPage />);

		expect(
			await screen.findByRole("button", { name: /Clock/ }),
		).toBeInTheDocument();
		expect(container.querySelector(".media-library-folder-pool")).toHaveClass(
			"ui-button-grid",
			"square-grid",
		);
		expect(screen.getAllByText("200/001")).not.toHaveLength(0);
		expect(
			screen.getByRole("button", { name: /Ten minutes/ }),
		).toHaveTextContent("600 s");
		expect(
			document.querySelector(".media-library-pool .ui-button-grid"),
		).toHaveClass("media-file-pool-grid", "media-library-file-pool-grid");
	});

	it("frames text previews at the configured main-output ratio", async () => {
		const output = anOutput();
		stubServer({
			outputs: [output],
			outputConfigurations: {
				[output.id]: anOutputConfiguration(output.id, output.name, {
					width: 1024,
					height: 768,
				}),
			},
		});
		render(<TextSourcesPage />);

		const preview = await screen.findByRole("figure", {
			name: "Clock preview",
		});
		await waitFor(() =>
			expect(preview.querySelector(".media-preview-picture")).toHaveStyle({
				aspectRatio: String(4 / 3),
			}),
		);
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

		await screen.findByLabelText("Name");
		// A clock has no words and no length.
		expect(
			screen.queryByRole("textbox", { name: "Content" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByLabelText("Length in seconds"),
		).not.toBeInTheDocument();

		// The kind is a listbox: its trigger shows what is chosen now.
		await userEvent.click(screen.getByRole("button", { name: "Text type" }));
		await userEvent.click(screen.getByRole("option", { name: "Fixed words" }));
		expect(
			screen.getByRole("textbox", { name: "Content" }),
		).toBeInTheDocument();
	});

	it("keeps the addressed preview and identity at the top without a duplicate address", async () => {
		stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		const preview = await screen.findByRole("figure", {
			name: "Clock preview",
		});
		expect(preview.parentElement).toHaveClass("media-generated-sticky-region");
		expect(preview).toHaveTextContent("200/001");
		const identity = screen
			.getByLabelText("Name")
			.closest(".media-source-identity-grid");
		expect(identity).toContainElement(
			screen.getByRole("button", { name: "Text type" }),
		);
		expect(screen.queryByText("Text 200/001")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("heading", { name: "Clock" }),
		).not.toBeInTheDocument();
	});

	it("defaults to type-specific Content and separates Appearance", async () => {
		stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		const tabs = await screen.findByRole("tablist", { name: "Text settings" });
		const heading = tabs.closest(".media-text-section-heading");
		const content = screen.getByRole("tabpanel", { name: "Content" });
		expect(heading).toBeInTheDocument();
		expect(content).toHaveClass("media-text-section-content");
		expect(heading?.parentElement).toBe(content.parentElement);
		expect(heading?.compareDocumentPosition(content as Node)).toBe(
			Node.DOCUMENT_POSITION_FOLLOWING,
		);
		expect(tabs).toContainElement(
			screen.getByRole("tab", { name: "Content", selected: true }),
		);
		expect(screen.getByLabelText("Display format")).toBeInTheDocument();
		expect(screen.queryByLabelText("Font")).not.toBeInTheDocument();

		await userEvent.click(screen.getByRole("tab", { name: "Appearance" }));
		expect(screen.getByRole("tabpanel", { name: "Appearance" })).toHaveClass(
			"media-text-section-content",
		);
		expect(screen.getByLabelText("Font")).toBeInTheDocument();
		expect(screen.getByLabelText("Height")).toBeInTheDocument();
		expect(screen.getByText("Alignment")).toBeInTheDocument();
		expect(screen.getByText("Centre")).toBeInTheDocument();
		expect(screen.getByText("Colour")).toBeInTheDocument();
		expect(screen.queryByLabelText("Display format")).not.toBeInTheDocument();
	});

	it("writes a new source at a free address in the text range", async () => {
		const server = stubServer();
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "New text source" }),
		);
		await userEvent.type(screen.getByLabelText("Name"), "House open");
		await userEvent.type(
			screen.getByRole("textbox", { name: "Content" }),
			"Doors in five",
		);
		await userEvent.click(screen.getByRole("button", { name: "Create" }));

		await waitFor(() => expect(server.text).toHaveLength(3));
		const created = server.text[2];
		expect(created.name).toBe("House open");
		expect(created.text).toBe("Doors in five");
		expect(created.address).toMatchObject({ folder: 200, file: 3 });
		expect(server.writes).toContain("/text/create");
	});

	it("puts New text source before the library mode tabs", async () => {
		stubServer();
		render(<TextSourcesPage />);

		const action = await screen.findByRole("button", {
			name: "New text source",
		});
		const tabs = document.querySelector(
			".ui-window-header [role='tablist']",
		) as HTMLElement;
		expect(
			action.compareDocumentPosition(tabs) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("opens a text folder inspector instead of implicitly choosing a source", async () => {
		const server = stubServer();
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: /201Text0\/254/u }),
		);

		expect(
			screen.getByRole("heading", { name: "Configure folder" }),
		).toBeInTheDocument();
		expect(screen.getAllByText("Folder 201")).not.toHaveLength(0);
		await userEvent.type(screen.getByLabelText("Folder name"), "Timers");
		await waitFor(() =>
			expect(
				server.folderPresentations.folders.find(
					(candidate) => candidate.folder === 201,
				)?.name,
			).toBe("Timers"),
		);
	});

	it("changes a countdown's length through the edit path", async () => {
		const server = stubServer({ text: [aCountdown()] });
		render(<TextSourcesPage />);

		const length = await screen.findByLabelText("Length in seconds");
		expect(screen.getByRole("tabpanel", { name: "Content" })).toContainElement(
			length,
		);
		expect(
			screen.getByRole("tablist", { name: "Text settings" }),
		).not.toContainElement(length);
		await userEvent.clear(length);
		await userEvent.type(length, "90");

		await waitFor(() => expect(server.text[0].durationSeconds).toBe(90));
		expect(
			screen.queryByRole("button", { name: "Save" }),
		).not.toBeInTheDocument();
		expect(server.writes).toContain("/text/200/2/update");
	});

	it("switches a countdown to a time every day and edits that time", async () => {
		const server = stubServer({ text: [aCountdown()] });
		render(<TextSourcesPage />);

		await screen.findByLabelText("Length in seconds");
		await userEvent.click(screen.getByRole("button", { name: "Text type" }));
		await userEvent.click(
			screen.getByRole("option", { name: "Countdown to a time every day" }),
		);
		const time = await screen.findByLabelText(/Counts down every day to/u);
		expect(time).toHaveAttribute("type", "time");
		expect(screen.queryByLabelText("Length in seconds")).not.toBeInTheDocument();
		await waitFor(() =>
			expect(server.text[0]).toMatchObject({
				kind: "countdown-time-of-day",
				timeOfDaySeconds: 21 * 3_600,
			}),
		);

		fireEvent.change(time, { target: { value: "19:30:15" } });
		await waitFor(() =>
			expect(server.text[0].timeOfDaySeconds).toBe(19 * 3_600 + 30 * 60 + 15),
		);
		expect(time).toHaveValue("19:30:15");
	});

	it("updates countdown formatting and its preview live", async () => {
		const server = stubServer({ text: [aCountdown()] });
		render(<TextSourcesPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "Display format" }),
		);
		await userEvent.click(screen.getByRole("option", { name: "mm:ss" }));
		const separator = screen.getByLabelText("Separator");
		await userEvent.clear(separator);
		await userEvent.type(separator, ".");

		await waitFor(() =>
			expect(server.text[0].format).toMatchObject({
				countdownPattern: "mm:ss",
				separator: ".",
			}),
		);
		expect(
			screen.getByRole("figure", { name: "Ten minutes preview" }),
		).toHaveTextContent("10.00");
		expect(
			screen.queryByRole("button", { name: "Save" }),
		).not.toBeInTheDocument();
	});

	it("does not expose parking or live-update explanatory copy", async () => {
		stubServer({ text: [aClock({ kind: "static", text: "Interval" })] });
		render(<TextSourcesPage />);

		await screen.findByRole("textbox", { name: "Content" });
		expect(
			screen.queryByLabelText("Available to a desk"),
		).not.toBeInTheDocument();
		expect(screen.queryByText(/changes update live/iu)).not.toBeInTheDocument();
		expect(screen.queryByText(/parked/iu)).not.toBeInTheDocument();
	});

	it("preserves line breaks in fixed words", async () => {
		const server = stubServer({
			text: [aClock({ kind: "static", text: "House open" })],
		});
		render(<TextSourcesPage />);

		const words = await screen.findByRole("textbox", { name: "Content" });
		expect(words.tagName).toBe("TEXTAREA");
		await userEvent.clear(words);
		await userEvent.type(words, "Doors open{enter}Five minutes");

		await waitFor(() =>
			expect(server.text[0].text).toBe("Doors open\nFive minutes"),
		);
	});

	it("removes a source", async () => {
		const server = stubServer({ text: [aClock()] });
		render(<TextSourcesPage />);

		await screen.findByLabelText("Name");
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

		const name = await screen.findByLabelText("Name");
		await userEvent.type(name, " refused");

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

describe("text preview formatting", () => {
	it("renders clock format, separator, and fixed offset deterministically", () => {
		const draft = emptyDraft();
		draft.kind = "clock";
		draft.format = {
			...aTextFormat(),
			clockPattern: "hh:mm",
			separator: ".",
			utcOffsetMinutes: 60,
		};
		expect(formatDraftPreview(draft, Date.UTC(2026, 0, 1, 13, 45, 30))).toBe(
			"02.45",
		);
	});

	it("supports continuous and clock-style countdown minutes", () => {
		const draft = emptyDraft();
		draft.kind = "countdown-duration";
		draft.durationSeconds = 3_661;
		draft.format = {
			...aTextFormat(),
			countdownPattern: "mm:ss",
			separator: ".",
		};
		expect(formatDraftPreview(draft, 0)).toBe("61.01");
		draft.format.rollover = true;
		expect(formatDraftPreview(draft, 0)).toBe("01.01");
	});

	it("counts to a time every day as the server does, in the server's offset", () => {
		const draft = emptyDraft();
		draft.kind = "countdown-time-of-day";
		draft.timeOfDay = "21:00";
		draft.format = { ...aTextFormat(), afterZero: "count-up", utcOffsetMinutes: null };
		const at = (hour: number) => Date.UTC(2026, 8, 24, hour);
		expect(formatDraftPreview(draft, at(8))).toBe("13:00:00");
		expect(formatDraftPreview(draft, at(22))).toBe("01:00:00");
		expect(formatDraftPreview(draft, at(24))).toBe("21:00:00");
		// Two hours ahead of UTC, 19:00 UTC is 21:00 there.
		expect(formatDraftPreview(draft, at(18), 120)).toBe("01:00:00");
	});
});
