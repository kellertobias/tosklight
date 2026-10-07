import { describe, expect, it, vi } from "vitest";
import {
	type ParameterValuesMutationPort,
	submitParameterComponentEdits,
} from "./parameterValueMutations";
import type { ParameterProjection } from "./useParameterProjection";

const projection = (overrides: Partial<ParameterProjection> = {}) =>
	({
		selectedGroupId: null,
		programmerValuesRoute: "normal",
		programmerFadeMillis: 2_000,
		...overrides,
	}) as unknown as ParameterProjection;

const edits = [
	{
		kind: "scalar",
		component: { kind: "focus" },
		operation: { kind: "set", value: { kind: "value", value: 0.5 } },
	},
] as const;

describe("parameter value mutation port: component edits", () => {
	it("carries component_edits and the displayed source through applyIntent", () => {
		const applyIntent = vi.fn(async (_input: unknown) => null);
		const port: ParameterValuesMutationPort = { batch: vi.fn(), applyIntent };
		submitParameterComponentEdits(port, projection(), "focus", ["a"], edits, {
			requestId: "r1",
			undoGroup: "u1",
			displayedSource: { lane: "normal", lease: 7 },
		});
		expect(applyIntent).toHaveBeenCalledWith({
			requestId: "r1",
			fixtureIds: ["a"],
			attribute: "focus",
			operation: { type: "component_edits", edits },
			undoGroup: "u1",
			timing: { fade: false, fadeMillis: null, delayMillis: null },
			displayedSource: { lane: "normal", lease: 7 },
		});
	});

	it("uses the Preload fade, targets a group, and is a quiet no-op without targets", () => {
		const applyIntent = vi.fn(async (_input: unknown) => null);
		const port: ParameterValuesMutationPort = { batch: vi.fn(), applyIntent };
		submitParameterComponentEdits(
			port,
			projection({ programmerValuesRoute: "preload", selectedGroupId: "g" }),
			"focus",
			["a"],
			edits,
		);
		expect(applyIntent.mock.calls[0]?.[0]).toMatchObject({
			fixtureIds: [],
			groupId: "g",
			timing: { fade: true, fadeMillis: 2_000 },
		});
		expect(submitParameterComponentEdits(port, projection(), "focus", [], edits)).toBeNull();
		expect(submitParameterComponentEdits(port, projection(), "focus", ["a"], [])).toBeNull();
		expect(applyIntent).toHaveBeenCalledOnce();
	});
});
