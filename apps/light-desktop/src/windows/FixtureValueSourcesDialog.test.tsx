import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FixtureValueSourcesDialog } from "./FixtureValueSourcesDialog";

const read = vi.hoisted(() => vi.fn());
vi.mock("../features/dmxDiagnostics/DmxDiagnosticsContext", () => ({
	useDmxDiagnostics: () => ({ readFixtureValueSources: read }),
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show",
}));
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});
const targets = [
	{ fixtureId: "first", id: 1, name: "Front" },
	{ fixtureId: "head", id: "2.1", name: "Head" },
];
const snapshot = (fixture_id: string, source: string) => ({
	fixture_id,
	generated_at: "2026-10-30T00:00:00Z",
	grand_master: 1,
	blackout: false,
	highlighted: false,
	values: [
		{
			attribute: "intensity",
			value: { kind: "normalized", value: 0.5 },
			sources: [source],
		},
	],
	dynamics: [],
});

describe("Fixture source inspection", () => {
	it("shows authoritative ownership and refreshes only on an explicit request", async () => {
		read
			.mockResolvedValueOnce(snapshot("first", "Playback 2.7 · Front"))
			.mockResolvedValueOnce(snapshot("first", "Programmer"));
		render(<FixtureValueSourcesDialog targets={targets} onClose={() => {}} />);
		expect(await screen.findByText("Playback 2.7 · Front")).toBeTruthy();
		expect(screen.getByText("50%")).toBeTruthy();
		expect(read).toHaveBeenCalledTimes(1);
		fireEvent.click(screen.getByRole("button", { name: "Refresh sources" }));
		expect(await screen.findByText("Programmer")).toBeTruthy();
		expect(screen.queryByText("Playback 2.7 · Front")).toBeNull();
		expect(read).toHaveBeenCalledTimes(2);
	});
	it("allows a logical head and recovers when a show replaces the available targets", async () => {
		read.mockImplementation(async (id) => snapshot(id, `Owner ${id}`));
		const view = render(
			<FixtureValueSourcesDialog
				targets={targets}
				initialFixtureId="head"
				onClose={() => {}}
			/>,
		);
		expect(await screen.findByText("Owner head")).toBeTruthy();
		view.rerender(
			<FixtureValueSourcesDialog targets={[targets[0]]} onClose={() => {}} />,
		);
		expect(await screen.findByText("Owner first")).toBeTruthy();
		expect(read.mock.calls.map(([id]) => id)).toEqual(["head", "first"]);
	});
	it("shows an actionable error and retries", async () => {
		read
			.mockRejectedValueOnce(new Error("Disconnected"))
			.mockResolvedValueOnce(snapshot("first", "Freeze (held output)"));
		render(<FixtureValueSourcesDialog targets={targets} onClose={() => {}} />);
		expect(await screen.findByRole("alert")).toHaveProperty(
			"textContent",
			expect.stringContaining("Disconnected"),
		);
		await waitFor(() =>
			expect(
				(
					screen.getByRole("button", {
						name: "Refresh sources",
					}) as HTMLButtonElement
				).disabled,
			).toBe(false),
		);
		fireEvent.click(screen.getByRole("button", { name: "Refresh sources" }));
		expect(await screen.findByText("Freeze (held output)")).toBeTruthy();
	});
});
