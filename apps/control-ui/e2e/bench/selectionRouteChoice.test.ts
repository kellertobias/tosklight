import { describe, expect, it, vi } from "vitest";
import { fixture, group } from "./selectionContract";
import {
	SeededSelectionRouteChoice,
	type SelectionRouteAdapter,
	UnsupportedSelectionRouteError,
} from "./selectionRouteChoice";

describe("seeded unqualified selection route choice", () => {
	it("chooses reproducibly from only eligible candidates and records its proof", async () => {
		const first = routes();
		const second = routes().reverse();
		const request = {
			action: "replace" as const,
			targets: [fixture(4)],
		};
		const firstChoice = new SeededSelectionRouteChoice("coverage-17", first);
		const reorderedChoice = new SeededSelectionRouteChoice(
			"coverage-17",
			second,
		);

		const actual = await firstChoice.execute(request);
		const reordered = await reorderedChoice.execute(request);

		expect(actual.report).toEqual(reordered.report);
		expect(actual.report).toEqual({
			seed: "coverage-17",
			actionIndex: 0,
			action: "replace",
			targetKinds: ["fixture"],
			candidates: ["api", "fixtureSheet", "keypad"],
			selected: expect.stringMatching(/^(api|fixtureSheet|keypad)$/),
		});
		expect(firstChoice.reports).toEqual([actual.report]);
	});

	it("uses the action index and can replay an exact recorded choice", async () => {
		const adapters = routes();
		const choice = new SeededSelectionRouteChoice(42, adapters);
		const request = {
			action: "add" as const,
			targets: [fixture(1), fixture(3)],
		};

		await choice.execute(request);
		const recorded = await choice.execute(request);
		const replayAdapters = routes();
		const replay = await new SeededSelectionRouteChoice(
			recorded.report.seed,
			replayAdapters,
		).replay(recorded.report, request);

		expect(recorded.report.actionIndex).toBe(1);
		expect(replay.report).toEqual(recorded.report);
		expect(
			replayAdapters.find(
				(adapter) => adapter.name === recorded.report.selected,
			)?.mutate,
		).toHaveBeenCalledOnce();
	});

	it("rejects unsupported target combinations before invoking mutation", async () => {
		const adapters = routes();
		const choice = new SeededSelectionRouteChoice("unsupported", adapters);

		await expect(
			choice.execute({
				action: "remove",
				targets: [fixture(1), group(2)],
			}),
		).rejects.toBeInstanceOf(UnsupportedSelectionRouteError);
		expect(
			adapters.every((adapter) => !vi.mocked(adapter.mutate).mock.calls.length),
		).toBe(true);

		const supported = await choice.execute({
			action: "replace",
			targets: [fixture(1)],
		});
		expect(supported.report.actionIndex).toBe(0);
	});

	it("rejects replay drift before invoking mutation", async () => {
		const request = {
			action: "replace" as const,
			targets: [fixture(4)],
		};
		const original = await new SeededSelectionRouteChoice(
			"coverage-17",
			routes(),
		).execute(request);
		const replayAdapters = routes().filter(
			(adapter) => adapter.name !== "fixtureSheet",
		);
		const replay = new SeededSelectionRouteChoice(
			"coverage-17",
			replayAdapters,
		);

		await expect(replay.replay(original.report, request)).rejects.toThrow(
			/does not match the recorded choice/,
		);
		expect(
			replayAdapters.every(
				(adapter) => !vi.mocked(adapter.mutate).mock.calls.length,
			),
		).toBe(true);
	});

	it("rejects duplicate route identities and empty seeds during setup", () => {
		expect(
			() =>
				new SeededSelectionRouteChoice("seed", [
					adapter("api", ["fixture"]),
					adapter("api", ["fixture"]),
				]),
		).toThrow(/registered twice/);
		expect(() => new SeededSelectionRouteChoice("  ", routes())).toThrow(
			/seed must not be empty/,
		);
	});
});

function routes(): SelectionRouteAdapter[] {
	return [
		adapter("pool", ["group", "group_range"]),
		adapter("keypad", [
			"fixture",
			"fixture_range",
			"group",
			"group_range",
			"dereferenced_group",
		]),
		adapter("fixtureSheet", ["fixture", "fixture_range"]),
		adapter("api", [
			"fixture",
			"fixture_range",
			"group",
			"group_range",
			"dereferenced_group",
		]),
	];
}

function adapter(
	name: SelectionRouteAdapter["name"],
	targets: SelectionRouteAdapter["capabilities"]["targets"],
): SelectionRouteAdapter {
	return {
		name,
		capabilities: {
			actions: ["replace", "add"],
			targets,
		},
		mutate: vi.fn(async ({ action }) => `${name}:${action}`),
	};
}
