import { useMemo } from "react";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";

export function usePatchSelection(active: boolean) {
	const projection = useProgrammingSelectionView(active);
	const actions = useProgrammingSelectionActions(active);
	const fixtureIds = useMemo(
		() => (projection ? new Set(projection.selected) : null),
		[projection],
	);
	return {
		fixtureIds,
		actions: projection ? actions : null,
	};
}
