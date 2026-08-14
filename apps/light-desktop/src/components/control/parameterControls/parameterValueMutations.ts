import type { AttributeValue } from "../../../api/types";
import type {
	BatchProgrammerValuesInput,
	ProgrammerValuesMutation,
	ProgrammerValueTiming,
} from "../../../features/programmerValues/contracts";
import type { ParameterProjection } from "./useParameterProjection";

export interface ParameterValuesMutationPort {
	batch(input: BatchProgrammerValuesInput): Promise<unknown>;
	applyIntent?(input: {
		requestId: string;
		fixtureIds: readonly string[];
		groupId?: string | null;
		attribute: string;
		operation:
			| { type: "absolute_set"; value: AttributeValue }
			| { type: "relative_step"; delta: number };
		undoGroup?: string | null;
		timing: ProgrammerValueTiming;
	}): Promise<unknown>;
	applyIndexedPreset?(input: {
		requestId: string;
		expectedSelectionRevision: number;
		attribute: string;
		targets: IndexedPresetMutationTargets;
	}): Promise<unknown>;
}

type IndexedPresetMutationTargets = ReadonlyArray<{
	fixtureId: string;
	functionId: string;
	expectedProfileRevision: number;
}>;

function fixtureTargets(projection: ParameterProjection, attribute: string) {
	return projection.selectedGroupId
		? []
		: (projection.supportedFixtureIdsByAttribute.get(attribute) ?? []);
}

export function parameterValueTiming(
	programmerFadeMillis: number | undefined,
): ProgrammerValueTiming {
	return {
		fade: true,
		fadeMillis: programmerFadeMillis ?? 3_000,
		delayMillis: null,
	};
}

export function setParameterMutations(
	projection: ParameterProjection,
	attribute: string,
	value: AttributeValue,
	timingOverride?: ProgrammerValueTiming,
) {
	const timing =
		timingOverride ?? parameterValueTiming(projection.programmerFadeMillis);
	if (projection.selectedGroupId)
		return [
			{
				action: "set_group",
				groupId: projection.selectedGroupId,
				attribute,
				value,
				timing,
			},
		] satisfies ProgrammerValuesMutation[];
	return fixtureTargets(projection, attribute).map(
		(fixtureId): ProgrammerValuesMutation => ({
			action: "set_fixture",
			fixtureId,
			attribute,
			value,
			timing,
		}),
	);
}

export function setParameterRangeMutations(
	projection: ParameterProjection,
	attribute: string,
	percentages: readonly number[],
) {
	const points = percentages.map(normalizePercentage);
	if (projection.selectedGroupId)
		return setParameterMutations(projection, attribute, {
			kind: "spread",
			value: points,
		});
	const fixtureIds = fixtureTargets(projection, attribute);
	if (fixtureIds.length === 0) return [];
	if (projection.programmerValuesRoute === "preload") {
		const timing = parameterValueTiming(projection.programmerFadeMillis);
		return resolveSpread(points, fixtureIds.length).map(
			(value, index): ProgrammerValuesMutation => ({
				action: "set_fixture",
				fixtureId: fixtureIds[index] as string,
				attribute,
				value: { kind: "normalized", value },
				timing,
			}),
		);
	}
	// The server resolves the per-fixture interpolation across the ordered selection.
	return [
		{
			action: "set_selection",
			fixtureIds,
			attribute,
			value: { kind: "spread", value: points },
			timing: parameterValueTiming(projection.programmerFadeMillis),
		},
	] satisfies ProgrammerValuesMutation[];
}

export function releaseParameterMutations(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId)
		return projection.groupProgrammerValues.some(
			(entry) => entry.attribute === attribute,
		)
			? ([
					{
						action: "release_group",
						groupId: projection.selectedGroupId,
						attribute,
					},
				] satisfies ProgrammerValuesMutation[])
			: [];
	const valuedFixtures = new Set(
		projection.programmerValues
			.filter((entry) => entry.attribute === attribute)
			.map((entry) => entry.fixtureId),
	);
	return projection.selectedFixtureIds.flatMap((fixtureId) =>
		valuedFixtures.has(fixtureId)
			? ([
					{
						action: "release_fixture",
						fixtureId,
						attribute,
					},
				] satisfies ProgrammerValuesMutation[])
			: [],
	);
}

export function submitParameterMutations(
	actions: ParameterValuesMutationPort | null,
	mutations: readonly ProgrammerValuesMutation[],
	requestId: () => string = () => crypto.randomUUID(),
) {
	if (!actions || mutations.length === 0) return Promise.resolve(null);
	return actions.batch({ requestId: requestId(), mutations });
}

