import { Button } from "@tosklight/ui";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import { FixturePatchSetupContent } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { usePatchPreviewHighlightDmx } from "../features/configuration/ConfigurationState";
import { useHighlightActions } from "../features/highlight/HighlightState";
import { usePatch } from "../features/patch/PatchContext";
import { PatchFeatureBoundary } from "../features/patch/PatchFeatureBoundary";
import { useProgrammingSelectionView } from "../features/programmingInteraction/ProgrammingInteractionView";
import { useDesktopBridge } from "../platform/desktop";
import { StageWindow } from "./StageWindow";
import type { WindowProps } from "./windowTypes";

export function PatchWindow({ active = true }: WindowProps) {
	const [tab, setTab] = useState<"fixtures" | "media">("fixtures");
	return (
		<PatchFeatureBoundary>
			{tab === "media" ? (
				<PatchMediaWindow
					active={active}
					onFixtures={() => setTab("fixtures")}
				/>
			) : (
				<PatchWindowContent active={active} onMedia={() => setTab("media")} />
			)}
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
	const highlightActions = useHighlightActions();
	const patch = usePatch();
	const patchPreviewHighlightDmx = usePatchPreviewHighlightDmx();
	const [stagePreviewOpen, setStagePreviewOpen] = useState(false);
	const stagePreview = useRef<HTMLElement>(null);
	const setPatchPreviewHighlight = useRef(
		highlightActions?.setPatchPreviewHighlight ?? (async () => false),
	);
	const [stagePreviewClearance, setStagePreviewClearance] = useState(0);
	const previewVisible = stagePreviewOpen;
	const dmxPreview = active && previewVisible && patchPreviewHighlightDmx;
	const selection = useProgrammingSelectionView(dmxPreview);
	useEffect(() => {
		if (highlightActions)
			setPatchPreviewHighlight.current =
				highlightActions.setPatchPreviewHighlight;
	}, [highlightActions]);
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
	const desktop = useDesktopBridge();
	const tauri = desktop.available;
	const [previewOffset, setPreviewOffset] = useState({ x: 0, y: 0 });
	const previewDrag = useRef<{
		pointerId: number;
		x: number;
		y: number;
		offsetX: number;
		offsetY: number;
	} | null>(null);
	const beginPreviewDrag = (event: ReactPointerEvent<HTMLElement>) => {
		try {
			event.currentTarget.setPointerCapture(event.pointerId);
		} catch {
			// A capture failure only loosens tracking outside the grip; the drag still works.
		}
		previewDrag.current = {
			pointerId: event.pointerId,
			x: event.clientX,
			y: event.clientY,
			offsetX: previewOffset.x,
			offsetY: previewOffset.y,
		};
	};
	const movePreviewDrag = (event: ReactPointerEvent<HTMLElement>) => {
		const drag = previewDrag.current;
		if (!drag || drag.pointerId !== event.pointerId) return;
		setPreviewOffset({
			x: drag.offsetX + event.clientX - drag.x,
			y: Math.min(0, drag.offsetY + event.clientY - drag.y),
		});
	};
	const finishPreviewDrag = () => {
		previewDrag.current = null;
	};
	return (
		<div
			className={`patch-window ${previewVisible ? "stage-preview-open" : ""}`}
		>
			<FixturePatchSetupContent
				active={active}
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
					style={{
						transform: `translate(${previewOffset.x}px, ${previewOffset.y}px)`,
					}}
				>
					<Button
						className="patch-stage-overlay-grip"
						aria-label="Move Stage preview"
						onPointerDown={beginPreviewDrag}
						onPointerMove={movePreviewDrag}
						onPointerUp={finishPreviewDrag}
						onPointerCancel={finishPreviewDrag}
					/>
					<StageWindow
						active={active}
						compact
						stageView={tauri ? "3d" : "2d"}
						showGroupShortcuts={false}
						followPreload={false}
						showSelection={false}
						showFloorGrid
						environmentBrightness={1}
						patchSelectionPreview
						patchedFixtures={patch.fixtures}
					/>
				</aside>
			)}
		</div>
	);
}

function PatchMediaWindow({
	active,
	onFixtures,
}: {
	active: boolean;
	onFixtures: () => void;
}) {
	return (
		<>
			<WindowHeader
				title="Show Patch"
				info={{ primary: "Media Servers" }}
				groups={[{
					id: "patch-kind",
					kind: "tabs",
					activeId: "media",
					onActiveChange: (id) => id === "fixtures" && onFixtures(),
					actions: [
						{ id: "fixtures", label: "Fixtures" },
						{
							id: "media",
							label: "Media Servers",
						},
					],
				}]}
			/>
			<WindowScrollArea>
				<main>
					<MediaServerSetup active={active} />
				</main>
			</WindowScrollArea>
		</>
	);
}
