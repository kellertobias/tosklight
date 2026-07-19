import { useCallback, useMemo } from "react";
import type { SelectionActionOutcome } from "../../features/programmingInteraction/contracts";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../../features/programmingInteraction/ProgrammingInteractionView";

const EMPTY_FIXTURE_IDS: readonly string[] = [];

export interface StageSelectionModel {
	fixtureIds: readonly string[];
	fixtureIdSet: ReadonlySet<string>;
	firstFixtureId: string | null;
	applyFixtureGesture(
		fixtureId: string,
		operation?: "add" | "remove",
	): Promise<SelectionActionOutcome | null>;
	replaceFixtureIds(
		fixtureIds: readonly string[],
	): Promise<SelectionActionOutcome | null>;
	clear(): Promise<SelectionActionOutcome | null>;
}

/** Selection state and semantic mutations used by every Stage rendering path. */
export function useStageSelection(active = true): StageSelectionModel {
	const projection = useProgrammingSelectionView(active);
	const actions = useProgrammingSelectionActions(active);
	const fixtureIds = projection?.selected ?? EMPTY_FIXTURE_IDS;
	const fixtureIdSet = useMemo(() => new Set(fixtureIds), [fixtureIds]);
	const applyFixtureGesture = useCallback(
		(fixtureId: string, operation: "add" | "remove" = "add") => {
			if (!actions || !fixtureId) return Promise.resolve(null);
			return actions.gesture({
				source: { type: "fixture", fixtureId },
				resolvedFixtures: [fixtureId],
				operation,
			});
		},
		[actions],
	);
	const replaceFixtureIds = useCallback(
		(nextFixtureIds: readonly string[]) =>
			actions?.replace({ resolvedFixtures: nextFixtureIds }) ??
			Promise.resolve(null),
		[actions],
	);
	const clear = useCallback(
		() => replaceFixtureIds(EMPTY_FIXTURE_IDS),
		[replaceFixtureIds],
	);
	return useMemo(
		() => ({
			fixtureIds,
			fixtureIdSet,
			firstFixtureId: fixtureIds[0] ?? null,
			applyFixtureGesture,
			replaceFixtureIds,
			clear,
		}),
		[
			applyFixtureGesture,
			clear,
			fixtureIds,
			fixtureIdSet,
			replaceFixtureIds,
		],
	);
}
