import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChannelsWindow } from "./ChannelsWindow";
import { DmxWindow } from "./DmxWindow";

const server = {
	status: "connected",
	readDmx: vi.fn().mockResolvedValue({ universes: [], overrides: [] }),
	readVisualization: vi.fn().mockResolvedValue({ values: [] }),
	patch: { fixtures: [], routes: [] },
	selectedFixtures: [],
	bootstrap: null,
	setDmxOverride: vi.fn(),
	setProgrammer: vi.fn(),
	setSelection: vi.fn(),
};

vi.mock("../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: { dmxDotSize: "small" },
		dispatch: vi.fn(),
	}),
}));

beforeEach(() => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe("high-rate diagnostic projection lifecycle", () => {
	it("keeps a covered Channels pane completely idle", async () => {
		const rendered = render(<ChannelsWindow active={false} compact />);
		await act(() => vi.advanceTimersByTimeAsync(1_000));
		expect(server.readVisualization).not.toHaveBeenCalled();

		rendered.rerender(<ChannelsWindow active compact />);
		await act(async () => undefined);
		expect(server.readVisualization).toHaveBeenCalledOnce();
	});

	it("keeps a covered DMX pane completely idle", async () => {
		const rendered = render(<DmxWindow active={false} compact />);
		await act(() => vi.advanceTimersByTimeAsync(1_000));
		expect(server.readDmx).not.toHaveBeenCalled();

		rendered.rerender(<DmxWindow active compact />);
		await act(async () => undefined);
		expect(server.readDmx).toHaveBeenCalledOnce();
	});
});