export function submitParameterStep(
	actions: ParameterValuesMutationPort | null,
	projection: ParameterProjection,
	attribute: string,
	delta: number,
	requestId: () => string = () => crypto.randomUUID(),
	undoGroup?: string | null,
) {
	if (
		!actions ||
		(!projection.selectedGroupId && fixtureTargets(projection, attribute).length === 0)
	)
		return Promise.resolve(null);
	if (!actions.applyIntent) {
		const base =
			projection.normalized.get(attribute) ??
			projection.programmerValues.find((value) => value.attribute === attribute)
				?.value;
		const normalized =
			typeof base === "number"
				? base
				: base?.kind === "normalized"
					? base.value
					: 0;
		return submitParameterMutations(
			actions,
			setParameterMutations(projection, attribute, {
				kind: "normalized",
				value: Math.max(0, Math.min(1, normalized + delta)),
			}),
			requestId,
		);
	}
	return actions.applyIntent({
		requestId: requestId(),
		fixtureIds: fixtureTargets(projection, attribute),
		...(projection.selectedGroupId
			? { groupId: projection.selectedGroupId }
			: {}),
		attribute,
		operation: { type: "relative_step", delta },
		...(undoGroup ? { undoGroup } : {}),
		timing:
			projection.programmerValuesRoute === "preload"
				? parameterValueTiming(projection.programmerFadeMillis)
				: immediateParameterTiming(),
	});
}

export function submitParameterAbsoluteIntent(
	actions: ParameterValuesMutationPort | null,
	projection: ParameterProjection,
	attribute: string,
	value: AttributeValue,
	requestId: () => string = () => crypto.randomUUID(),
	undoGroup?: string | null,
) {
	if (
		!actions?.applyIntent ||
		(!projection.selectedGroupId && fixtureTargets(projection, attribute).length === 0)
	)
		return null;
	return actions.applyIntent({
		requestId: requestId(),
		fixtureIds: fixtureTargets(projection, attribute),
		...(projection.selectedGroupId
			? { groupId: projection.selectedGroupId }
			: {}),
		attribute,
		operation: { type: "absolute_set", value },
		...(undoGroup ? { undoGroup } : {}),
		timing:
			projection.programmerValuesRoute === "preload" ||
			projection.directEntryUsesProgrammerFade
				? parameterValueTiming(projection.programmerFadeMillis)
				: immediateParameterTiming(),
	});
}

export function immediateParameterTiming(): ProgrammerValueTiming {
	return { fade: false, fadeMillis: null, delayMillis: null };
}

export function parameterMutationKey(
	mutations: readonly ProgrammerValuesMutation[],
) {
	return mutations
		.map((mutation) => {
			if (mutation.action === "set_group")
				return `group:${mutation.groupId}:${mutation.attribute}`;
			if (mutation.action === "set_fixture")
				return `fixture:${mutation.fixtureId}:${mutation.attribute}`;
			if (mutation.action === "set_selection")
				return `selection:${mutation.fixtureIds.join(",")}:${mutation.attribute}`;
			return mutation.action;
		})
		.join("\u0000");
}

function normalizePercentage(value: number) {
	return Math.max(0, Math.min(100, value)) / 100;
}

/** Mirrors the core anchor rule for the Preload protocol's fixture-only writes. */
function resolveSpread(points: readonly number[], count: number) {
	if (count === 0) return [];
	const first = points[0] ?? 0;
	if (points.length <= 1 || count === 1)
		return Array.from({ length: count }, () => first);
	if (points.length > count)
		return Array.from({ length: count }, (_, index) => {
			const position = (index * (points.length - 1)) / (count - 1);
			const left = Math.floor(position);
			const right = Math.ceil(position);
			return (
				(points[left] ?? first) +
				((points[right] ?? first) - (points[left] ?? first)) * (position - left)
			);
		});
	const anchors: Array<[number, number]> = [];
	const denominator = points.length - 1;
	for (let point = 0; point < points.length; point++) {
		const numerator = point * (count - 1);
		const item = Math.floor(numerator / denominator);
		const remainder = numerator % denominator;
		const value = points[point] ?? first;
		if (remainder === 0) anchors.push([item, value]);
		else if (remainder * 2 === denominator)
			anchors.push([item, value], [item + 1, value]);
		else anchors.push([item + (remainder * 2 > denominator ? 1 : 0), value]);
	}
	const resolved = Array.from({ length: count }, () => 0);
	for (let index = 0; index + 1 < anchors.length; index++) {
		const [leftItem, leftValue] = anchors[index] as [number, number];
		const [rightItem, rightValue] = anchors[index + 1] as [number, number];
		resolved[leftItem] = leftValue;
		resolved[rightItem] = rightValue;
		const span = Math.max(0, rightItem - leftItem);
		for (let step = 1; step < span; step++)
			resolved[leftItem + step] =
				(leftValue * (span - step) + rightValue * step) / span;
	}
	return resolved;
}
