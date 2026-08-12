import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { appReducer, initialState } from "../../state/appReducer";
import {
	availableWindowCategoryChoices,
	WindowPicker,
	windowCategories,
	windowChoices,
} from "./WindowPicker";

const app = vi.hoisted(() => ({
	state: {
		windowPicker: { x: 3, y: 4, width: 8, height: 6 },
	},
	dispatch: vi.fn(),
}));
const media = vi.hoisted(() => ({ mediaServers: [] as Array<{ id: string }> }));

vi.mock("../../state/AppContext", () => ({ useApp: () => app }));
vi.mock("../../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => media,
}));

afterEach(() => {
	cleanup();
	app.dispatch.mockReset();
	media.mediaServers = [];
});

describe("Open Window categories", () => {
	it("represents every pane exactly once with a description", () => {
		const choices = windowCategories.flatMap(({ choices }) => choices);
		expect(choices).toHaveLength(18);
		expect(new Set(choices.map(({ kind }) => kind)).size).toBe(18);
		expect(choices.every(({ description }) => description.length > 0)).toBe(
			true,
		);
		expect(windowChoices).toEqual(
			choices.map(({ kind, title }) => [kind, title]),
		);
		expect(windowCategories.map(({ label }) => label)).toEqual([
			"Programming",
			"Playback & Automation",
			"Show & Visual",
			"Miscellaneous",
		]);
		expect(
			availableWindowCategoryChoices("programming").map(({ kind }) => kind),
		).toContain("macros");
		expect(
			availableWindowCategoryChoices("miscellaneous").map(
				({ kind }) => kind,
			),
		).toEqual([
			"running",
			"scheduler",
			"file_manager",
			"help",
			"text_editor",
		]);
		expect(windowChoices).toContainEqual(["cuelists", "Cuelists"]);
		expect(windowChoices.some(([kind]) => kind === "cuelist_pool")).toBe(false);
	});

	it("keeps Media discoverable without a media server", () => {
		expect(
			availableWindowCategoryChoices("show").map(({ kind }) => kind),
		).toContain("media");
	});
});

describe("Open Window modal", () => {
	it("uses title tabs and shared descriptive selection cards", async () => {
		render(<WindowPicker />);

		const dialog = screen.getByRole("dialog", { name: "Open Window" });
		expect(dialog).toBeVisible();
		expect(dialog.parentElement).toHaveClass("window-picker-layer");
		expect(dialog.parentElement).not.toHaveClass("ui-modal-wide");
		const programming = screen.getByRole("tab", { name: "Programming" });
		expect(programming).toHaveAttribute("aria-selected", "true");
		expect(screen.getByRole("tabpanel", { name: "Programming" })).toBeVisible();
		expect(screen.getByText("Preset pool")).toBeVisible();
		expect(
			screen.getByText("Store and recall reusable attribute values."),
		).toBeVisible();
		expect(screen.queryByText("Running")).not.toBeInTheDocument();

		await waitFor(() => expect(document.activeElement).toBe(programming));
		fireEvent.click(screen.getByRole("tab", { name: "Miscellaneous" }));
		expect(
			screen.getByRole("tabpanel", { name: "Miscellaneous" }),
		).toBeVisible();
		expect(screen.getByText("Running")).toBeVisible();
		expect(screen.getByText("Text Editor")).toBeVisible();

		fireEvent.click(screen.getByRole("tab", { name: "Programming" }));
		expect(screen.getByText("Macro Pool")).toBeVisible();
	});

	it("shows unavailable Media and adds a selected pane at the requested placement", () => {
		render(<WindowPicker />);
		fireEvent.click(screen.getByRole("tab", { name: "Show & Visual" }));
		expect(screen.getByText("Media")).toBeVisible();

		fireEvent.click(screen.getByRole("tab", { name: "Miscellaneous" }));
		fireEvent.click(screen.getByText("Running"));
		expect(app.dispatch).toHaveBeenCalledWith({
			type: "ADD_WINDOW",
			kind: "running",
		});

		const emptyDesk = {
			...initialState,
			activeDeskId: "picker-test",
			desks: [{ id: "picker-test", name: "Picker", panes: [] }],
			windowPicker: app.state.windowPicker,
		};
		const added = appReducer(emptyDesk, {
			type: "ADD_WINDOW",
			kind: "running",
		});
		expect(added.windowPicker).toBeNull();
		expect(added.desks[0].panes[0]).toMatchObject({
			kind: "running",
			x: 3,
			y: 4,
			width: 8,
			height: 6,
		});
	});

	it("uses the modal stack close contract for Escape and the title close action", () => {
		render(<WindowPicker />);
		fireEvent.keyDown(document, { key: "Escape" });
		expect(app.dispatch).toHaveBeenCalledWith({
			type: "OPEN_WINDOW_PICKER",
			rect: null,
		});

		app.dispatch.mockReset();
		fireEvent.click(screen.getByRole("button", { name: "Close Open Window" }));
		expect(app.dispatch).toHaveBeenCalledWith({
			type: "OPEN_WINDOW_PICKER",
			rect: null,
		});
	});

	it("shows Media when a media server is available", () => {
		media.mediaServers = [{ id: "media-server" }];
		render(<WindowPicker />);
		fireEvent.click(screen.getByRole("tab", { name: "Show & Visual" }));
		expect(screen.getByText("Media")).toBeVisible();
	});
});
