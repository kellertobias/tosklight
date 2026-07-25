import { describe, expect, it } from "vitest";
import { retainEquivalent } from "./pollingEquivalence";

describe("polled value equivalence", () => {
	it("keeps the previous reference for an equivalent freshly decoded value", () => {
		const current = {
			active: true,
			mode: "step",
			remembered: [{ fixture_id: "a", number: 1 }],
			active_fixture: { fixture_id: "a", number: 1 },
		};
		const next = structuredClone(current);

		expect(retainEquivalent(current, next)).toBe(current);
	});

	it("adopts the next value when any nested field changes", () => {
		const current = { remembered: [{ fixture_id: "a", number: 1 }] };
		const next = { remembered: [{ fixture_id: "a", number: 2 }] };

		expect(retainEquivalent(current, next)).toBe(next);
	});

	it("adopts the next value when a field is added or removed", () => {
		const current: Record<string, unknown> = { active: true };
		const extended: Record<string, unknown> = { active: true, message: "x" };

		expect(retainEquivalent(current, extended)).toBe(extended);
		expect(retainEquivalent(extended, current)).toBe(current);
	});

	it("handles null transitions in both directions", () => {
		expect(retainEquivalent(null, null)).toBeNull();
		const next = { active: true };
		expect(retainEquivalent(null, next)).toBe(next);
		expect(retainEquivalent(next, null)).toBeNull();
	});

	it("treats an unserializable value as changed rather than equivalent", () => {
		const cyclic: Record<string, unknown> = {};
		cyclic.self = cyclic;

		expect(retainEquivalent({ active: true }, cyclic)).toBe(cyclic);
	});
});
