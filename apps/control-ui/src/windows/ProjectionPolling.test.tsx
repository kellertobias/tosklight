import { act, cleanup, render } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VisualizationRuntimeTransport } from "../features/visualizationRuntime/transport";
import { VisualizationRuntimeProvider } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { ChannelsWindow } from "./ChannelsWindow";
import { DmxWindow } from "./DmxWindow";

vi.mock("../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => server.status,
	useServerError: () => null,
}));

vi.mock("../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	usePatchedFixturesView: () => [],
}));

const server = {
	status: "connected",
	readDmx: vi.fn().mockResolvedValue({ universes: [], overrides: [] }),
	readVisualization: vi.fn().mockResolvedValue({ values: [] }),
	patch: { fixtures: [], routes: [] },
	outputRoutes: [],
	selectedFixtures: [],
	bootstrap: null,
	setDmxOverride: vi.fn(),
	setProgrammer: vi.fn(),
	setSelection: vi.fn(),
};
const visualizationTransport = {
	loadSnapshot: vi.fn(async (_scope, lane: "normal" | "preload") => ({
		revision: 1,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 1,
		blackout: false,
		preload: lane === "preload",
		values: [],
		profile_output_values: [],
	})),
} satisfies VisualizationRuntimeTransport;

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
		const rendered = render(
			visualization(<ChannelsWindow active={false} compact />),
		);
		await act(() => vi.advanceTimersByTimeAsync(1_000));
		expect(visualizationTransport.loadSnapshot).not.toHaveBeenCalled();

		rendered.rerender(visualization(<ChannelsWindow active compact />));
		await act(async () => undefined);
		expect(visualizationTransport.loadSnapshot).toHaveBeenCalledOnce();
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

function visualization(child: ReactNode) {
	return (
		<VisualizationRuntimeProvider
			showId="11111111-1111-4111-8111-111111111111"
			sessionId="22222222-2222-4222-8222-222222222222"
			authorityKey="server-a"
			transport={visualizationTransport}
		>
			{child}
		</VisualizationRuntimeProvider>
	);
}
