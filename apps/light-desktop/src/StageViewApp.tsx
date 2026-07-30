import { useEffect, useState } from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { AppProvider } from "./state/AppContext";
import { StageWindow } from "./windows/StageWindow";

export function useAuxiliaryStagePerformance() {
	const [focused, setFocused] = useState(false);
	useEffect(() => {
		let warmedUp = false;
		const handleFocus = () => {
			if (warmedUp) setFocused(true);
		};
		const handleBlur = () => setFocused(false);
		const warmup = window.setTimeout(() => {
			warmedUp = true;
			setFocused(document.hasFocus());
		}, 250);
		window.addEventListener("focus", handleFocus);
		window.addEventListener("blur", handleBlur);
		return () => {
			window.clearTimeout(warmup);
			window.removeEventListener("focus", handleFocus);
			window.removeEventListener("blur", handleBlur);
		};
	}, []);
	return focused
		? {
				renderQuality: "lines_and_beams" as const,
				visualizationIntervalMillis: 100,
				pixelRatioCap: 1.25,
			}
		: {
				renderQuality: "lines_only" as const,
				visualizationIntervalMillis: 1_000,
				pixelRatioCap: 0.25,
			};
}

/**
 * Dedicated view-only 3D stage window opened from the Show Patch (long-press "Preview Stage").
 * Clicking fixtures applies the shared desk programming selection, so the patch sheet in the
 * main window highlights the same rows; positions themselves are edited in the Patch.
 */
export function StageViewApp() {
	const performance = useAuxiliaryStagePerformance();
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
							stageRenderQuality={performance.renderQuality}
							visualizationIntervalMillis={
								performance.visualizationIntervalMillis
							}
							pixelRatioCap={performance.pixelRatioCap}
							showSelection
							showFloorGrid
							showBeamGuides
						/>
					</div>
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
