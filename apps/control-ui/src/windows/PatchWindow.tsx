import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { FixturePatchSetup } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { WindowHeader, WindowScrollArea } from "../components/window-kit";
import { StageWindow } from "./StageWindow";
import { useServer } from "../api/ServerContext";
import { useDesktopBridge } from "../platform/desktop";

export function PatchWindow(_: WindowProps) {
	const server = useServer();
	const [tab, setTab] = useState<"fixtures" | "media">("fixtures");
	const [stagePreviewOpen, setStagePreviewOpen] = useState(false);
	const stagePreview = useRef<HTMLElement>(null);
	const setPatchPreviewHighlight = useRef(server.setPatchPreviewHighlight);
	const [stagePreviewClearance, setStagePreviewClearance] = useState(0);
	const previewVisible = stagePreviewOpen && tab === "fixtures";
	const dmxPreview =
		previewVisible &&
		(server.configuration?.patch_preview_highlight_dmx ?? false);
	useEffect(() => {
		setPatchPreviewHighlight.current = server.setPatchPreviewHighlight;
	}, [server.setPatchPreviewHighlight]);
	useEffect(() => {
		void setPatchPreviewHighlight.current(
			dmxPreview,
			dmxPreview ? server.selectedFixtures : [],
		);
	}, [dmxPreview, server.selectedFixtures]);
	useEffect(
		() => () => {
			void setPatchPreviewHighlight.current(false);
		},
		[],
	);
	useLayoutEffect(() => {
		const overlay = stagePreview.current;
		if (!previewVisible || !overlay) return setStagePreviewClearance(0);
		const measure = () =>
			setStagePreviewClearance(
				Math.ceil(overlay.getBoundingClientRect().height) + 20,
			);
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(overlay);
		return () => observer.disconnect();
	}, [previewVisible]);
	const tauri = useDesktopBridge().available;
	return (
		<div
			className={`patch-window ${previewVisible ? "stage-preview-open" : ""}`}
		>
			{tab === "fixtures" ? (
				<FixturePatchSetup
					onMedia={() => setTab("media")}
					stagePreviewOpen={stagePreviewOpen}
					stagePreviewClearance={stagePreviewClearance}
					onStagePreview={() => setStagePreviewOpen((open) => !open)}
				/>
			) : (
				<>
					<WindowHeader
						title="Show Patch"
						info={{ primary: "Media Servers" }}
						actions={[
							[
								{
									id: "fixtures",
									label: "Fixtures",
									onClick: () => setTab("fixtures"),
								},
								{
									id: "media",
									label: "Media Servers",
									active: true,
									onClick: () => undefined,
								},
							],
						]}
					/>
					<WindowScrollArea>
						<main>
							<MediaServerSetup />
						</main>
					</WindowScrollArea>
				</>
			)}
			{previewVisible && (
				<aside
					ref={stagePreview}
					className="patch-stage-overlay"
					aria-label="Patch Stage preview"
				>
					<StageWindow
						compact
						stageView={tauri ? "3d" : "2d"}
						showGroupShortcuts={false}
						followPreload={false}
						showSelection={false}
						showFloorGrid
						showBeamGuides
						environmentBrightness={1}
						patchSelectionPreview
					/>
				</aside>
			)}
		</div>
	);
}
