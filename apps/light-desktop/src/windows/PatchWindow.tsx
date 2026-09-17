import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import { FixturePatchSetupContent } from "../components/setup/FixturePatchSetup";
import { PATCH_IMPORT_CSV_EVENT } from "../components/setup/fixturePatch/ShowPatchSettings";
import {
	type ShowPatchView,
	ShowPatchViewHeader,
} from "../components/setup/fixturePatch/showPatchHeader";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { PsnSetup } from "../components/setup/PsnSetup";
import {
	mediaDiscoveryGroup,
	useMediaServerDiscovery,
} from "../components/setup/useMediaServerDiscovery";
import { PatchFeatureBoundary } from "../features/patch/PatchFeatureBoundary";
import { useDesktopBridge } from "../platform/desktop";
import type { WindowProps } from "./windowTypes";

export function PatchWindow({
	active = true,
	compact = false,
	patchView = "fixtures",
	patchHiddenColumns,
	paneId,
}: WindowProps) {
	const [tab, setTabState] = useState<ShowPatchView>(patchView);
	// A pending Import CSV; cleared on every view change so a remount never reopens it.
	const [csvImportRequest, setCsvImportRequest] = useState(0);
	const setTab = (view: ShowPatchView) => {
		setCsvImportRequest(0);
		setTabState(view);
	};
	const importCsv = () => {
		setTabState("fixtures");
		setCsvImportRequest((request) => request + 1);
	};
	useEffect(() => {
		if (!paneId) return;
		const onRequest = (event: Event) => {
			if ((event as CustomEvent<{ paneId?: string }>).detail?.paneId !== paneId)
				return;
			setTabState("fixtures");
			setCsvImportRequest((request) => request + 1);
		};
		window.addEventListener(PATCH_IMPORT_CSV_EVENT, onRequest);
		return () => window.removeEventListener(PATCH_IMPORT_CSV_EVENT, onRequest);
	}, [paneId]);
	return (
		<PatchFeatureBoundary>
			{tab !== "fixtures" && (
				<PatchConfigurationWindow
					view={tab}
					active={active}
					compact={compact}
					onView={setTab}
					onImportCsv={importCsv}
				/>
			)}
			{tab === "fixtures" && (
				<PatchWindowContent
					active={active}
					compact={compact}
					hiddenColumns={patchHiddenColumns}
					csvImportRequest={csvImportRequest}
					onMedia={() => setTab("media")}
					onTracking={() => setTab("tracking")}
				/>
			)}
		</PatchFeatureBoundary>
	);
}

function PatchWindowContent({
	active,
	compact,
	hiddenColumns,
	csvImportRequest,
	onMedia,
	onTracking,
}: {
	active: boolean;
	compact: boolean;
	hiddenColumns: WindowProps["patchHiddenColumns"];
	csvImportRequest: number;
	onMedia: () => void;
	onTracking: () => void;
}) {
	const desktop = useDesktopBridge();
	const [rendererError, setRendererError] = useState<string | null>(null);
	const openStageRenderer = async () => {
		setRendererError(null);
		try {
			await desktop.openVisualizer();
		} catch (error) {
			setRendererError(
				error instanceof Error
					? error.message
					: "The Stage renderer could not be opened.",
			);
		}
	};
	return (
		<div className="patch-window">
			<FixturePatchSetupContent
				active={active}
				compact={compact}
				hiddenColumns={hiddenColumns}
				csvImportRequest={csvImportRequest}
				onMedia={onMedia}
				onTracking={onTracking}
				onOpenStageWindow={desktop.available ? openStageRenderer : undefined}
			/>
			{rendererError && <p role="alert">{rendererError}</p>}
		</div>
	);
}

/**
 * Media Servers and Tracking as screens of the Show Patch.
 *
 * They sit beside Fixtures because that is what they are: part of setting the show up, done once
 * with the rig. Each gets the same header shape as Fixtures and one scroller filling the window,
 * with the same inner margins as Settings, so the last control is always reachable.
 */
function PatchConfigurationWindow({
	view,
	active,
	compact,
	onView,
	onImportCsv,
}: {
	view: "media" | "tracking";
	active: boolean;
	compact: boolean;
	onView: (view: ShowPatchView) => void;
	onImportCsv: () => void;
}) {
	// Discovery belongs to the window so Refresh Discovery can sit in the title.
	const discovery = useMediaServerDiscovery(active && view === "media");
	return (
		<div className="patch-window patch-configuration-window" data-view={view}>
			<ShowPatchViewHeader
				view={view}
				compact={compact}
				onView={onView}
				onImportCsv={onImportCsv}
				groups={view === "media" ? [mediaDiscoveryGroup(discovery)] : []}
			/>
			<WindowScrollArea className="patch-configuration-scroll">
				<main className="patch-configuration-content">
					{view === "media" ? (
						<MediaServerSetup active={active} discovery={discovery} />
					) : (
						<PsnSetup active={active} />
					)}
				</main>
			</WindowScrollArea>
		</div>
	);
}
