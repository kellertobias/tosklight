import { useCallback, useMemo } from "react";
import type { SelectionActionOutcome } from "../programmingInteraction/contracts";
import {
	type ProgrammingSelectionActions,
	useProgrammingSelectionActions,
} from "../programmingInteraction/ProgrammingInteractionView";
import type { StagedSelectionWrite } from "../programmingInteraction/selectionWriter";
import type { ShowObject } from "../showObjects/contracts";
import { useShowObjectsStore } from "../showObjects/ShowObjectsState";
import type { ShowObjectsStore } from "../showObjects/store";

type GroupObject = ShowObject<"group">;

/** Group activation selects the complete ordered membership; filtering is a later rule. */
const WHOLE_GROUP_RULE = { type: "all" } as const;

export type GroupSelectionWrite = Promise<SelectionActionOutcome | null> | null;

export interface GroupSelectionActions {
	/** Live activation keeps the Group relationship; membership changes retarget it. */
	selectLive(group: GroupObject): GroupSelectionWrite;
	stageLive(group: GroupObject): StagedSelectionWrite | null;
	/** Frozen activation captures the membership as it exists at this Show revision. */
	selectFrozen(group: GroupObject): GroupSelectionWrite;
}

interface CapturedGroupAuthority {
	actions: ProgrammingSelectionActions;
}

/**
 * Captures the Programming and Group authority owned by the touch that starts a
 * selection, so a Show or session replacement arriving afterwards cannot retarget
 * the write into the replacement scope. A missing authority refuses the mutation
 * rather than falling back to legacy selection state.
 */
function captureGroupAuthority(
	actions: ProgrammingSelectionActions | null,
	store: ShowObjectsStore,
): CapturedGroupAuthority | null {
	if (!actions || actions.status() !== "ready") return null;
	const snapshot = store.getSnapshot();
	if (!snapshot.showId || !snapshot.readyCollections.has("group")) return null;
	return { actions };
}

/** The one scoped Group activation contract shared by the Group Pool and Group Strip. */
export function useGroupSelectionActions(active = true): GroupSelectionActions {
	const actions = useProgrammingSelectionActions(active);
	const store = useShowObjectsStore();
	const selectLive = useCallback(
		(group: GroupObject): GroupSelectionWrite => {
			const authority = captureGroupAuthority(actions, store);
			if (!authority) return null;
			return authority.actions.gesture({
				source: { type: "live_group", groupId: group.id },
				resolvedFixtures: group.body.fixtures,
			});
		},
		[actions, store],
	);
	const stageLive = useCallback(
		(group: GroupObject) => {
			const authority = captureGroupAuthority(actions, store);
			return (
				authority?.actions.stageGesture?.({
					source: { type: "live_group", groupId: group.id },
					resolvedFixtures: group.body.fixtures,
				}) ?? null
			);
		},
		[actions, store],
	);
	const selectFrozen = useCallback(
		(group: GroupObject): GroupSelectionWrite => {
			const authority = captureGroupAuthority(actions, store);
			if (!authority) return null;
			return authority.actions.selectGroup({
				groupId: group.id,
				resolvedFixtures: group.body.fixtures,
				mode: "frozen",
				rule: WHOLE_GROUP_RULE,
			});
		},
		[actions, store],
	);
	return useMemo(
		() => ({ selectLive, selectFrozen, stageLive }),
		[selectFrozen, selectLive, stageLive],
	);
}
