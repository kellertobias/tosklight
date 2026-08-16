import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import type { SetupWindowController } from "./controller";
import {
	DefaultsSection,
	HighlightLookSettings,
	HighlightSection,
	OthersSection,
	PatchHighlightSettings,
	PlaybackDefaultsSettings,
} from "./ProgrammerSection";

afterEach(cleanup);

describe("Desk Setup Defaults layout", () => {
	const controller = {
		defaultsTab: "record-update",
		programmerSettingsError: null,
		recordSettings: {
			mode: "merge",
			cueOnly: false,
			mergeActiveCue: false,
		},
		setRecordSettings: vi.fn(),
		updateSettings: {
			cue_mode: "add_to_current_cue",
			preset_mode: "update_existing",
			group_mode: "update_existing",
			show_update_modal_on_touch: false,
		},
		setUpdateSettings: vi.fn(),
		draft: {
			frame_rate_hz: 44,
			cuelist_auto_off_at_zero_default: false,
			cuelist_auto_off_flash_release_default: false,
			start_after_first_recording: false,
		} as DeskConfiguration,
		editDraft: vi.fn(),
	} as unknown as SetupWindowController;

	it("keeps Record and Update defaults in one compact two-card group", () => {
		const { container } = render(<DefaultsSection controller={controller} />);

		const group = container.querySelector(".defaults-record-update");
		expect(group).not.toBeNull();
		expect(group?.querySelectorAll(":scope > article")).toHaveLength(2);
		expect(screen.getByText("Record defaults")).toBeInTheDocument();
		expect(screen.getByText("Update defaults")).toBeInTheDocument();
	});

	it("mounts only the selected Defaults pane after every tab switch", () => {
		const view = render(<DefaultsSection controller={controller} />);
		expect(screen.getByText("Record defaults")).toBeInTheDocument();
		expect(screen.queryByText("Cuelist playback defaults")).toBeNull();
		expect(screen.queryByText("Pool color defaults")).toBeNull();

		view.rerender(
			<DefaultsSection
				controller={{ ...controller, defaultsTab: "playback" }}
			/>,
		);
		expect(screen.queryByText("Record defaults")).toBeNull();
		expect(screen.getByText("Cuelist playback defaults")).toBeInTheDocument();
		expect(screen.queryByText("Pool color defaults")).toBeNull();

		view.rerender(
			<DefaultsSection controller={{ ...controller, defaultsTab: "pools" }} />,
		);
		expect(screen.queryByText("Record defaults")).toBeNull();
		expect(screen.queryByText("Cuelist playback defaults")).toBeNull();
		expect(
			screen.getByRole("heading", { name: "Pool color defaults" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("heading", { name: "Defaults" })).toBeNull();
		expect(screen.queryByText(/server-wide presentation colors/)).toBeNull();
		const grid = view.container.querySelector(".pool-color-defaults-grid");
		expect(grid).toHaveStyle({ "--form-columns": "3" });
	});
});

function renderSettings(
	highlight_look: NonNullable<DeskConfiguration["highlight_look"]>,
	highlight_look_feedback?: string[],
) {
	const draft = {
		frame_rate_hz: 44,
		highlight_look,
		highlight_look_feedback,
	} as DeskConfiguration;
	const editDraft = vi.fn();
	const controller = {
		draft,
		editDraft,
	} as unknown as SetupWindowController;
	const { container } = render(
		<HighlightLookSettings controller={controller} />,
	);
	return { container, draft, editDraft };
}

describe("Desk Setup Highlight Look", () => {
	it("combines the patch switch into the compact top-label Highlight grid", () => {
		const draft = {
			frame_rate_hz: 44,
			highlight_look: {
				intensity: 1,
				color: "white",
				iris: null,
				zoom: null,
				focus: null,
				frost: null,
				compatibility: "semantic",
			},
			patch_preview_highlight_dmx: false,
		} as DeskConfiguration;
		const { container } = render(
			<HighlightSection
				controller={
					{
						draft,
						editDraft: vi.fn(),
						programmerSettingsError: null,
					} as unknown as SetupWindowController
				}
			/>,
		);
		expect(
			container.querySelectorAll(".programmer-setup-list > article"),
		).toHaveLength(1);
		expect(container.querySelector(".highlight-look-grid")).toHaveClass(
			"labels-top",
		);
		expect(
			screen.getByLabelText("Highlight patch selection via DMX"),
		).not.toBeChecked();
		expect(
			screen.queryByText(
				"One semantic identification look for every show on this desk.",
			),
		).toBeNull();
	});

	it("uses the compact multi-column Highlight layout", () => {
		const { container } = renderSettings({
			intensity: 1,
			color: "white",
			iris: null,
			zoom: null,
			focus: null,
			frost: null,
			compatibility: "semantic",
		});
		const grid = container.querySelector(".highlight-look-grid");
		expect(grid).toHaveStyle({ "--form-columns": "3" });
	});

	it("keeps intensity required, Shutter fixed to Open, and optional parts ignored independently", () => {
		const { draft, editDraft } = renderSettings({
			intensity: 0.8,
			color: null,
			iris: null,
			zoom: 0.4,
			focus: null,
			frost: null,
			compatibility: "semantic",
		});

		expect(screen.getByLabelText(/Intensity/)).toHaveValue("80");
		expect(screen.getByRole("button", { name: "Shutter" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Shutter" })).toHaveTextContent(
			"Open where available",
		);
		expect(screen.getByRole("button", { name: "Color" })).toHaveTextContent(
			"Ignore",
		);
		expect(
			screen.getByRole("button", { name: "Iris contribution" }),
		).toHaveTextContent("Ignore");
		expect(screen.queryByLabelText("Iris (%)")).not.toBeInTheDocument();
		expect(screen.getByLabelText("Zoom (%)")).toHaveValue("40");

		fireEvent.click(screen.getByRole("button", { name: "Iris contribution" }));
		fireEvent.click(
			within(
				screen.getByRole("listbox", { name: "Iris contribution" }),
			).getByRole("option", { name: "Configured" }),
		);
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			highlight_look: { ...draft.highlight_look, iris: 1 },
		});
	});

	it("writes one semantic color without changing the other look parts", () => {
		const { draft, editDraft } = renderSettings({
			intensity: 1,
			color: null,
			iris: null,
			zoom: null,
			focus: null,
			frost: null,
			compatibility: "semantic",
		});

		fireEvent.click(screen.getByRole("button", { name: "Color" }));
		fireEvent.click(
			within(screen.getByRole("listbox", { name: "Color" })).getByRole(
				"option",
				{ name: "Amber" },
			),
		);
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			highlight_look: { ...draft.highlight_look, color: "amber" },
		});
	});

	it.each([
		["legacy_raw", "LegacyRaw"],
		["needs_review", "NeedsReview"],
	] as const)("shows %s compatibility and requires an explicit semantic-look action", (compatibility, label) => {
		const { draft, editDraft } = renderSettings({
			intensity: 1,
			color: null,
			iris: null,
			zoom: null,
			focus: null,
			frost: null,
			compatibility,
		});

		expect(screen.getByRole("alert")).toHaveTextContent(label);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Use semantic Highlight Look",
			}),
		);
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			highlight_look: {
				...draft.highlight_look,
				compatibility: "semantic",
			},
		});
	});

	it("does not present unsupported optional semantic parts as an error", () => {
		renderSettings(
			{
				intensity: 1,
				color: "blue",
				iris: null,
				zoom: null,
				focus: null,
				frost: null,
				compatibility: "semantic",
			},
			["Fixture 7 Dimmer: Color is unavailable"],
		);

		expect(screen.queryByRole("status")).not.toBeInTheDocument();
		expect(
			screen.queryByText("Fixture 7 Dimmer: Color is unavailable"),
		).not.toBeInTheDocument();
	});
});

