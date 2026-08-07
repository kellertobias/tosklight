// @vitest-environment jsdom

import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DesktopBridge } from "../../platform/desktop/types";
import type { NativeStagePane } from "./useNativeStagePane";
import { useStagePanePicks } from "./useStagePanePicks";
import type { StageSelectionModel } from "./useStageSelection";

const bridge = vi.hoisted(() => ({ current: null as DesktopBridge | null }));

vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => bridge.current,
}));

function selectionModel(selected: string[] = []) {
	return {
		fixtureIds: selected,
		fixtureIdSet: new Set(selected),
		firstFixtureId: selected[0] ?? null,
		applyFixtureGesture: vi.fn(async () => null),
		replaceFixtureIds: vi.fn(async () => null),
	} as unknown as StageSelectionModel & {
		applyFixtureGesture: ReturnType<typeof vi.fn>;
		replaceFixtureIds: ReturnType<typeof vi.fn>;
	};
}

function drive(selection: StageSelectionModel, active = true) {
	function Probe() {
		useStagePanePicks({ active } as NativeStagePane, selection, true);
		return null;
	}
	return render(<Probe />);
}

describe("useStagePanePicks", () => {
	afterEach(() => {
		cleanup();
		bridge.current = null;
	});

	/*
	 * The renderer resolves what is under the pointer and answers with a fixture; this is the half
	 * that decides what that means. It has to mean exactly what the desk's own Stage means, or the
	 * two renderers disagree about the one thing an operator must be able to trust.
	 */
	it("selects the fixture the renderer answered with", async () => {
		const selection = selectionModel();
		bridge.current = {
			takeStagePanePicks: vi.fn(async () => [["fixture-1", false]]),
		} as unknown as DesktopBridge;
		drive(selection);
		await waitFor(() =>
			expect(selection.applyFixtureGesture).toHaveBeenCalledWith(
				"fixture-1",
				"add",
			),
		);
	});

	/** Shift over a fixture already selected takes it out, as it does on the desk's own Stage. */
	it("removes a fixture the operator extended onto twice", async () => {
		const selection = selectionModel(["fixture-1"]);
		bridge.current = {
			takeStagePanePicks: vi.fn(async () => [["fixture-1", true]]),
		} as unknown as DesktopBridge;
		drive(selection);
		await waitFor(() =>
			expect(selection.applyFixtureGesture).toHaveBeenCalledWith(
				"fixture-1",
				"remove",
			),
		);
	});

	/** A click on nothing clears, which is what empty floor does on the desk's Stage. */
	it("clears the selection when the operator clicked nothing", async () => {
		const selection = selectionModel(["fixture-1"]);
		bridge.current = {
			takeStagePanePicks: vi.fn(async () => [[null, false]]),
		} as unknown as DesktopBridge;
		drive(selection);
		await waitFor(() =>
			expect(selection.replaceFixtureIds).toHaveBeenCalledWith([]),
		);
	});

	/** Nothing is drained while no renderer is drawing, so a stale pick cannot arrive later. */
	it("asks for nothing while the pane is not drawing", async () => {
		const selection = selectionModel();
		const takeStagePanePicks = vi.fn(async () => []);
		bridge.current = { takeStagePanePicks } as unknown as DesktopBridge;
		drive(selection, false);
		await new Promise((resolve) => setTimeout(resolve, 120));
		expect(takeStagePanePicks).not.toHaveBeenCalled();
	});
});
