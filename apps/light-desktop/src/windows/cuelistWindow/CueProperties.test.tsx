import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Cue } from "../../api/types";
import { CuePropertyModal } from "./CueProperties";

const cue: Cue = {
	id: "cue-1",
	number: 1,
	name: "Opening",
	fade_millis: 2_000,
	delay_millis: 500,
	out_fade_millis: 3_000,
	out_delay_millis: 750,
	trigger: { type: "manual" },
	changes: [],
};

afterEach(cleanup);

describe("CuePropertyModal", () => {
	it.each([
		["name", "Cue Name", "Reprise", { name: "Reprise" }],
		[
			"information",
			"Cue Information",
			"Stand by follow spot",
			{ information: "Stand by follow spot" },
		],
	] as const)(
		"saves persistent Cue %s text",
		async (property, label, value, expected) => {
			const onSave = vi.fn(async (_cue: Cue) => true);
			render(
				<CuePropertyModal
					cue={cue}
					cues={[cue]}
					property={property}
					editError=""
					onCancel={vi.fn()}
					onSave={onSave}
				/>,
			);
			fireEvent.change(screen.getByRole("textbox", { name: label }), {
				target: { value },
			});
			fireEvent.click(screen.getByRole("button", { name: "Save" }));
			await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
			expect(onSave.mock.calls[0]?.[0]).toMatchObject(expected);
		},
	);

	it("saves only the exact timing draft through the authoritative callback", async () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		const onCancel = vi.fn();
		render(
			<CuePropertyModal
				cue={cue}
				cues={[cue]}
				property="inFade"
				editError=""
				onCancel={onCancel}
				onSave={onSave}
			/>,
		);

		const dialog = screen.getByRole("dialog", { name: "In Fade" });
		expect(dialog).toBeInTheDocument();
		fireEvent.change(within(dialog).getByRole("textbox", { name: "In Fade" }), {
			target: { value: "4.5" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save" }));

		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0]).toMatchObject({
			fade_millis: 4_500,
			delay_millis: 500,
			trigger: { type: "manual" },
		});
		expect(onCancel).toHaveBeenCalledOnce();
	});

	it("cancels without mutating the Cue", () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		const onCancel = vi.fn();
		render(
			<CuePropertyModal
				cue={cue}
				cues={[cue]}
				property="outDelay"
				editError=""
				onCancel={onCancel}
				onSave={onSave}
			/>,
		);

		fireEvent.change(
			within(screen.getByRole("dialog", { name: "Out Delay" })).getByRole(
				"textbox",
				{ name: "Out Delay" },
			),
			{ target: { value: "9" } },
		);
		expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Cancel Out Delay" }));
		expect(onCancel).toHaveBeenCalledOnce();
		expect(onSave).not.toHaveBeenCalled();
	});

	it.each(["trigger", "triggerTime", "inDelay", "outDelay"] as const)(
		"places the %s Save action in the modal title",
		(property) => {
			render(
				<CuePropertyModal
					cue={cue}
					cues={[cue]}
					property={property}
					editError=""
					onCancel={vi.fn()}
					onSave={vi.fn(async (_cue: Cue) => true)}
				/>,
			);

			const dialog = screen.getByRole("dialog", {
				name:
					property === "trigger"
						? "Trigger"
						: property === "triggerTime"
							? "Trigger Time"
							: property === "inDelay"
								? "In Delay"
								: "Out Delay",
			});
			expect(
				within(dialog)
					.getByRole("button", { name: "Save" })
					.closest(".ui-title-chrome-terminals"),
			).toHaveClass("ui-title-chrome-terminals");
			expect(within(dialog).queryByRole("button", { name: "Cancel" })).toBeNull();
		},
	);

	it("keeps trigger-kind changes transactional until Save", async () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		render(
			<CuePropertyModal
				cue={cue}
				cues={[cue]}
				property="trigger"
				editError=""
				onCancel={vi.fn()}
				onSave={onSave}
			/>,
		);

		fireEvent.click(
			within(screen.getByRole("dialog", { name: "Trigger" })).getByRole(
				"button",
				{ name: "GO" },
			),
		);
		fireEvent.click(screen.getByRole("option", { name: "FOLLOW" }));
		expect(onSave).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0].trigger).toEqual({
			type: "follow",
			delay_millis: 0,
		});
	});

	it("stores a Jump by stable Cue identity without replacing other Cue actions", async () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		const source: Cue = {
			...cue,
			actions: [{ type: "timecode_stop", timecode_id: "timecode-1" }],
		};
		const destination: Cue = {
			...cue,
			id: "cue-2",
			number: 2,
			name: "Second",
		};
		render(
			<CuePropertyModal
				cue={source}
				cues={[source, destination]}
				property="jump"
				editError=""
				onCancel={vi.fn()}
				onSave={onSave}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "No Jump" }));
		fireEvent.click(screen.getByRole("option", { name: "Cue 2 · Second" }));
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(onSave.mock.calls[0]?.[0].actions).toEqual([
			{ type: "timecode_stop", timecode_id: "timecode-1" },
			{ type: "jump", cue_id: "cue-2", count: 1 },
		]);
	});

	it("requires Jump Count to be a positive whole number", () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		render(
			<CuePropertyModal
				cue={{
					...cue,
					actions: [{ type: "jump", cue_id: "cue-1", count: 2 }],
				}}
				cues={[cue]}
				property="jumpCount"
				editError=""
				onCancel={vi.fn()}
				onSave={onSave}
			/>,
		);
		fireEvent.change(screen.getByRole("textbox", { name: "Jump Count" }), {
			target: { value: "0" },
		});
		expect(screen.getByRole("alert")).toHaveTextContent(
			"whole Jump Count of one or greater",
		);
		expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
	});

	it("keeps the modal open with the actionable error when authority rejects Save", async () => {
		const onSave = vi.fn(async (_cue: Cue) => false);
		const onCancel = vi.fn();
		render(
			<CuePropertyModal
				cue={cue}
				cues={[cue]}
				property="inFade"
				editError="Cue edit was not saved. Review the draft and try again after the revision conflict."
				onCancel={onCancel}
				onSave={onSave}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
		expect(screen.getByRole("dialog", { name: "In Fade" })).toBeInTheDocument();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Cue edit was not saved",
		);
		expect(onCancel).not.toHaveBeenCalled();
	});

	it("keeps an invalid timing draft open and does not call authority", () => {
		const onSave = vi.fn(async (_cue: Cue) => true);
		render(
			<CuePropertyModal
				cue={cue}
				cues={[cue]}
				property="inFade"
				editError=""
				onCancel={vi.fn()}
				onSave={onSave}
			/>,
		);
		const dialog = screen.getByRole("dialog", { name: "In Fade" });
		fireEvent.change(within(dialog).getByRole("textbox", { name: "In Fade" }), {
			target: { value: "-1" },
		});

		expect(within(dialog).getByRole("alert")).toHaveTextContent(
			"Enter a time of zero or greater",
		);
		expect(within(dialog).getByRole("button", { name: "Save" })).toBeDisabled();
		expect(onSave).not.toHaveBeenCalled();
	});
});
