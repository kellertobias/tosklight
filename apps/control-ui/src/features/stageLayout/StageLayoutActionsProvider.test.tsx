import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	StageLayoutActionOutcome,
	StageLayoutActionRequest,
	StagePositionAxis,
} from "../../api/stageLayoutTypes";
import {
	type StageLayoutActions,
	StageLayoutActionsProvider,
	useStageLayoutActions,
} from "./StageLayoutActionsProvider";
import { type StageLayoutObject, StageLayoutStore } from "./store";

const storedLayout = {
	kind: "stage_layout",
	id: "main",
	revision: 5,
	updated_at: "",
	body: { version: 2, positions: {}, positions3d: {} },
} as StageLayoutObject;

function renderActions(
	showId: string | null,
	moveOutcome: () => Promise<StageLayoutActionOutcome>,
) {
	const moveStageSelection = vi.fn((_request: StageLayoutActionRequest) =>
		moveOutcome(),
	);
	const putStageLayout = vi.fn(async () => undefined);
	const readStageLayout = vi.fn(async () => storedLayout);
	const onApplied = vi.fn();
	const onError = vi.fn();
	const observed: { current: StageLayoutActions | null } = { current: null };
	function Reader() {
		observed.current = useStageLayoutActions();
		return null;
	}
	render(
		<StageLayoutActionsProvider
			store={new StageLayoutStore()}
			showId={showId}
			putStageLayout={putStageLayout}
			moveStageSelection={moveStageSelection}
			readStageLayout={readStageLayout}
			onApplied={onApplied}
			onError={onError}
		>
			<Reader />
		</StageLayoutActionsProvider>,
	);
	return {
		observed,
		moveStageSelection,
		putStageLayout,
		readStageLayout,
		onApplied,
		onError,
	};
}

describe("stage layout move-selection action", () => {
	afterEach(cleanup);

	it("submits one intent request and reconciles the stored layout", async () => {
		const harness = renderActions("show-1", async () => ({
			request_id: "generated",
			revision: 6,
			moved_fixture_ids: ["fixture-b", "fixture-a"],
			replayed: false,
			changed: true,
		}));

		await harness.observed.current?.moveStageSelection(
			["fixture-b", "fixture-a"],
			"rotation_x" satisfies StagePositionAxis,
			-4.5,
		);

		expect(harness.moveStageSelection).toHaveBeenCalledTimes(1);
		expect(harness.moveStageSelection).toHaveBeenCalledWith({
			request_id: expect.stringMatching(/[0-9a-f-]{36}/),
			action: {
				type: "move_selection",
				fixture_ids: ["fixture-b", "fixture-a"],
				axis: "rotation_x",
				delta: -4.5,
			},
		});
		expect(harness.putStageLayout).not.toHaveBeenCalled();
		expect(harness.readStageLayout).toHaveBeenCalledWith("show-1");
		expect(harness.onApplied).toHaveBeenCalledWith(storedLayout);
		expect(harness.onError).toHaveBeenCalledWith(null);
	});

	it("requires an open show before moving stage positions", async () => {
		const harness = renderActions(null, async () => {
			throw new Error("unreachable");
		});

		await harness.observed.current?.moveStageSelection(["fixture-a"], "x", 1);

		expect(harness.moveStageSelection).not.toHaveBeenCalled();
		expect(harness.onError).toHaveBeenCalledWith(
			"Open a show before moving stage positions",
		);
	});

	it("reports a failed intent without applying a layout", async () => {
		const harness = renderActions("show-1", async () => {
			throw new Error("selection contains an unknown fixture");
		});

		await harness.observed.current?.moveStageSelection(["fixture-a"], "y", 2);

		await waitFor(() =>
			expect(harness.onError).toHaveBeenCalledWith(
				"selection contains an unknown fixture",
			),
		);
		expect(harness.onApplied).not.toHaveBeenCalled();
	});
});
