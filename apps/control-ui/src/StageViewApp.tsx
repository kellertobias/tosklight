import { ServerProvider } from "./api/ServerContext";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppProvider } from "./state/AppContext";
import { StageWindow } from "./windows/StageWindow";

/**
 * Dedicated view-only 3D stage window opened from the Show Patch (long-press "Preview Stage").
 * Clicking fixtures applies the shared desk programming selection, so the patch sheet in the
 * main window highlights the same rows; positions themselves are edited in the Patch.
 */
export function StageViewApp() {
	return (
		<ServerProvider sessionRole="secondary">
			<AppProvider>
				<div className="stage-view-shell">
					<StageWindow
						compact
						stageView="3d"
						showGroupShortcuts={false}
						followPreload={false}
						showSelection
						showFloorGrid
						showBeamGuides
					/>
				</div>
			</AppProvider>
			<DeskLockOverlay />
		</ServerProvider>
	);
}
