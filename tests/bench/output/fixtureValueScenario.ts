import { expect } from "@playwright/test";
import type { VisualizationSnapshot } from "../../../apps/light-desktop/src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { FixtureReference } from "./fixtureDmxContract";

export type FixtureValueExpectation = Readonly<Record<string, number>>;

/** Assertions over resolved logical fixture values, including virtual attributes without DMX channels. */
export class FixtureValueAssertions {
	constructor(private readonly api: ApiDriver) {}

	async expect(
		target: FixtureReference,
		expected: FixtureValueExpectation,
	): Promise<void> {
		const entries = Object.entries(expected);
		if (entries.length === 0)
			throw new Error(
				"Fixture value expectation must name at least one attribute",
			);
		const fixtureId = await this.fixtureId(target);
		for (const [attribute, value] of entries) {
			if (!attribute.trim())
				throw new Error("Fixture value attribute must not be empty");
			if (!Number.isFinite(value) || value < 0 || value > 1)
				throw new Error(
					`Fixture value for ${attribute} must be normalized from 0 through 1`,
				);
			await expect
				.poll(async () => this.normalizedValue(fixtureId, attribute), {
					message: `Fixture ${target.number} ${attribute} should resolve to ${value}`,
				})
				.toBeCloseTo(value, 5);
		}
	}

	private async fixtureId(target: FixtureReference): Promise<string> {
		if (target.head != null || target.multipatch != null)
			throw new Error(
				"Resolved fixture-value assertions currently address whole fixtures",
			);
		const fixture = (await this.api.patch()).fixtures.find(
			(candidate) => candidate.fixture_number === target.number,
		);
		if (!fixture) throw new Error(`Fixture ${target.number} is not patched`);
		return fixture.fixture_id;
	}

	private async normalizedValue(
		fixtureId: string,
		attribute: string,
	): Promise<number | undefined> {
		const snapshot = await this.api.request<VisualizationSnapshot>(
			"GET",
			"/api/v2/output/visualization",
		);
		const value: unknown = snapshot.values.find(
			(candidate) =>
				candidate.fixture_id === fixtureId && candidate.attribute === attribute,
		)?.value;
		if (typeof value === "number") return value;
		return isNormalizedValue(value) ? value.value : 0;
	}
}

function isNormalizedValue(
	value: unknown,
): value is { kind: "normalized"; value: number } {
	return (
		typeof value === "object" &&
		value !== null &&
		"kind" in value &&
		value.kind === "normalized" &&
		"value" in value &&
		typeof value.value === "number"
	);
}
