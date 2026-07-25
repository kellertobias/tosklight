import type { DmxSnapshot, PatchSnapshot } from "../../../src/api/types";
import type { ApiDriver } from "../core/api";
import {
	describeExpectedDmxByte,
	dmxByteAccepts,
	type FixtureDMXExpectation,
	type FixtureDMXTarget,
	validateFixtureDmxExpectations,
} from "./fixtureDmxContract";
import {
	assignedFixtureDmxTargets,
	type ResolvedFixtureDmxComponent,
	resolveFixtureDmxComponents,
} from "./fixtureDmxResolver";

export {
	type ExpectedDMXByte,
	type FixtureDMXExpectation,
	type FixtureDMXTarget,
	type FixtureQualifier,
	type FixtureRangeReference,
	type FixtureReference,
	fixture,
	fixtureRange,
} from "./fixtureDmxContract";

interface FixtureDmxSource {
	patch(): Promise<PatchSnapshot>;
	request<T>(
		method: string,
		path: string,
		body?: unknown,
		authenticate?: boolean,
	): Promise<T>;
}

interface LastObservation {
	patchRevision: number;
	frame: DmxSnapshot;
	resolved: ResolvedFixtureDmxComponent[];
	mismatches: string[];
}

const DEFAULT_TIMEOUT = 2_000;

export class FixtureDmxAssertions {
	constructor(
		private readonly source: FixtureDmxSource,
		private readonly timeout = DEFAULT_TIMEOUT,
		private readonly applicationTime: () => string = () => "(unavailable)",
	) {}

	async expect(
		target: FixtureDMXTarget,
		expected: FixtureDMXExpectation,
	): Promise<void> {
		const entries = validateFixtureDmxExpectations(expected);
		const deadline = Date.now() + this.timeout;
		let last: LastObservation | undefined;
		do {
			const before = await this.source.patch();
			const resolved = resolveFixtureDmxComponents(before, target, entries);
			const frame = await this.source.request<DmxSnapshot>(
				"GET",
				"/api/v2/output/dmx",
				undefined,
				false,
			);
			const after = await this.source.patch();
			if (after.revision !== before.revision) continue;
			const mismatches = compareFrame(resolved, frame);
			last = { patchRevision: before.revision, frame, resolved, mismatches };
			if (mismatches.length === 0) return;
			await pollDelay();
		} while (Date.now() < deadline);
		throw new Error(formatMismatch(last, this.applicationTime()));
	}

	async expectAbsent(target: FixtureDMXTarget): Promise<void> {
		const failures = assignedFixtureDmxTargets(
			await this.source.patch(),
			target,
		);
		if (failures.length)
			throw new Error(
				`Expected no DMX assignment:\n${failures.map((failure) => `- ${failure}`).join("\n")}`,
			);
	}
}

export function fixtureDmxAssertions(api: ApiDriver): FixtureDmxAssertions {
	return new FixtureDmxAssertions(api);
}

function compareFrame(
	resolved: readonly ResolvedFixtureDmxComponent[],
	frame: DmxSnapshot,
): string[] {
	return resolved.flatMap((component) => {
		const actual = frame.universes.find(
			(candidate) => candidate.universe === component.universe,
		)?.slots[component.address - 1];
		return actual !== undefined && dmxByteAccepts(component.expected, actual)
			? []
			: [
					`${component.description} actual ${actual === undefined ? "(universe/frame unavailable)" : actual}, expected ${describeExpectedDmxByte(component.expected)}`,
				];
	});
}

function formatMismatch(
	last: LastObservation | undefined,
	applicationTime: string,
): string {
	if (!last)
		return `Fixture DMX assertion could not obtain a stable patch snapshot at ${applicationTime}`;
	return [
		`Fixture DMX did not match the latest logical frame at ${applicationTime} (patch revision ${last.patchRevision}, frame revision ${last.frame.revision}):`,
		...last.mismatches.map((mismatch) => `- ${mismatch}`),
	].join("\n");
}

function pollDelay(): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, 10));
}
