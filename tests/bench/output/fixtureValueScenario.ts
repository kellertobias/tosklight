import { expect } from "@playwright/test";
import type { VisualizationSnapshot } from "../../../apps/light-desktop/src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { FixtureReference } from "./fixtureDmxContract";

/**
 * Expected resolved values by address. A plain address (`intensity`, `gobo`) is a normalized
 * value from 0 through 1. Since the TL-552 cutover Color and Position are semantic families, so
 * their components are addressed as `owner:component`:
 *
 * - `color:red`, `color:green`, `color:blue`, `color:amber`: the semantic recipe, 0 through 1;
 * - `color:white_blend`, `color:uv`, `color:relative_output`: 0 through 1;
 * - `position:pan`, `position:tilt`: resolved Angles in degrees.
 */
export type FixtureValueExpectation = Readonly<Record<string, number>>;

const COLOR_COMPONENTS = new Set([
	"red",
	"green",
	"blue",
	"amber",
	"white_blend",
	"uv",
	"relative_output",
]);
const POSITION_COMPONENTS = new Set(["pan", "tilt"]);

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
			const address = parseAddress(attribute);
			if (!Number.isFinite(value))
				throw new Error(`Fixture value for ${attribute} must be finite`);
			if (!address.degrees && (value < 0 || value > 1))
				throw new Error(
					`Fixture value for ${attribute} must be normalized from 0 through 1`,
				);
			await expect
				.poll(async () => this.resolvedValue(fixtureId, address), {
					message: `Fixture ${target.number} ${attribute} should resolve to ${value}`,
				})
				// Angles resolve in degrees; 0.01° is far below one DMX step of any lamp.
				.toBeCloseTo(value, address.degrees ? 2 : 5);
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

	private async resolvedValue(
		fixtureId: string,
		address: ValueAddress,
	): Promise<number | undefined> {
		const snapshot = await this.api.request<VisualizationSnapshot>(
			"GET",
			"/api/v2/output/visualization",
		);
		const value: unknown = snapshot.values.find(
			(candidate) =>
				candidate.fixture_id === fixtureId &&
				candidate.attribute === address.attribute,
		)?.value;
		if (address.component) return familyComponent(value, address);
		if (typeof value === "number") return value;
		return isNormalizedValue(value) ? value.value : 0;
	}
}

interface ValueAddress {
	attribute: string;
	component?: string;
	degrees: boolean;
}

function parseAddress(text: string): ValueAddress {
	if (!text.trim()) throw new Error("Fixture value attribute must not be empty");
	const [owner, component, ...rest] = text.split(":");
	if (component === undefined) return { attribute: owner, degrees: false };
	if (rest.length === 0 && owner === "color" && COLOR_COMPONENTS.has(component))
		return { attribute: "color", component, degrees: false };
	if (
		rest.length === 0 &&
		owner === "position" &&
		POSITION_COMPONENTS.has(component)
	)
		return { attribute: "position", component, degrees: true };
	throw new Error(
		`Fixture value address ${text} is not a known semantic family component`,
	);
}

/**
 * One component of a resolved semantic family value. An absent or non-semantic Color resolves
 * every component to 0, as an absent scalar did before the cutover; absent Angles are
 * `undefined`, because 0° is a real pose.
 */
function familyComponent(
	value: unknown,
	address: ValueAddress,
): number | undefined {
	const family = record(value);
	if (address.attribute === "position") {
		const angles = record(family?.value);
		if (family?.kind !== "position" || angles?.kind !== "angles")
			return undefined;
		const scalar = record(angles[`${address.component}_degrees`]);
		return scalar?.kind === "value" && typeof scalar.value === "number"
			? scalar.value
			: undefined;
	}
	const program = record(family?.value);
	const intent = record(program?.intent);
	if (
		family?.kind !== "color_program" ||
		program?.kind !== "semantic" ||
		!intent
	)
		return 0;
	const recipe = record(intent.recipe);
	switch (address.component) {
		case "red":
		case "green":
		case "blue": {
			const rgb = recipe?.rgb;
			const index = ["red", "green", "blue"].indexOf(address.component);
			return Array.isArray(rgb) && typeof rgb[index] === "number"
				? rgb[index]
				: 0;
		}
		case "amber":
			return typeof recipe?.amber === "number" ? recipe.amber : 0;
		case "white_blend":
			return typeof intent.white_blend === "number" ? intent.white_blend : 0;
		case "uv": {
			const uv = record(intent.uv);
			return typeof uv?.amount === "number" ? uv.amount : 0;
		}
		case "relative_output":
			return typeof intent.relative_output === "number"
				? intent.relative_output
				: 0;
		default:
			return undefined;
	}
}

function record(value: unknown): Record<string, unknown> | undefined {
	return typeof value === "object" && value !== null && !Array.isArray(value)
		? (value as Record<string, unknown>)
		: undefined;
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
