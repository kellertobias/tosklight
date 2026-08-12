// @vitest-environment jsdom

import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DesktopBridge } from "../../platform/desktop/types";
import type { NativeStagePane } from "./useNativeStagePane";
import { useStagePaneSelection } from "./useStagePaneSelection";
import type { StageSelectionModel } from "./useStageSelection";

const bridge = vi.hoisted(() => ({ current: null as DesktopBridge | null }));

vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => bridge.current,
}));

function selectionModel(fixtureIds: string[]) {
	return { fixtureIds } as unknown as StageSelectionModel;
}

function Probe({
	selected,
	visible,
	active = true,
}: {
	selected: string[];
	visible: boolean;
	active?: boolean;
}) {
	useStagePaneSelection(
		{ active } as NativeStagePane,
		selectionModel(selected),
		visible,
	);
	return null;
}

describe("useStagePaneSelection", () => {
	afterEach(() => {
		cleanup();
		bridge.current = null;
	});

	it("removes the displayed selection immediately when visibility is turned off", async () => {
		const setStagePaneSelection = vi.fn(async () => undefined);
		bridge.current = { setStagePaneSelection } as unknown as DesktopBridge;
		const view = render(<Probe selected={["fixture-1"]} visible />);
		await waitFor(() =>
			expect(setStagePaneSelection).toHaveBeenLastCalledWith(["fixture-1"]),
		);

		view.rerender(<Probe selected={["fixture-1"]} visible={false} />);
		await waitFor(() =>
			expect(setStagePaneSelection).toHaveBeenLastCalledWith([]),
		);
	});

	it("keeps new authoritative selections hidden and restores the current one", async () => {
		const setStagePaneSelection = vi.fn(async () => undefined);
		bridge.current = { setStagePaneSelection } as unknown as DesktopBridge;
		const view = render(<Probe selected={["fixture-1"]} visible={false} />);
		await waitFor(() =>
			expect(setStagePaneSelection).toHaveBeenLastCalledWith([]),
		);

		view.rerender(<Probe selected={["fixture-2"]} visible={false} />);
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(setStagePaneSelection).toHaveBeenCalledTimes(1);
		expect(setStagePaneSelection).toHaveBeenLastCalledWith([]);

		view.rerender(<Probe selected={["fixture-2"]} visible />);
		await waitFor(() =>
			expect(setStagePaneSelection).toHaveBeenLastCalledWith(["fixture-2"]),
		);
	});

	it("does not send selection to an inactive pane", async () => {
		const setStagePaneSelection = vi.fn(async () => undefined);
		bridge.current = { setStagePaneSelection } as unknown as DesktopBridge;
		render(<Probe selected={["fixture-1"]} visible active={false} />);
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(setStagePaneSelection).not.toHaveBeenCalled();
	});
});
