import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { WindowProps } from "./windowTypes";
import {
	FixturePatchSetupContent,
	PatchFeatureBoundary,
} from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { WindowHeader, WindowScrollArea } from "../components/window-kit";
import { StageWindow } from "./StageWindow";
import { useServer } from "../api/ServerContext";
import { useDesktopBridge } from "../platform/desktop";
import { usePatch } from "../features/patch/PatchContext";
import { useProgrammingSelectionView } from "../features/programmingInteraction/ProgrammingInteractionView";

export function PatchWindow({ active = true }: WindowProps) {
	const [tab, setTab] = useState<"fixtures" | "media">("fixtures");
	if (tab === "media")
		return <PatchMediaWindow onFixtures={() => setTab("fixtures")} />;
	return (
		<PatchFeatureBoundary>
			<PatchWindowContent
				active={active}
				onMedia={() => setTab("media")}
			/>
		</PatchFeatureBoundary>
	);
}

function PatchWindowContent({
	active,
	onMedia,
}: {
	active: boolean;
	onMedia: () => void;
}) {
	const server = useServer();
	const patch = usePatch();
	const [stagePreviewOpen, setStagePreviewOpen] = useState(false);
	const stagePreview = useRef<HTMLElement>(null);
	const setPatchPreviewHighlight = useRef(server.setPatchPreviewHighlight);
	const [stagePreviewClearance, setStagePreviewClearance] = useState(0);
	const previewVisible = stagePreviewOpen;
	const dmxPreview =
		active &&
		previewVisible &&
		(server.configuration?.patch_preview_highlight_dmx ?? false);
	const selection = useProgrammingSelectionView(dmxPreview);
	useEffect(() => {
		setPatchPreviewHighlight.current = server.setPatchPreviewHighlight;
	}, [server.setPatchPreviewHighlight]);
	useEffect(() => {
		void setPatchPreviewHighlight.current(
			dmxPreview,
			dmxPreview ? [...(selection?.selected ?? [])] : [],
		);
	}, [dmxPreview, selection?.selected]);
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
			<FixturePatchSetupContent
				onMedia={onMedia}
				stagePreviewOpen={stagePreviewOpen}
				stagePreviewClearance={stagePreviewClearance}
				onStagePreview={() => setStagePreviewOpen((open) => !open)}
			/>
			{previewVisible && (
				<aside
					ref={stagePreview}
					className="patch-stage-overlay"
					aria-label="Patch Stage preview"
				>
					<StageWindow
						active={active}
						compact
						stageView={tauri ? "3d" : "2d"}
						showGroupShortcuts={false}
						followPreload={false}
						showSelection={false}
						showFloorGrid
						showBeamGuides
						environmentBrightness={1}
						patchSelectionPreview
						patchedFixtures={patch.fixtures}
					/>
				</aside>
			)}
		</div>
	);
}

function PatchMediaWindow({ onFixtures }: { onFixtures: () => void }) {
	return (
		<>
			<WindowHeader
				title="Show Patch"
				info={{ primary: "Media Servers" }}
				actions={[
					[
						{ id: "fixtures", label: "Fixtures", onClick: onFixtures },
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
	);
}
