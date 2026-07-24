import { ServerRuntime } from "./api/ServerRuntime";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { AppProvider } from "./state/AppContext";
import { StageWindow } from "./windows/StageWindow";

/**
 * Dedicated view-only 3D stage window opened from the Show Patch (long-press "Preview Stage").
 * Clicking fixtures applies the shared desk programming selection, so the patch sheet in the
 * main window highlights the same rows; positions themselves are edited in the Patch.
 */
export function StageViewApp() {
	return (
		<ServerRuntime sessionRole="secondary">
			<AppProvider>
				<PatchFeatureBoundary>
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
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
