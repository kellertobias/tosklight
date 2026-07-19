import type {
	SelectionExpression,
	SelectionGestureSource,
	SelectionProjection,
	SelectionReference,
	SelectionRule,
} from "./contracts";

export type SelectionReducer = (
	current: SelectionProjection,
) => SelectionProjection;

export function replaceSelectionPrediction(
	fixtures: readonly string[],
): SelectionReducer {
	const selected = unique(fixtures);
	return (current) => ({
		...current,
		selected,
		expression: { type: "static" },
		gestureOpen: false,
	});
}

export function gestureSelectionPrediction(
	source: SelectionGestureSource,
	resolvedFixtures: readonly string[],
	remove: boolean,
): SelectionReducer {
	const fixtures = unique(resolvedFixtures);
	const references = gestureReferences(source, fixtures, remove);
	return (current) => {
		const openExpression =
			current.gestureOpen && current.expression?.type === "sources"
				? current.expression
				: null;
		const selected = updateSelected(
			openExpression ? current.selected : [],
			fixtures,
			remove,
		);
		const items = openExpression?.items ?? [];
		return {
			...current,
			selected,
			expression: { type: "sources", items: [...items, ...references] },
			gestureOpen: true,
		};
	};
}

export function groupSelectionPrediction(
	groupId: string,
	fixtures: readonly string[],
	frozen: boolean,
	rule: SelectionRule,
	showRevision: number,
): SelectionReducer {
	const selected = applySelectionRule(unique(fixtures), rule);
	const expression: SelectionExpression = frozen
		? { type: "frozen_group", groupId, sourceRevision: showRevision }
		: { type: "live_group", groupId, rule };
	return (current) => ({
		...current,
		selected,
		expression,
		gestureOpen: false,
	});
}

export function ruleSelectionPrediction(
	rule: SelectionRule,
): SelectionReducer {
	return (current) => {
		validateSelectionRule(rule);
		const liveGroup =
			current.expression?.type === "live_group"
				? current.expression
				: null;
		// A filtered live Group does not retain its unfiltered membership locally.
		// Keep the complete projection stable until authority arrives rather than
		// exposing a new rule beside members produced by the old rule.
		if (liveGroup && liveGroup.rule.type !== "all") return current;
		const selected = applySelectionRule(current.selected, rule);
		return {
			...current,
			selected,
			expression: liveGroup
				? { ...liveGroup, rule }
				: { type: "static" },
			gestureOpen: false,
		};
	};
}

export function applySelectionRule(
	fixtures: readonly string[],
	rule: SelectionRule,
) {
	validateSelectionRule(rule);
	return fixtures.filter((_, index) => {
		const oneBased = index + 1;
		switch (rule.type) {
			case "all":
				return true;
			case "odd":
				return oneBased % 2 === 1;
			case "even":
				return oneBased % 2 === 0;
			case "every_nth":
				return index >= rule.offset && (index - rule.offset) % rule.n === 0;
		}
	});
}

function validateSelectionRule(rule: SelectionRule) {
	if (rule.type !== "every_nth") return;
	if (!Number.isSafeInteger(rule.n) || rule.n < 1)
		throw new Error("every-Nth selection requires a positive integer interval");
	if (!Number.isSafeInteger(rule.offset) || rule.offset < 0)
		throw new Error("every-Nth selection requires a non-negative integer offset");
}

function gestureReferences(
	source: SelectionGestureSource,
	fixtures: readonly string[],
	remove: boolean,
): SelectionReference[] {
	if (source.type === "live_group")
		return [
			remove
				? { type: "remove_live_group", groupId: source.groupId }
				: { type: "live_group", groupId: source.groupId },
		];
	return fixtures.map((fixtureId) => ({
		type: remove ? "remove_fixture" : "fixture",
		fixtureId,
	}));
}

function updateSelected(
	current: readonly string[],
	fixtures: readonly string[],
	remove: boolean,
) {
	if (remove) {
		const removed = new Set(fixtures);
		return current.filter((fixture) => !removed.has(fixture));
	}
	return unique([...current, ...fixtures]);
}

function unique(fixtures: readonly string[]) {
	return [...new Set(fixtures)];
}
