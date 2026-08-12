import { useEffect } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { NativeStagePane } from "./useNativeStagePane";
import type { StageSelectionModel } from "./useStageSelection";

/**
 * Telling the renderer what the operator has selected, so it can draw it.
 *
 * Selection travels both ways across this boundary, and only one direction decides anything. The
 * renderer resolves what is under a pointer and reports the fixture; the desk decides what pointing
 * at it meant; and the answer comes back here to be drawn. A renderer that kept its own idea of
 * what is selected would be a second answer to the one question an operator has to be able to
 * trust — and the two would disagree the first time a selection was made anywhere else, which on a
 * desk is most of the time.
 *
 * Sent on change rather than per frame. A selection is a handful of ids that changes when an
 * operator does something, not something the picture has to be reminded of sixty times a second.
 * Show Selection masks only this renderer payload: the desk retains the authoritative selection,
 * so turning display back on can restore it and picking can keep changing it while it is hidden.
 */
export function useStagePaneSelection(
	pane: NativeStagePane,
	selection: StageSelectionModel,
	showSelection: boolean,
) {
	const bridge = useDesktopBridge();
	const active = pane.active;
	// Joined rather than passed as an array: the effect has to re-run when the *contents* change,
	// and a fresh array with the same ids in it is a different value every render.
	const fixtures = selection.fixtureIds.join(",");
	const visibleFixtures = showSelection ? fixtures : "";

	useEffect(() => {
		if (!active) return;
		void bridge.setStagePaneSelection(
			visibleFixtures ? visibleFixtures.split(",") : [],
		);
	}, [active, bridge, visibleFixtures]);
}
