import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Cue } from "../../api/types";
import { CuePropertyModal } from "./CueProperties";
import type { CueEditableProperty } from "./CueTable";

const cue: Cue = {
	id: "cue-1",
	number: "1",
	name: "Opening",
	fade_millis: 2_000,
	delay_millis: 500,
	out_fade_millis: 3_000,
	out_delay_millis: 750,
	trigger: { type: "manual" },
	changes: [],
};

afterEach(cleanup);

function renderProperty(
	property: CueEditableProperty,
	overrides: Partial<Parameters<typeof CuePropertyModal>[0]> = {},
) {
	const onSave = vi.fn(async (_cue: Cue) => true);
	const onCancel = vi.fn();
	render(
		<CuePropertyModal
			cue={cue}
			cues={[cue]}
			property={property}
			editError=""
			onCancel={onCancel}
			onSave={onSave}
			{...overrides}
		/>,
	);
	return { onSave, onCancel };
}

function appendKeyboardValue(next: string) {
	for (const key of next) fireEvent.keyDown(window, { key });
}

describe("CuePropertyModal direct editors", () => {
	it.each([
		["name", "Cue Name", "keyboard-modal"],
		["information", "Cue Information", "keyboard-modal"],
		["jump", "Jump", "ui-grouped-selection-modal"],
		["trigger", "Trigger", "ui-grouped-selection-modal"],
		["triggerTime", "Trigger Time", "direct-value-modal"],
		["inDelay", "In Delay", "direct-value-modal"],
		["inFade", "In Fade", "direct-value-modal"],
		["outDelay", "Out Delay", "direct-value-modal"],
		["outFade", "Out Fade", "direct-value-modal"],
	] as const)("opens the %s direct surface without an intermediate form", (property, label, className) => {
		renderProperty(property);
		const dialog = screen.getByRole("dialog", { name: label });
		expect(dialog).toHaveClass(className);
		expect(dialog.querySelector(".cue-property-modal-body")).toBeNull();
	});

	it("commits Cue Name from the direct keyboard", async () => {
		const { onSave } = renderProperty("name");
		appendKeyboardValue(" Reprise");
		fireEvent.keyDown(window, { key: "Enter" });

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0]).toMatchObject({
			name: "Opening Reprise",
		});
	});

	it("commits Cue Information from the direct multiline keyboard", async () => {
		const { onSave } = renderProperty("information");
		appendKeyboardValue("Stand by follow spot");
		fireEvent.click(screen.getByRole("button", { name: "Done" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0]).toMatchObject({
			information: "Stand by follow spot",
		});
	});

	it("commits exact timing from the direct number model", async () => {
		const { onSave } = renderProperty("inFade");
		fireEvent.click(screen.getByRole("button", { name: "4" }));
		fireEvent.click(screen.getByRole("button", { name: "." }));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0]).toMatchObject({
			fade_millis: 4_500,
			delay_millis: 500,
		});
	});

	it("links Out Fade to Release without discarding its explicit value", async () => {
		const { onSave } = renderProperty("outFade");
		fireEvent.click(screen.getByRole("button", { name: "Use Release" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0]).toMatchObject({
			out_fade_millis: 3_000,
			out_fade_link: "release",
		});
	});

	it("shows the linked source and restores the retained explicit Out Fade", async () => {
		const { onSave } = renderProperty("outFade", {
			cue: { ...cue, out_fade_link: "release" },
		});
		expect(screen.getByText("Linked to Release · 3.0 s")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Use explicit value" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0].out_fade_millis).toBe(3_000);
		expect(onSave.mock.calls[0]?.[0].out_fade_link).toBeUndefined();
	});

	it("snapshots the effective source when unlinking without a retained explicit value", async () => {
		const fade = renderProperty("outFade", {
			cue: { ...cue, out_fade_millis: undefined, out_fade_link: "release" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Use explicit value" }));
		await waitFor(() => expect(fade.onSave).toHaveBeenCalledOnce());
		expect(fade.onSave.mock.calls[0]?.[0]).toMatchObject({
			out_fade_millis: 3_000,
		});
		cleanup();

		const delay = renderProperty("outDelay", {
			cue: {
				...cue,
				fade_millis: 0,
				out_delay_millis: undefined,
				out_delay_link: "in_fade",
			},
		});
		expect(screen.getByText("Linked to In Fade · 3.0 s")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Use explicit value" }));
		await waitFor(() => expect(delay.onSave).toHaveBeenCalledOnce());
		expect(delay.onSave.mock.calls[0]?.[0]).toMatchObject({
			out_delay_millis: 3_000,
		});
	});

	it("links Out Delay to In Fade and clears only the link for a numeric override", async () => {
		const linked = renderProperty("outDelay");
		fireEvent.click(screen.getByRole("button", { name: "Use In Fade" }));
		await waitFor(() => expect(linked.onSave).toHaveBeenCalledOnce());
		expect(linked.onSave.mock.calls[0]?.[0]).toMatchObject({
			out_delay_millis: 750,
			out_delay_link: "in_fade",
		});
		cleanup();

		const explicit = renderProperty("outDelay", {
			cue: { ...cue, out_delay_link: "in_fade" },
		});
		expect(screen.getByText("Linked to In Fade · 2.0 s")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "4" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		await waitFor(() => expect(explicit.onSave).toHaveBeenCalledOnce());
		expect(explicit.onSave.mock.calls[0]?.[0].out_delay_millis).toBe(4_000);
		expect(explicit.onSave.mock.calls[0]?.[0].out_delay_link).toBeUndefined();
	});

	it("commits Trigger directly from the grouped chooser", async () => {
		const { onSave } = renderProperty("trigger");
		fireEvent.click(screen.getByRole("button", { name: /FOLLOW/ }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0].trigger).toEqual({
			type: "follow",
			delay_millis: 0,
		});
	});

	it("stores a Jump by stable Cue identity through the grouped chooser", async () => {
		const destination: Cue = {
			...cue,
			id: "cue-2",
			number: "2",
			name: "Second",
		};
		const { onSave } = renderProperty("jump", {
			cue: {
				...cue,
				actions: [{ type: "timecode_stop", timecode_id: "timecode-1" }],
			},
			cues: [cue, destination],
		});
		fireEvent.click(screen.getByRole("button", { name: /Cue 2 · Second/ }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0].actions).toEqual([
			{ type: "timecode_stop", timecode_id: "timecode-1" },
			{ type: "jump", cue_id: "cue-2", count: 1 },
		]);
	});

	it("keeps invalid timing open with an actionable error", () => {
		const { onSave } = renderProperty("inFade");
		fireEvent.click(screen.getByRole("button", { name: "−" }));
		fireEvent.click(screen.getByRole("button", { name: "1" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));

		expect(screen.getByRole("dialog", { name: "In Fade" })).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Enter a time of zero or greater",
		);
		expect(onSave).not.toHaveBeenCalled();
	});

	it("keeps the direct editor open when authority rejects the commit", async () => {
		const onSave = vi.fn(async (_cue: Cue) => false);
		renderProperty("inFade", {
			onSave,
			editError: "Cue edit was not saved. Review the draft and try again.",
		});
		fireEvent.click(screen.getByRole("button", { name: "3" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(screen.getByRole("dialog", { name: "In Fade" })).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Cue edit was not saved",
		);
	});
});
