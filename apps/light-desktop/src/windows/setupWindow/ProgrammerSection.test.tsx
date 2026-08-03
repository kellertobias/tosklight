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
	HighlightLookSettings,
	PatchHighlightSettings,
} from "./ProgrammerSection";

afterEach(cleanup);

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
	render(<HighlightLookSettings controller={controller} />);
	return { draft, editDraft };
}

describe("Desk Setup Highlight Look", () => {
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

	it("shows fixture-specific feedback for unsupported semantic parts", () => {
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

		expect(screen.getByRole("status")).toHaveTextContent(
			"Fixture 7 Dimmer: Color is unavailable",
		);
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