describe("Desk Setup Highlight patch", () => {
	it("uses the approved group and literal Stage choices", () => {
		const draft = {
			frame_rate_hz: 44,
			patch_preview_highlight_dmx: true,
		} as DeskConfiguration;
		const editDraft = vi.fn();
		render(
			<PatchHighlightSettings
				controller={{ draft, editDraft } as unknown as SetupWindowController}
			/>,
		);

		expect(screen.getByText("Highlight patch")).toBeInTheDocument();
		expect(
			screen.getByLabelText("Highlight patch selection via DMX"),
		).toBeChecked();
		expect(screen.getByText("Stage only")).toBeInTheDocument();
		expect(screen.getByText("Stage and DMX")).toBeInTheDocument();
	});
});

describe("Desk Setup Cuelist playback defaults", () => {
	it("shows the literal independent defaults and edits only the selected setting", () => {
		const draft = {
			frame_rate_hz: 44,
			cuelist_auto_off_at_zero_default: false,
			cuelist_auto_off_flash_release_default: true,
			start_after_first_recording: false,
		} as DeskConfiguration;
		const editDraft = vi.fn();
		render(
			<PlaybackDefaultsSettings
				controller={{ draft, editDraft } as unknown as SetupWindowController}
			/>,
		);

		expect(screen.getByText("Cuelist playback defaults")).toBeInTheDocument();
		expect(screen.getByLabelText("When fader reaches zero")).not.toBeChecked();
		expect(screen.getByLabelText("When Flash is released")).toBeChecked();
		expect(
			screen.getByLabelText("Start after first recording"),
		).not.toBeChecked();

		fireEvent.click(screen.getByLabelText("When fader reaches zero"));
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			cuelist_auto_off_at_zero_default: true,
		});
	});
});

describe("Desk Setup direct timing and Preload defaults", () => {
	it("shows the fresh Immediate and Programmer plus Virtual capture contract", () => {
		const draft = {
			frame_rate_hz: 44,
			command_line_at_uses_programmer_fade: false,
			preload_programmer_changes: true,
			preload_physical_playback_actions: false,
			preload_virtual_playback_actions: true,
		} as DeskConfiguration;
		render(
			<OthersSection
				controller={
					{
						draft,
						editDraft: vi.fn(),
						programmerSettingsError: null,
					} as unknown as SetupWindowController
				}
			/>,
		);

		expect(
			screen.getByLabelText("Direct entry uses Programmer Fade"),
		).not.toBeChecked();
		expect(screen.getByText("Immediate")).toBeInTheDocument();
		expect(screen.getByLabelText("Preload programmer changes")).toBeChecked();
		expect(
			screen.getByLabelText("Preload physical playback actions"),
		).not.toBeChecked();
		expect(
			screen.getByLabelText("Preload virtual playback actions"),
		).toBeChecked();
		expect(
			screen.getByText(/Physical Flash and hardware\/physical fader movements/),
		).toBeInTheDocument();
	});
});
