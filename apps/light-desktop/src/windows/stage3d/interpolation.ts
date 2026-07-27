import type { AttributeValue, VisualizationSnapshot } from "../../api/types";

export const STAGE_INTERPOLATION_MILLIS = 100;

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

function lerp(from: number, to: number, progress: number) {
	return from + (to - from) * progress;
}
