import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { IndexedPresetChoice } from "../../control/parameterControls/indexedPresetChoices";
import { MediaPlayModeDialog, playModeMutations } from "./media";

afterEach(cleanup);

function choice(
	id: string,
	label: string,
	semanticId: string,
): IndexedPresetChoice {
	return {
		id,
		label,
		description: "All selected fixtures",
		kind: "indexed",
		semanticId,
		controlKind: null,
		targets: [{ fixtureId: "fixture-1" }],
		disabled: false,
	};
}

describe("MediaPlayModeDialog", () => {
	it("shows only authored Play Mode choices and the live selection", () => {
		render(
			<MediaPlayModeDialog
				choices={[
					choice("loop", "Loop", "loop"),
					choice("once", "Play once", "once"),
				]}
				value="loop"
				mixed={false}
				disabled={false}
				apply={vi.fn(async () => undefined)}
			/>,
		);

		expect(screen.getByRole("button", { name: /Loop/ })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		expect(screen.getByRole("button", { name: /Play once/ })).toHaveAttribute(
			"aria-pressed",
			"false",
		);
		expect(screen.queryByText(/Folder|File|Transport|Speed/)).toBeNull();
	});

	it("dispatches the selected semantic Play Mode", () => {
		const apply = vi.fn(async (_choice: IndexedPresetChoice) => undefined);
		const loop = choice("loop", "Loop", "loop");
		render(
			<MediaPlayModeDialog
				choices={[loop]}
				value={null}
				mixed={true}
				disabled={false}
				apply={apply}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: /Loop/ }));
		expect(apply).toHaveBeenCalledWith(loop);
		expect(screen.getByText("Mixed selection")).toBeInTheDocument();
	});

	it("addresses only the capability targets with a discrete Programmer value", () => {
		const loop = choice("loop", "Loop", "loop");
		loop.targets = [{ fixtureId: "head-2" }, { fixtureId: "fixture-4" }];
		expect(playModeMutations(loop, 1_500)).toEqual([
			{
				action: "set_fixture",
				fixtureId: "head-2",
				attribute: "media.play_mode",
				value: { kind: "discrete", value: "loop" },
				timing: { fade: true, fadeMillis: 1_500, delayMillis: null },
			},
			{
				action: "set_fixture",
				fixtureId: "fixture-4",
				attribute: "media.play_mode",
				value: { kind: "discrete", value: "loop" },
				timing: { fade: true, fadeMillis: 1_500, delayMillis: null },
			},
		]);
	});
});
