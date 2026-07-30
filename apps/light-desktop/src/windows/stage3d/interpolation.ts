import type { AttributeValue, VisualizationSnapshot } from "../../api/types";

// Leave the final portion of the 100 ms publication cadence for both the
// interpolation callback and the separate demand-render callback. Settling at
// the authoritative value early is non-predictive and prevents the next source
// sample from cancelling the preceding sample before its canvas submission.
export const STAGE_INTERPOLATION_MILLIS = 60;
export const MAX_INTERPOLATED_STAGE_FIXTURES = 96;
export const MAX_INTERPOLATED_STAGE_SCENE_FIXTURES = 160;

export function stageVisualizationChanged(
	from: VisualizationSnapshot,
	to: VisualizationSnapshot,
) {
	return (
		from.preload !== to.preload ||
		from.blackout !== to.blackout ||
		from.grand_master !== to.grand_master ||
		!sameEntries(from.values, to.values) ||
		!sameEntries(
			from.profile_output_values ?? [],
			to.profile_output_values ?? [],
		)
	);
}

export function changedStageFixtureIds(
	from: VisualizationSnapshot,
	to: VisualizationSnapshot,
) {
	if (
		from.preload !== to.preload ||
		from.blackout !== to.blackout ||
		from.grand_master !== to.grand_master
	)
		return null;
	const changed = new Set<string>();
	collectChangedFixtureIds(from.values, to.values, changed);
	collectChangedFixtureIds(
		from.profile_output_values ?? [],
		to.profile_output_values ?? [],
		changed,
	);
	return changed;
}

export function shouldInterpolateStageChanges(
	changedFixtureIds: ReadonlySet<string> | null,
) {
	return (
		changedFixtureIds !== null &&
		changedFixtureIds.size > 0 &&
		changedFixtureIds.size <= MAX_INTERPOLATED_STAGE_FIXTURES
	);
}

export function shouldInterpolateStageSceneChanges(
	sceneFixtureCount: number,
	changedFixtureIds: ReadonlySet<string> | null,
) {
	return (
		sceneFixtureCount <= MAX_INTERPOLATED_STAGE_SCENE_FIXTURES &&
		shouldInterpolateStageChanges(changedFixtureIds)
	);
}

export function remainingStageInterpolationMillis(
	generatedAt: string,
	now = Date.now(),
) {
	const sourceAt = Date.parse(generatedAt);
	const sourceAge = Number.isFinite(sourceAt) ? Math.max(0, now - sourceAt) : 0;
	return Math.max(0, STAGE_INTERPOLATION_MILLIS - sourceAge);
}

export function interpolateVisualizationSnapshot(
	from: VisualizationSnapshot,
	to: VisualizationSnapshot,
	progress: number,
): VisualizationSnapshot {
	const amount = Math.max(0, Math.min(1, progress));
	if (
		amount >= 1 ||
		from.preload !== to.preload ||
		from.blackout !== to.blackout
	)
		return to;
	return {
		...to,
		grand_master: lerp(from.grand_master, to.grand_master, amount),
		values: interpolateEntries(from.values, to.values, amount),
		profile_output_values: interpolateEntries(
			from.profile_output_values ?? [],
			to.profile_output_values ?? [],
			amount,
		),
	};
}

function interpolateEntries<
	T extends {
		fixture_id: string;
		attribute: string;
		value: AttributeValue;
	},
>(from: readonly T[], to: readonly T[], progress: number): T[] {
	const previous = new Map(
		from.map((entry) => [entryKey(entry), entry.value] as const),
	);
	return to.map((entry) => {
		const fromValue = previous.get(entryKey(entry));
		return fromValue
			? {
					...entry,
					value: interpolateAttribute(fromValue, entry.value, progress),
				}
			: entry;
	});
}

function interpolateAttribute(
	from: AttributeValue,
	to: AttributeValue,
	progress: number,
): AttributeValue {
	if (from.kind !== to.kind) return to;
	switch (to.kind) {
		case "normalized":
			return {
				kind: "normalized",
				value: lerp(
					(from as Extract<AttributeValue, { kind: "normalized" }>).value,
					to.value,
					progress,
				),
			};
		case "raw_dmx":
		case "raw_dmx_exact":
			return {
				kind: to.kind,
				value: lerp(
					(
						from as Extract<
							AttributeValue,
							{ kind: "raw_dmx" | "raw_dmx_exact" }
						>
					).value,
					to.value,
					progress,
				),
			};
		case "spread": {
			const values = (from as Extract<AttributeValue, { kind: "spread" }>)
				.value;
			if (values.length !== to.value.length) return to;
			return {
				kind: "spread",
				value: to.value.map((value, index) =>
					lerp(values[index] ?? value, value, progress),
				),
			};
		}
		case "color_xyz": {
			const value = (from as Extract<AttributeValue, { kind: "color_xyz" }>)
				.value;
			return {
				kind: "color_xyz",
				value: {
					x: lerp(value.x, to.value.x, progress),
					y: lerp(value.y, to.value.y, progress),
					z: lerp(value.z, to.value.z, progress),
				},
			};
		}
		case "discrete":
			return to;
	}
}

function entryKey(entry: { fixture_id: string; attribute: string }) {
	return `${entry.fixture_id}\u0000${entry.attribute}`;
}

function sameEntries<
	T extends {
		fixture_id: string;
		attribute: string;
		value: AttributeValue;
	},
>(left: readonly T[], right: readonly T[]) {
	if (left.length !== right.length) return false;
	for (let index = 0; index < left.length; index++) {
		const leftEntry = left[index];
		const rightEntry = right[index];
		if (
			!leftEntry ||
			!rightEntry ||
			leftEntry.fixture_id !== rightEntry.fixture_id ||
			leftEntry.attribute !== rightEntry.attribute ||
			!sameAttribute(leftEntry.value, rightEntry.value)
		)
			return false;
	}
	return true;
}

function collectChangedFixtureIds<
	T extends {
		fixture_id: string;
		attribute: string;
		value: AttributeValue;
	},
>(left: readonly T[], right: readonly T[], changed: Set<string>) {
	const previous = new Map(
		left.map((entry) => [entryKey(entry), entry] as const),
	);
	for (const entry of right) {
		const before = previous.get(entryKey(entry));
		if (!before || !sameAttribute(before.value, entry.value))
			changed.add(entry.fixture_id);
		previous.delete(entryKey(entry));
	}
	for (const entry of previous.values()) changed.add(entry.fixture_id);
}

function sameAttribute(left: AttributeValue, right: AttributeValue) {
	if (left.kind !== right.kind) return false;
	switch (left.kind) {
		case "normalized":
		case "raw_dmx":
		case "raw_dmx_exact":
		case "discrete":
			return left.value === right.value;
		case "spread":
			return (
				left.value.length ===
					(right as Extract<AttributeValue, { kind: "spread" }>).value.length &&
				left.value.every(
					(value, index) =>
						value ===
						(right as Extract<AttributeValue, { kind: "spread" }>).value[index],
				)
			);
		case "color_xyz": {
			const value = (right as Extract<AttributeValue, { kind: "color_xyz" }>)
				.value;
			return (
				left.value.x === value.x &&
				left.value.y === value.y &&
				left.value.z === value.z
			);
		}
	}
}

function lerp(from: number, to: number, progress: number) {
	return from + (to - from) * progress;
}
