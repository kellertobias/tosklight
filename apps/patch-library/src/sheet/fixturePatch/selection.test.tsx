import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	type PatchHost,
	PatchHostProvider,
	noPatchSelection,
	type PatchSelectionHost,
} from "../../host";
import type { PatchedFixture } from "../../wire";
import {
	fixtureSelectionIds,
	orderedFixtureSelectionIds,
	toggledFixtureSelection,
	usePatchSelection,
} from "./selection";

const FIXTURE_1 = "fixture-1";
const FIXTURE_2 = "fixture-2";

function patched(
	fixtureId: string,
	headIds: readonly string[] = [],
): PatchedFixture {
	return {
		fixture_id: fixtureId,
		logical_heads: headIds.map((fixture_id) => ({ fixture_id })),
	} as unknown as PatchedFixture;
}

const secondFixture = patched(FIXTURE_2);

function host(selection: PatchSelectionHost): PatchHost {
	return {
		library: null,
		selection,
		editArmed: false,
		setEditArmed: () => undefined,
	};
}

function ToggleProbe() {
	const selection = usePatchSelection();
	const selected = selection.orderedFixtureIds;
	return (
		<>
			<output data-testid="patch-selection">
				{selected?.join(",") ?? "none"}
			</output>
			<button
				type="button"
				onClick={() =>
					selection.replace({
						resolvedFixtures: toggledFixtureSelection(
							selected ?? [],
							secondFixture,
						),
					})
				}
			>
				Toggle second fixture
			</button>
		</>
	);
}

afterEach(cleanup);

describe("patch sheet selection", () => {
	it("reads the selection the host supplies", () => {
		render(
			<PatchHostProvider
				value={host({
					fixtureIds: new Set([FIXTURE_1]),
					orderedFixtureIds: [FIXTURE_1],
					replace: () => undefined,
				})}
			>
				<ToggleProbe />
			</PatchHostProvider>,
		);
		expect(screen.getByTestId("patch-selection")).toHaveTextContent(FIXTURE_1);
	});

	it("asks the host to replace the selection with the resolved fixtures", () => {
		const replace = vi.fn();
		render(
			<PatchHostProvider
				value={host({
					fixtureIds: new Set([FIXTURE_1]),
					orderedFixtureIds: [FIXTURE_1],
					replace,
				})}
			>
				<ToggleProbe />
			</PatchHostProvider>,
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Toggle second fixture" }),
		);
		expect(replace).toHaveBeenCalledWith({
			resolvedFixtures: [FIXTURE_1, FIXTURE_2],
		});
	});

	it("keeps working for a host with no programmer selection", () => {
		render(
			<PatchHostProvider value={host(noPatchSelection)}>
				<ToggleProbe />
			</PatchHostProvider>,
		);
		expect(screen.getByTestId("patch-selection")).toHaveTextContent("none");
		expect(() =>
			fireEvent.click(
				screen.getByRole("button", { name: "Toggle second fixture" }),
			),
		).not.toThrow();
	});

	it("resolves a multi-head fixture to its logical heads", () => {
		expect(fixtureSelectionIds(patched("master", ["left", "right"]))).toEqual([
			"left",
			"right",
		]);
		expect(fixtureSelectionIds(patched("plain"))).toEqual(["plain"]);
	});

	it("orders resolved fixtures without repeating a shared head", () => {
		expect(
			orderedFixtureSelectionIds([
				patched("master", ["left", "right"]),
				patched("second", ["right", "far"]),
			]),
		).toEqual(["left", "right", "far"]);
	});

	it("removes all logical heads without disturbing earlier ordered fixtures", () => {
		expect(
			toggledFixtureSelection(
				[FIXTURE_1, "head-left", "head-right"],
				patched("master", ["head-left", "head-right"]),
			),
		).toEqual([FIXTURE_1]);
	});
});
