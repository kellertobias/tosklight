import { useEffect } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { NativeStagePane } from "./useNativeStagePane";
import type { StageSelectionModel } from "./useStageSelection";

/**
 * Applying what the operator pointed at in the native pane.
 *
 * The renderer answers a click with a fixture, because it is the only side that knows what the
 * geometry put under the pointer. What that *means* is the desk's: the same gesture the desk's own
 * Stage applies, through the same selection actions, so a fixture picked in either renderer ends up
 * selected the same way and by the same authority.
 *
 * Drained on a timer rather than pushed. The desk already polls the pane, and a selection arriving
 * through a second mechanism could be applied out of order with the first — which for a selection
 * means the operator ending up with a different rig than the one they clicked.
 */
const DRAIN_INTERVAL = 60;

export function useStagePanePicks(
	pane: NativeStagePane,
	selection: StageSelectionModel,
	interactive = true,
) {
	const bridge = useDesktopBridge();
	const active = pane.active;
	useEffect(() => {
		if (!active || !interactive) return;
		let cancelled = false;
		const drain = async () => {
			const picks = await bridge.takeStagePanePicks();
			if (cancelled || picks.length === 0) return;
			for (const [fixtureId, additive] of picks) {
				// A click on nothing clears the selection, which is what the desk's own Stage does
				// with a click on empty floor.
				if (!fixtureId) {
					await selection.replaceFixtureIds([]);
					continue;
				}
				// Shift or Command over a fixture already selected removes it, exactly as the
				// desk's Stage does, so the two renderers cannot disagree about what a modifier
				// means.
				await selection.applyFixtureGesture(
					fixtureId,
					additive && selection.fixtureIdSet.has(fixtureId) ? "remove" : "add",
				);
			}
		};
		const timer = window.setInterval(() => void drain(), DRAIN_INTERVAL);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [active, interactive, bridge, selection]);
}
